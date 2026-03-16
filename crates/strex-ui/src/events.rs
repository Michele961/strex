//! WebSocket event types streamed from the server to the browser.

use serde::Serialize;

/// Events streamed from the server to the browser over WebSocket.
///
/// Tagged with `"type"` field in JSON (e.g. `{"type":"run_started","total":3}`).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WsEvent {
    /// Sent once when the run begins.
    RunStarted {
        /// Total number of requests that will be executed.
        total: usize,
    },
    /// Sent after each request completes (pass or fail).
    RequestCompleted {
        /// Request name from the collection YAML.
        name: String,
        /// HTTP method (GET, POST, etc.).
        method: String,
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
    },
    /// Sent once when the run finishes.
    RunFinished {
        /// Number of requests that passed.
        passed: usize,
        /// Number of requests that failed.
        failed: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

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
            passed: true,
            status: Some(200),
            duration_ms: 45,
            failures: vec![],
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"request_completed""#));
        assert!(json.contains(r#""passed":true"#));
        assert!(json.contains(r#""status":200"#));
    }

    #[test]
    fn request_completed_failure_includes_failures_array() {
        let event = WsEvent::RequestCompleted {
            name: "Login".into(),
            method: "POST".into(),
            passed: false,
            status: Some(401),
            duration_ms: 120,
            failures: vec!["status expected 200, got 401".into()],
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("status expected 200, got 401"));
    }

    #[test]
    fn run_finished_serializes_with_type_tag() {
        let event = WsEvent::RunFinished {
            passed: 2,
            failed: 1,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"run_finished""#));
        assert!(json.contains(r#""passed":2"#));
        assert!(json.contains(r#""failed":1"#));
    }
}
