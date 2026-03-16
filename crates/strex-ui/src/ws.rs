//! WebSocket run handler — streams execution events to the browser.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::Deserialize;

use std::path::Path;

use strex_core::{
    execute_collection_streaming, parse_collection, parse_csv, parse_json, AssertionFailure,
    AssertionType, ExecutionContext, HttpResponse, RequestOutcome, RunnerOpts,
};

use crate::events::WsEvent;

/// Configuration sent by the browser as the first WebSocket message.
#[derive(Debug, Deserialize)]
pub struct RunConfig {
    /// Path to the collection YAML file.
    pub collection: String,
    /// Optional path to a data file (.csv or .json).
    pub data: Option<String>,
    /// Number of concurrent iteration tasks for data-driven runs.
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    /// When true, stop launching new iterations after the first failure.
    #[serde(default)]
    pub fail_fast: bool,
}

fn default_concurrency() -> usize {
    1
}

/// Format an [`AssertionFailure`] as a human-readable string.
///
/// - `Status`   → `"status expected {expected}, got {actual}"`
/// - `JsonPath` → `"jsonPath {path} expected {expected}, got {actual}"`
/// - `Header`   → `"header expected {expected}, got {actual}"`
/// - `Script`   → `"{expected}"` (`actual` is always empty for script assertions)
fn format_failure(failure: &AssertionFailure) -> String {
    match failure.assertion_type {
        AssertionType::Status => {
            format!(
                "status expected {}, got {}",
                failure.expected, failure.actual
            )
        }
        AssertionType::JsonPath => {
            let path = failure.path.as_deref().unwrap_or("?");
            format!(
                "jsonPath {} expected {}, got {}",
                path, failure.expected, failure.actual
            )
        }
        AssertionType::Header => {
            format!(
                "header expected {}, got {}",
                failure.expected, failure.actual
            )
        }
        AssertionType::Script => failure.expected.clone(),
    }
}

/// Maximum response body bytes to forward to the browser.
const BODY_LIMIT: usize = 10_240;

/// Fields extracted from a [`RequestOutcome`] for use in [`WsEvent::RequestCompleted`].
///
/// `(passed, status, failures, error, response_body, response_headers, request_body, url)`
type OutcomeFields = (
    bool,
    Option<u16>,
    Vec<String>,
    Option<String>,
    Option<String>,
    Option<std::collections::HashMap<String, String>>,
    Option<String>,
    String,
);

/// Truncate `body` to at most [`BODY_LIMIT`] bytes, respecting UTF-8 character boundaries.
///
/// Appends `" [truncated]"` if the body was cut.
fn truncate_body(body: &str) -> String {
    if body.len() <= BODY_LIMIT {
        body.to_string()
    } else {
        let boundary = (0..=BODY_LIMIT)
            .rev()
            .find(|&i| body.is_char_boundary(i))
            .unwrap_or(0); // unreachable: is_char_boundary(0) is always true for any &str
        format!("{} [truncated]", &body[..boundary])
    }
}

/// Send a [`WsEvent`] serialized as JSON text over the WebSocket.
///
/// Returns `Ok(())` on success; swallows send errors silently (client may have disconnected).
async fn send_event(socket: &mut WebSocket, event: WsEvent) {
    if let Ok(json) = serde_json::to_string(&event) {
        let _ = socket.send(Message::Text(json)).await;
    }
}

/// Send a fatal error event and close the socket.
async fn send_error(socket: &mut WebSocket, message: String) {
    // Reuse RunFinished with 0/0 is not clean; use a Text message describing the error
    // so the browser can display it.  The WsEvent enum has no Error variant, so we emit
    // a raw JSON object that the frontend treats as an error signal.
    let json = serde_json::json!({ "type": "error", "message": message }).to_string();
    let _ = socket.send(Message::Text(json)).await;
}

/// WebSocket upgrade handler — upgrades the HTTP connection and starts `handle_socket`.
pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

/// Drive the full request/response lifecycle for one WebSocket connection.
///
/// Reads the first text message as a [`RunConfig`], executes the collection while
/// streaming [`WsEvent`]s, then closes the connection.
async fn handle_socket(mut socket: WebSocket) {
    // Wait for the first text message containing the run configuration.
    let config = loop {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => match serde_json::from_str::<RunConfig>(&text) {
                Ok(cfg) => break cfg,
                Err(e) => {
                    send_error(&mut socket, format!("Invalid run config: {e}")).await;
                    return;
                }
            },
            Some(Ok(_)) => continue, // ignore non-text frames
            _ => return,             // connection closed or error before config
        }
    };

    if let Err(e) = run_collection_and_stream(&mut socket, config).await {
        send_error(&mut socket, e.to_string()).await;
    }
}

/// Execute the collection described by `config`, streaming [`WsEvent`]s over `socket`.
///
/// Returns `Err` only for infrastructure failures (file I/O, parse errors).
/// Per-request assertion failures and HTTP errors are reported via [`WsEvent::RequestCompleted`].
async fn run_collection_and_stream(
    socket: &mut WebSocket,
    config: RunConfig,
) -> anyhow::Result<()> {
    let collection = parse_collection(Path::new(&config.collection))?;

    if let Some(data_path) = config.data {
        // --- Data-driven run ---
        let content = std::fs::read_to_string(&data_path)
            .map_err(|e| anyhow::anyhow!("Could not read data file '{data_path}': {e}"))?;

        let rows = if data_path.ends_with(".csv") {
            parse_csv(&content).map_err(|e| anyhow::anyhow!("CSV parse error: {e}"))?
        } else if data_path.ends_with(".json") {
            parse_json(&content).map_err(|e| anyhow::anyhow!("JSON parse error: {e}"))?
        } else {
            return Err(anyhow::anyhow!(
                "Unsupported data file extension (expected .csv or .json): {data_path}"
            ));
        };

        let total = rows.len() * collection.requests.len();
        send_event(socket, WsEvent::RunStarted { total }).await;

        type DataChannelPayload = (usize, std::collections::HashMap<String, String>, Option<(String, String, u64, OutcomeFields)>);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DataChannelPayload>();

        let arc_col = std::sync::Arc::new(collection);
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(config.concurrency));
        let fail_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fail_fast = config.fail_fast;

        let runner = tokio::spawn(async move {
            let mut join_set: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

            for (idx, row) in rows.into_iter().enumerate() {
                if fail_fast && fail_flag.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }

                let col = std::sync::Arc::clone(&arc_col);
                let sem = std::sync::Arc::clone(&semaphore);
                let flag = std::sync::Arc::clone(&fail_flag);
                let tx = tx.clone();
                let row_clone = row.clone();

                let permit = match sem.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                if fail_fast && flag.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }

                join_set.spawn(async move {
                    let _permit = permit;
                    let ctx = ExecutionContext::new_with_data(&col, &row_clone);
                    let iteration = idx + 1;

                    let _ = tx.send((iteration, row_clone.clone(), None));

                    let col_result = execute_collection_streaming(
                        &col,
                        ctx,
                        RunnerOpts::default(),
                        |req_result| {
                            let tx = tx.clone();
                            let row_for_send = row_clone.clone();
                            let name = req_result.name.clone();
                            let method = col
                                .requests
                                .iter()
                                .find(|r| r.name == name)
                                .map(|r| r.method.clone())
                                .unwrap_or_default();
                            let duration_ms = req_result.duration_ms;
                            let fields = outcome_fields(&req_result.outcome, &req_result.response);
                            async move {
                                let _ = tx.send((iteration, row_for_send, Some((name, method, duration_ms, fields))));
                            }
                        },
                    )
                    .await;

                    if fail_fast && !col_result.passed() {
                        flag.store(true, std::sync::atomic::Ordering::Release);
                    }
                });
            }

            while join_set.join_next().await.is_some() {}
        });

        let mut passed_count = 0usize;
        let mut failed_count = 0usize;
        let mut total_duration_ms: u64 = 0;

        while let Some((iteration, row, payload)) = rx.recv().await {
            match payload {
                None => {
                    send_event(socket, WsEvent::IterationStarted { iteration, row }).await;
                }
                Some((name, method, duration_ms, fields)) => {
                    let (passed, status, failures, error, response_body, response_headers, request_body, url) = fields;
                    if passed {
                        passed_count += 1;
                    } else {
                        failed_count += 1;
                    }
                    total_duration_ms += duration_ms;
                    send_event(
                        socket,
                        WsEvent::RequestCompleted {
                            name,
                            method,
                            url,
                            passed,
                            status,
                            duration_ms,
                            failures,
                            error,
                            response_body,
                            response_headers,
                            request_body,
                        },
                    )
                    .await;
                }
            }
        }

        let _ = runner.await;

        let total_requests = passed_count + failed_count;
        let avg_response_ms = if total_requests > 0 {
            total_duration_ms / total_requests as u64
        } else {
            0
        };

        send_event(
            socket,
            WsEvent::RunFinished {
                passed: passed_count,
                failed: failed_count,
                total_duration_ms,
                avg_response_ms,
            },
        )
        .await;
    } else {
        // --- Single run ---
        let total = collection.requests.len();
        send_event(socket, WsEvent::RunStarted { total }).await;

        let method_map: std::collections::HashMap<String, String> = collection
            .requests
            .iter()
            .map(|r| (r.name.clone(), r.method.clone()))
            .collect();

        type ChannelPayload = (String, u64, OutcomeFields);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ChannelPayload>();

        let ctx = ExecutionContext::new(&collection);
        let runner = tokio::spawn(async move {
            execute_collection_streaming(
                &collection,
                ctx,
                RunnerOpts::default(),
                |req_result| {
                    let tx = tx.clone();
                    let name = req_result.name.clone();
                    let duration_ms = req_result.duration_ms;
                    let fields = outcome_fields(&req_result.outcome, &req_result.response);
                    async move {
                        let _ = tx.send((name, duration_ms, fields));
                    }
                },
            )
            .await
        });

        let mut passed_count = 0usize;
        let mut failed_count = 0usize;
        let mut total_duration_ms: u64 = 0;

        while let Some((name, duration_ms, fields)) = rx.recv().await {
            let (passed, status, failures, error, response_body, response_headers, request_body, url) =
                fields;

            if passed {
                passed_count += 1;
            } else {
                failed_count += 1;
            }

            total_duration_ms += duration_ms;

            let method = method_map.get(&name).cloned().unwrap_or_default();

            send_event(
                socket,
                WsEvent::RequestCompleted {
                    name,
                    method,
                    url,
                    passed,
                    status,
                    duration_ms,
                    failures,
                    error,
                    response_body,
                    response_headers,
                    request_body,
                },
            )
            .await;
        }

        let _ = runner.await;

        let total_requests = passed_count + failed_count;
        let avg_response_ms = if total_requests > 0 {
            total_duration_ms / total_requests as u64
        } else {
            0
        };

        send_event(
            socket,
            WsEvent::RunFinished {
                passed: passed_count,
                failed: failed_count,
                total_duration_ms,
                avg_response_ms,
            },
        )
        .await;
    }

    Ok(())
}

/// Decompose a [`RequestOutcome`] and optional HTTP response into the fields needed by
/// [`WsEvent::RequestCompleted`].
///
/// `response` is `None` when a stopping error occurred before the HTTP phase.
///
/// Returns an [`OutcomeFields`] tuple: `(passed, status, failures, error, response_body, response_headers, request_body, url)`.
fn outcome_fields(outcome: &RequestOutcome, response: &Option<HttpResponse>) -> OutcomeFields {
    let status = response.as_ref().map(|r| r.status);
    let response_body = response.as_ref().map(|r| truncate_body(&r.body));
    let response_headers = response.as_ref().map(|r| r.headers.clone());
    let request_body = response
        .as_ref()
        .and_then(|r| r.request_body.as_deref().map(truncate_body));
    let url = response
        .as_ref()
        .map(|r| r.url.clone())
        .unwrap_or_default();
    match outcome {
        RequestOutcome::Passed => (
            true,
            status,
            vec![],
            None,
            response_body,
            response_headers,
            request_body,
            url,
        ),
        RequestOutcome::AssertionsFailed(failures) => {
            let msgs = failures.iter().map(format_failure).collect();
            (
                false,
                status,
                msgs,
                None,
                response_body,
                response_headers,
                request_body,
                url,
            )
        }
        RequestOutcome::Error(e) => (
            false,
            status,
            vec![],
            Some(e.to_string()),
            response_body,
            response_headers,
            request_body,
            url,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strex_core::{AssertionFailure, AssertionType};

    #[test]
    fn format_failure_status() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Status,
            expected: "200".into(),
            actual: "404".into(),
            path: None,
        };
        assert_eq!(format_failure(&f), "status expected 200, got 404");
    }

    #[test]
    fn format_failure_json_path() {
        let f = AssertionFailure {
            assertion_type: AssertionType::JsonPath,
            expected: "$.id exists".into(),
            actual: "null".into(),
            path: Some("$.id".into()),
        };
        assert_eq!(
            format_failure(&f),
            "jsonPath $.id expected $.id exists, got null"
        );
    }

    #[test]
    fn format_failure_header() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Header,
            expected: "application/json".into(),
            actual: "text/plain".into(),
            path: None,
        };
        assert_eq!(
            format_failure(&f),
            "header expected application/json, got text/plain"
        );
    }

    #[test]
    fn format_failure_script() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Script,
            expected: "must be non-empty".into(),
            actual: String::new(),
            path: None,
        };
        assert_eq!(format_failure(&f), "must be non-empty");
    }

    #[test]
    fn default_concurrency_is_one() {
        assert_eq!(default_concurrency(), 1);
    }

    #[test]
    fn run_config_deserializes_with_defaults() {
        let json = r#"{"collection":"./col.yaml"}"#;
        let cfg: RunConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.collection, "./col.yaml");
        assert!(cfg.data.is_none());
        assert_eq!(cfg.concurrency, 1);
        assert!(!cfg.fail_fast);
    }

    #[test]
    fn run_config_deserializes_full() {
        let json =
            r#"{"collection":"col.yaml","data":"data.csv","concurrency":4,"fail_fast":true}"#;
        let cfg: RunConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.data.as_deref(), Some("data.csv"));
        assert_eq!(cfg.concurrency, 4);
        assert!(cfg.fail_fast);
    }

    #[test]
    fn error_json_has_correct_shape() {
        let json = serde_json::json!({ "type": "error", "message": "test" }).to_string();
        assert!(json.contains(r#""type":"error""#));
        assert!(json.contains(r#""message":"test""#));
    }

    #[test]
    fn outcome_fields_passed() {
        let (passed, status, failures, error, body, headers, _request_body, _url) =
            outcome_fields(&RequestOutcome::Passed, &None);
        assert!(passed);
        assert!(status.is_none());
        assert!(failures.is_empty());
        assert!(error.is_none());
        assert!(body.is_none());
        assert!(headers.is_none());
    }

    #[test]
    fn outcome_fields_assertions_failed() {
        let failures_in = vec![AssertionFailure {
            assertion_type: AssertionType::Status,
            expected: "200".into(),
            actual: "500".into(),
            path: None,
        }];
        let (passed, _status, failures, error, _body, _headers, _request_body, _url) =
            outcome_fields(&RequestOutcome::AssertionsFailed(failures_in), &None);
        assert!(!passed);
        assert_eq!(failures, vec!["status expected 200, got 500"]);
        assert!(error.is_none());
    }

    #[test]
    fn truncate_body_short_body_unchanged() {
        let body = "hello";
        assert_eq!(truncate_body(body), "hello");
    }

    #[test]
    fn truncate_body_long_body_is_truncated() {
        let body = "x".repeat(20_000);
        let result = truncate_body(&body);
        assert!(result.ends_with(" [truncated]"));
        assert_eq!(result.len(), BODY_LIMIT + " [truncated]".len());
    }

    #[test]
    fn outcome_fields_includes_request_body() {
        use std::collections::HashMap;
        use strex_core::RequestTiming;
        let response = Some(HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: String::new(),
            timing: RequestTiming::default(),
            request_body: Some("hello=world".to_string()),
            url: String::new(),
        });
        let (.., request_body, _url) = outcome_fields(&RequestOutcome::Passed, &response);
        assert_eq!(request_body.as_deref(), Some("hello=world"));
    }

    #[test]
    fn outcome_fields_truncates_long_request_body() {
        use std::collections::HashMap;
        use strex_core::RequestTiming;
        let long_body = "x".repeat(20_000);
        let response = Some(HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: String::new(),
            timing: RequestTiming::default(),
            request_body: Some(long_body),
            url: String::new(),
        });
        let (.., request_body, _url) = outcome_fields(&RequestOutcome::Passed, &response);
        let rb = request_body.unwrap();
        assert!(rb.ends_with(" [truncated]"));
        assert_eq!(rb.len(), BODY_LIMIT + " [truncated]".len());
    }
}
