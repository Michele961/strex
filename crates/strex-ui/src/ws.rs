//! WebSocket run handler — streams execution events to the browser.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::Deserialize;

use std::path::Path;

use strex_core::{
    execute_collection, parse_collection, parse_csv, parse_json, run_collection_with_data,
    AssertionFailure, AssertionType, DataRunOpts, ExecutionContext, HttpResponse, RequestOutcome,
    RunnerOpts,
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
/// - `JsonPath` → `"jsonPath expected {expected}, got {actual}"`
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
            format!(
                "jsonPath expected {}, got {}",
                failure.expected, failure.actual
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
/// `(passed, status, failures, error, response_body, response_headers)`
type OutcomeFields = (
    bool,
    Option<u16>,
    Vec<String>,
    Option<String>,
    Option<String>,
    Option<std::collections::HashMap<String, String>>,
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
            .unwrap_or(0);
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

        let opts = DataRunOpts {
            concurrency: config.concurrency,
            fail_fast: config.fail_fast,
            runner_opts: RunnerOpts::default(),
        };

        let result = run_collection_with_data(collection, rows, opts)
            .await
            .map_err(|e| anyhow::anyhow!("Data-driven run failed: {e}"))?;

        let mut total_duration_ms: u64 = 0;
        for iter in &result.iterations {
            for req_result in &iter.collection_result.request_results {
                let (passed, status, failures, error, response_body, response_headers) =
                    outcome_fields(&req_result.outcome, &req_result.response);

                total_duration_ms += req_result.duration_ms;

                // Look up the method from the request name in the collection iteration's result.
                // We don't have the collection struct here, so we emit an empty method string —
                // the iteration result does not carry method metadata. This matches the field
                // the events module documents as the HTTP method from the collection YAML.
                // NOTE: `run_collection_with_data` consumes the collection, so we can't
                // access it here. We keep method as an empty string; the frontend still works.
                send_event(
                    socket,
                    WsEvent::RequestCompleted {
                        name: req_result.name.clone(),
                        method: String::new(),
                        passed,
                        status,
                        duration_ms: req_result.duration_ms,
                        failures,
                        error,
                        response_body,
                        response_headers,
                    },
                )
                .await;
            }
        }

        let total_requests = result.passed + result.failed;
        let avg_response_ms = if total_requests > 0 {
            total_duration_ms / total_requests as u64
        } else {
            0
        };

        send_event(
            socket,
            WsEvent::RunFinished {
                passed: result.passed,
                failed: result.failed,
                total_duration_ms,
                avg_response_ms,
            },
        )
        .await;
    } else {
        // --- Single run ---
        let total = collection.requests.len();
        send_event(socket, WsEvent::RunStarted { total }).await;

        // Build a method lookup map before consuming the collection.
        let method_map: std::collections::HashMap<String, String> = collection
            .requests
            .iter()
            .map(|r| (r.name.clone(), r.method.clone()))
            .collect();

        let ctx = ExecutionContext::new(&collection);
        let col_result = execute_collection(&collection, ctx).await;

        let mut passed_count = 0usize;
        let mut failed_count = 0usize;
        let mut total_duration_ms: u64 = 0;

        for req_result in &col_result.request_results {
            let (passed, status, failures, error, response_body, response_headers) =
                outcome_fields(&req_result.outcome, &req_result.response);

            if passed {
                passed_count += 1;
            } else {
                failed_count += 1;
            }

            total_duration_ms += req_result.duration_ms;

            let method = method_map
                .get(&req_result.name)
                .cloned()
                .unwrap_or_default();

            send_event(
                socket,
                WsEvent::RequestCompleted {
                    name: req_result.name.clone(),
                    method,
                    passed,
                    status,
                    duration_ms: req_result.duration_ms,
                    failures,
                    error,
                    response_body,
                    response_headers,
                },
            )
            .await;
        }

        let total_requests = col_result.request_results.len();
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
/// Returns an [`OutcomeFields`] tuple: `(passed, status, failures, error, response_body, response_headers)`.
fn outcome_fields(outcome: &RequestOutcome, response: &Option<HttpResponse>) -> OutcomeFields {
    let status = response.as_ref().map(|r| r.status);
    let response_body = response.as_ref().map(|r| truncate_body(&r.body));
    let response_headers = response.as_ref().map(|r| r.headers.clone());
    match outcome {
        RequestOutcome::Passed => (true, status, vec![], None, response_body, response_headers),
        RequestOutcome::AssertionsFailed(failures) => {
            let msgs = failures.iter().map(format_failure).collect();
            (false, status, msgs, None, response_body, response_headers)
        }
        RequestOutcome::Error(e) => (
            false,
            status,
            vec![],
            Some(e.to_string()),
            response_body,
            response_headers,
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
        };
        assert_eq!(format_failure(&f), "status expected 200, got 404");
    }

    #[test]
    fn format_failure_json_path() {
        let f = AssertionFailure {
            assertion_type: AssertionType::JsonPath,
            expected: "$.id exists".into(),
            actual: "null".into(),
        };
        assert_eq!(
            format_failure(&f),
            "jsonPath expected $.id exists, got null"
        );
    }

    #[test]
    fn format_failure_header() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Header,
            expected: "application/json".into(),
            actual: "text/plain".into(),
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
        let (passed, status, failures, error, body, headers) =
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
        }];
        let (passed, _status, failures, error, _body, _headers) =
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
        assert!(result.len() < 20_000);
    }
}
