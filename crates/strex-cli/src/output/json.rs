use std::io::Write;

use strex_core::{RequestOutcome, RequestResult};

use crate::output::{format_failure, RunOutcome, RunResult};

/// Write a JSON report to `writer`.
///
/// Single run: `{ "passed": N, "failed": N, "requests": [...] }`
/// Data-driven: `{ "passed": N, "failed": N, "iterations": [...] }`
pub fn print(result: &RunResult, writer: &mut impl Write) -> anyhow::Result<()> {
    let json = build_json(result);
    serde_json::to_writer(writer, &json)?;
    Ok(())
}

fn build_json(result: &RunResult) -> serde_json::Value {
    match &result.outcome {
        RunOutcome::Single(col_result) => {
            let passed = col_result
                .request_results
                .iter()
                .filter(|r| matches!(r.outcome, RequestOutcome::Passed))
                .count();
            let failed = col_result.request_results.len() - passed;
            let requests: Vec<serde_json::Value> = col_result
                .request_results
                .iter()
                .map(request_to_json)
                .collect();
            serde_json::json!({ "passed": passed, "failed": failed, "requests": requests })
        }
        RunOutcome::DataDriven(data_result) => {
            let iterations: Vec<serde_json::Value> = data_result
                .iterations
                .iter()
                .map(|iter_result| {
                    let iter_passed = iter_result.collection_result.passed();
                    let requests: Vec<serde_json::Value> = iter_result
                        .collection_result
                        .request_results
                        .iter()
                        .map(request_to_json)
                        .collect();
                    let row: serde_json::Map<String, serde_json::Value> = iter_result
                        .row
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    serde_json::json!({
                        "row_index": iter_result.row_index,
                        "row": serde_json::Value::Object(row),
                        "passed": iter_passed,
                        "requests": requests,
                    })
                })
                .collect();
            serde_json::json!({
                "passed": data_result.passed,
                "failed": data_result.failed,
                "iterations": iterations,
            })
        }
    }
}

fn request_to_json(req_result: &RequestResult) -> serde_json::Value {
    let status: serde_json::Value = req_result
        .response
        .as_ref()
        .map(|r| serde_json::Value::Number(r.status.into()))
        .unwrap_or(serde_json::Value::Null);

    match &req_result.outcome {
        RequestOutcome::Passed => serde_json::json!({
            "name": req_result.name,
            "passed": true,
            "status": status,
            "assertions": [],
        }),
        RequestOutcome::AssertionsFailed(failures) => {
            let assertions: Vec<serde_json::Value> = failures
                .iter()
                .map(|f| serde_json::json!({ "passed": false, "message": format_failure(f) }))
                .collect();
            serde_json::json!({
                "name": req_result.name,
                "passed": false,
                "status": status,
                "assertions": assertions,
            })
        }
        RequestOutcome::Error(e) => serde_json::json!({
            "name": req_result.name,
            "passed": false,
            "status": status,
            "error": e.to_string(),
            "assertions": [],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{RunOutcome, RunResult};
    use std::collections::HashMap;
    use strex_core::{
        AssertionFailure, AssertionType, Collection, CollectionResult, HttpResponse, Request,
        RequestError, RequestOutcome, RequestResult, RequestTiming,
    };

    fn make_collection() -> Collection {
        Collection {
            name: "API Tests".to_string(),
            version: "1.0".to_string(),
            environment: HashMap::new(),
            variables: HashMap::new(),
            requests: vec![Request {
                name: "Get User".to_string(),
                method: "GET".to_string(),
                url: "https://example.com/user".to_string(),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: None,
                assertions: vec![],
                timeout: None,
            }],
        }
    }

    #[test]
    fn single_passed_produces_valid_json_with_counts() {
        let result = RunResult {
            collection: make_collection(),
            outcome: RunOutcome::Single(CollectionResult {
                request_results: vec![RequestResult {
                    name: "Get User".to_string(),
                    outcome: RequestOutcome::Passed,
                    duration_ms: 10,
                    response: Some(HttpResponse {
                        status: 200,
                        headers: HashMap::new(),
                        body: String::new(),
                        timing: RequestTiming::default(),
                    }),
                }],
            }),
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["passed"], 1);
        assert_eq!(v["failed"], 0);
        assert_eq!(v["requests"][0]["name"], "Get User");
        assert_eq!(v["requests"][0]["passed"], true);
        assert_eq!(v["requests"][0]["status"], 200);
    }

    #[test]
    fn failed_assertion_appears_in_assertions_array() {
        let result = RunResult {
            collection: make_collection(),
            outcome: RunOutcome::Single(CollectionResult {
                request_results: vec![RequestResult {
                    name: "Get User".to_string(),
                    outcome: RequestOutcome::AssertionsFailed(vec![AssertionFailure {
                        assertion_type: AssertionType::Status,
                        expected: "200".to_string(),
                        actual: "404".to_string(),
                    }]),
                    duration_ms: 5,
                    response: None,
                }],
            }),
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["requests"][0]["passed"], false);
        assert!(v["requests"][0]["status"].is_null());
        assert_eq!(
            v["requests"][0]["assertions"][0]["message"],
            "status expected 200, got 404"
        );
    }

    #[test]
    fn error_outcome_has_error_field_and_null_status() {
        let result = RunResult {
            collection: make_collection(),
            outcome: RunOutcome::Single(CollectionResult {
                request_results: vec![RequestResult {
                    name: "Get User".to_string(),
                    outcome: RequestOutcome::Error(RequestError::ConnectionRefused {
                        url: "https://example.com".to_string(),
                    }),
                    duration_ms: 1,
                    response: None,
                }],
            }),
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["requests"][0]["passed"], false);
        assert!(v["requests"][0]["status"].is_null());
        assert!(v["requests"][0]["error"].is_string());
    }

    #[test]
    fn data_driven_produces_iterations_array() {
        use strex_core::{DataRunResult, IterationResult};
        let iter = IterationResult {
            row_index: 0,
            row: [("email".to_string(), "alice@example.com".to_string())].into(),
            collection_result: CollectionResult {
                request_results: vec![RequestResult {
                    name: "Create".to_string(),
                    outcome: RequestOutcome::Passed,
                    duration_ms: 5,
                    response: None,
                }],
            },
        };
        let result = RunResult {
            collection: make_collection(),
            outcome: RunOutcome::DataDriven(DataRunResult {
                iterations: vec![iter],
                passed: 1,
                failed: 0,
            }),
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["passed"], 1);
        assert_eq!(v["failed"], 0);
        assert!(v["iterations"].is_array());
        assert_eq!(v["iterations"][0]["row_index"], 0);
        assert_eq!(v["iterations"][0]["row"]["email"], "alice@example.com");
        assert_eq!(v["iterations"][0]["passed"], true);
    }
}
