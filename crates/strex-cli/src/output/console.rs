use std::io::Write;

use strex_core::RequestOutcome;

use crate::output::{format_failure, RunOutcome, RunResult};

/// Write a human-readable console report to `writer`.
///
/// Zips `collection.requests` with `request_results` — both are in declaration order,
/// so index `i` in `request_results` corresponds to `collection.requests[i]`.
pub fn print(result: &RunResult, writer: &mut impl Write) -> anyhow::Result<()> {
    let method_width = result
        .collection
        .requests
        .iter()
        .map(|r| r.method.len())
        .max()
        .unwrap_or(0);

    match &result.outcome {
        RunOutcome::Single(col_result) => {
            for (request, req_result) in result
                .collection
                .requests
                .iter()
                .zip(col_result.request_results.iter())
            {
                print_request(
                    writer,
                    &request.method,
                    &req_result.name,
                    &req_result.outcome,
                    method_width,
                )?;
            }
            let total = col_result.request_results.len();
            let passed = col_result
                .request_results
                .iter()
                .filter(|r| matches!(r.outcome, RequestOutcome::Passed))
                .count();
            let skipped = col_result
                .request_results
                .iter()
                .filter(|r| matches!(r.outcome, RequestOutcome::Skipped))
                .count();
            let failed = total - passed - skipped;
            if skipped > 0 {
                writeln!(
                    writer,
                    "\n{total} requests · {passed} passed · {failed} failed · {skipped} skipped"
                )?;
            } else {
                writeln!(
                    writer,
                    "\n{total} requests · {passed} passed · {failed} failed"
                )?;
            }
        }
        RunOutcome::DataDriven(data_result) => {
            for iter_result in &data_result.iterations {
                // Sort keys for deterministic output order.
                let mut pairs: Vec<(&String, &String)> = iter_result.row.iter().collect();
                pairs.sort_by_key(|(k, _)| k.as_str());
                let row_summary: String = pairs
                    .iter()
                    .take(3)
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let ellipsis = if iter_result.row.len() > 3 { "…" } else { "" };
                writeln!(
                    writer,
                    "── Row {} ({}{}) {}",
                    iter_result.row_index + 1,
                    row_summary,
                    ellipsis,
                    "─".repeat(20),
                )?;
                for (request, req_result) in result
                    .collection
                    .requests
                    .iter()
                    .zip(iter_result.collection_result.request_results.iter())
                {
                    print_request(
                        writer,
                        &request.method,
                        &req_result.name,
                        &req_result.outcome,
                        method_width,
                    )?;
                }
            }
            writeln!(
                writer,
                "\n{} iterations · {} passed · {} failed",
                data_result.iterations.len(),
                data_result.passed,
                data_result.failed,
            )?;
        }
    }
    Ok(())
}

fn print_request(
    writer: &mut impl Write,
    method: &str,
    name: &str,
    outcome: &RequestOutcome,
    method_width: usize,
) -> anyhow::Result<()> {
    let symbol = match outcome {
        RequestOutcome::Passed => "✓",
        RequestOutcome::Skipped => "-",
        _ => "✗",
    };
    writeln!(writer, "{method:<method_width$}  {name}  {symbol}")?;
    match outcome {
        RequestOutcome::AssertionsFailed(failures) => {
            for f in failures {
                writeln!(writer, "    assertion failed: {}", format_failure(f))?;
            }
        }
        RequestOutcome::Error(e) => {
            writeln!(writer, "    error: {e}")?;
        }
        RequestOutcome::Passed | RequestOutcome::Skipped => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{RunOutcome, RunResult};
    use std::collections::HashMap;
    use strex_core::{
        AssertionFailure, AssertionType, Collection, CollectionResult, DataRunResult,
        IterationResult, Request, RequestOutcome, RequestResult,
    };

    fn make_collection(method: &str, req_name: &str) -> Collection {
        Collection {
            name: "Suite".to_string(),
            version: "1.0".to_string(),
            environment: HashMap::new(),
            variables: HashMap::new(),
            requests: vec![Request {
                name: req_name.to_string(),
                method: method.to_string(),
                url: "https://example.com".to_string(),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: None,
                assertions: vec![],
                timeout: None,
                on_failure: None,
            }],
        }
    }

    #[test]
    fn passed_request_shows_checkmark_and_summary() {
        let result = RunResult {
            collection: make_collection("GET", "Ping"),
            outcome: RunOutcome::Single(CollectionResult {
                request_results: vec![RequestResult {
                    name: "Ping".to_string(),
                    outcome: RequestOutcome::Passed,
                    duration_ms: 10,
                    response: None,
                    logs: vec![],
                    passed_assertions: vec![],
                }],
            }),
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains('✓'), "expected ✓ in:\n{out}");
        assert!(out.contains("1 passed"), "expected '1 passed' in:\n{out}");
        assert!(out.contains("0 failed"), "expected '0 failed' in:\n{out}");
    }

    #[test]
    fn failed_assertion_shows_cross_and_failure_message() {
        let result = RunResult {
            collection: make_collection("POST", "Login"),
            outcome: RunOutcome::Single(CollectionResult {
                request_results: vec![RequestResult {
                    name: "Login".to_string(),
                    outcome: RequestOutcome::AssertionsFailed(vec![AssertionFailure {
                        assertion_type: AssertionType::Status,
                        expected: "200".to_string(),
                        actual: "401".to_string(),
                        path: None,
                    }]),
                    duration_ms: 5,
                    response: None,
                    logs: vec![],
                    passed_assertions: vec![],
                }],
            }),
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains('✗'), "expected ✗ in:\n{out}");
        assert!(
            out.contains("status expected 200, got 401"),
            "expected failure message in:\n{out}"
        );
        assert!(out.contains("1 failed"), "expected '1 failed' in:\n{out}");
    }

    #[test]
    fn data_driven_shows_row_header_and_summary() {
        let iter = IterationResult {
            row_index: 0,
            row: [("email".to_string(), "alice@example.com".to_string())].into(),
            collection_result: CollectionResult {
                request_results: vec![RequestResult {
                    name: "Create".to_string(),
                    outcome: RequestOutcome::Passed,
                    duration_ms: 5,
                    response: None,
                    logs: vec![],
                    passed_assertions: vec![],
                }],
            },
        };
        let result = RunResult {
            collection: make_collection("POST", "Create"),
            outcome: RunOutcome::DataDriven(DataRunResult {
                iterations: vec![iter],
                passed: 1,
                failed: 0,
            }),
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Row 1"), "expected 'Row 1' in:\n{out}");
        assert!(
            out.contains("alice@example.com"),
            "expected row data in:\n{out}"
        );
        assert!(out.contains("1 passed"), "expected '1 passed' in:\n{out}");
    }
}
