//! WebSocket event types streamed from the server to the browser.

use std::collections::HashMap;

use serde::Serialize;

/// A single console output entry emitted by a script.
#[derive(Debug, Serialize)]
pub(crate) struct ConsoleLog {
    /// Severity: `"log"`, `"warn"`, or `"error"`.
    pub level: &'static str,
    /// The logged message text.
    pub message: String,
}

/// Events streamed from the server to the browser over WebSocket.
///
/// Tagged with `"type"` field in JSON (e.g. `{"type":"run_started","total":3}`).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// RequestCompleted holds several Vecs and Strings required for serialisation; WsEvent
// values are short-lived and never stored in collections, so the size difference
// between variants has no practical impact.
#[allow(clippy::large_enum_variant)]
pub(crate) enum WsEvent {
    /// Sent once when the run begins.
    RunStarted {
        /// Total number of requests that will be executed.
        total: usize,
    },
    /// Sent once per data-driven iteration, immediately before its first request.
    IterationStarted {
        /// 1-based iteration number.
        iteration: usize,
        /// The data row for this iteration (column name → value).
        row: std::collections::HashMap<String, String>,
    },
    /// Sent after each request completes (pass or fail).
    RequestCompleted {
        /// Request name from the collection YAML.
        name: String,
        /// HTTP method (GET, POST, etc.).
        method: String,
        /// Fully-interpolated request URL.
        url: String,
        /// True if all assertions passed.
        passed: bool,
        /// HTTP status code. None if a network error prevented a response.
        status: Option<u16>,
        /// Duration of the full request lifecycle in milliseconds.
        duration_ms: u64,
        /// Assertion failure messages. Empty on pass or network error.
        failures: Vec<String>,
        /// Network/script error message. None if no error.
        error: Option<String>,
        /// Response body, truncated to 10 240 bytes. None on network error.
        response_body: Option<String>,
        /// Response headers (lowercase names). None on network error.
        response_headers: Option<HashMap<String, String>>,
        /// Serialised outgoing request body, truncated to 10 240 bytes. `None` when the request had no body.
        request_body: Option<String>,
        /// Console output from pre- and post-request scripts.
        logs: Vec<ConsoleLog>,
        /// Human-readable descriptions of declarative assertions that passed (e.g. "status 200").
        passed_assertions: Vec<String>,
    },
    /// Sent once when the run finishes.
    RunFinished {
        /// Number of requests that passed.
        passed: usize,
        /// Number of requests that failed (excludes skipped).
        failed: usize,
        /// Number of requests that were skipped due to a prior failure.
        skipped: usize,
        /// Sum of all request `duration_ms` values in milliseconds.
        total_duration_ms: u64,
        /// Mean request duration in milliseconds (0 if no requests ran).
        avg_response_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_started_serializes_with_type_tag() {
        let mut row = HashMap::new();
        row.insert("username".to_string(), "alice".to_string());
        let event = WsEvent::IterationStarted { iteration: 1, row };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"iteration_started""#));
        assert!(json.contains(r#""iteration":1"#));
        assert!(json.contains("username"));
        assert!(json.contains("alice"));
    }

    #[test]
    fn run_started_serializes_with_type_tag() {
        let event = WsEvent::RunStarted { total: 3 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"run_started""#));
        assert!(json.contains(r#""total":3"#));
    }

    #[test]
    fn request_completed_pass_serializes_correctly() {
        let event = WsEvent::RequestCompleted {
            name: "Get User".into(),
            method: "GET".into(),
            url: "https://api.example.com/users/1".into(),
            passed: true,
            status: Some(200),
            duration_ms: 45,
            failures: vec![],
            error: None,
            response_body: Some("{}".into()),
            response_headers: Some(HashMap::from([(
                "content-type".into(),
                "application/json".into(),
            )])),
            request_body: None,
            logs: vec![],
            passed_assertions: vec![],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"request_completed""#));
        assert!(json.contains(r#""passed":true"#));
        assert!(json.contains(r#""response_body":"{}""#));
        assert!(json.contains("content-type"));
    }

    #[test]
    fn request_completed_failure_includes_failures_array() {
        let event = WsEvent::RequestCompleted {
            name: "Login".into(),
            method: "POST".into(),
            url: "https://api.example.com/login".into(),
            passed: false,
            status: Some(401),
            duration_ms: 120,
            failures: vec!["status expected 200, got 401".into()],
            error: None,
            response_body: None,
            response_headers: None,
            request_body: None,
            logs: vec![],
            passed_assertions: vec![],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("status expected 200, got 401"));
    }

    #[test]
    fn run_finished_includes_timing_fields() {
        let event = WsEvent::RunFinished {
            passed: 2,
            failed: 1,
            skipped: 0,
            total_duration_ms: 500,
            avg_response_ms: 166,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"run_finished""#));
        assert!(json.contains(r#""total_duration_ms":500"#));
        assert!(json.contains(r#""avg_response_ms":166"#));
    }
}
