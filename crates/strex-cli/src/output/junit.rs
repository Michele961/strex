use std::io::Write;

use strex_core::{CollectionResult, RequestOutcome};

use crate::output::{format_failure, RunOutcome, RunResult};

/// Escape XML special characters in a string.
///
/// Replaces `&`, `<`, `>`, `"`, and `'` with their XML entity equivalents.
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

/// Write a single `<testsuite>` element to `writer`.
///
/// Each `<testcase>` corresponds to one [`RequestResult`] in `col_result`.
/// - Passed requests produce a self-closing `<testcase name="..."/>`.
/// - Assertion failures produce a `<testcase>` with a `<failure message="..."/>` child.
/// - Errors produce a `<testcase>` with an `<error message="..."/>` child.
fn write_testsuite(
    writer: &mut impl Write,
    suite_name: &str,
    col_result: &CollectionResult,
) -> anyhow::Result<()> {
    let tests = col_result.request_results.len();
    let failures = col_result
        .request_results
        .iter()
        .filter(|r| matches!(r.outcome, RequestOutcome::AssertionsFailed(_)))
        .count();
    let errors = col_result
        .request_results
        .iter()
        .filter(|r| matches!(r.outcome, RequestOutcome::Error(_)))
        .count();

    writeln!(
        writer,
        r#"  <testsuite name="{}" tests="{}" failures="{}" errors="{}">"#,
        escape_xml(suite_name),
        tests,
        failures,
        errors,
    )?;

    for req in &col_result.request_results {
        let escaped_name = escape_xml(&req.name);
        match &req.outcome {
            RequestOutcome::Passed => {
                writeln!(writer, r#"    <testcase name="{}"/>"#, escaped_name)?;
            }
            RequestOutcome::AssertionsFailed(assertion_failures) => {
                // Use the first failure's message; subsequent failures are omitted per spec.
                let msg = assertion_failures
                    .first()
                    .map(|f| escape_xml(&format_failure(f)))
                    .unwrap_or_default();
                writeln!(writer, r#"    <testcase name="{}">"#, escaped_name)?;
                writeln!(writer, r#"      <failure message="{}"/>"#, msg)?;
                writeln!(writer, r#"    </testcase>"#)?;
            }
            RequestOutcome::Error(e) => {
                let msg = escape_xml(&e.to_string());
                writeln!(writer, r#"    <testcase name="{}">"#, escaped_name)?;
                writeln!(writer, r#"      <error message="{}"/>"#, msg)?;
                writeln!(writer, r#"    </testcase>"#)?;
            }
        }
    }

    writeln!(writer, r#"  </testsuite>"#)?;
    Ok(())
}

/// Write a JUnit XML report to `writer`.
///
/// Single runs produce one `<testsuite>` named after the collection.
/// Data-driven runs produce one `<testsuite>` per iteration, named
/// `"<collection-name> row <row_index>"`.
///
/// The output starts with an XML declaration and wraps all suites in a
/// `<testsuites>` root element.
#[allow(dead_code)]
pub fn print(result: &RunResult, writer: &mut impl Write) -> anyhow::Result<()> {
    writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(writer, r#"<testsuites>"#)?;

    match &result.outcome {
        RunOutcome::Single(col_result) => {
            write_testsuite(writer, &result.collection.name, col_result)?;
        }
        RunOutcome::DataDriven(data_result) => {
            for iter in &data_result.iterations {
                let suite_name = format!("{} row {}", result.collection.name, iter.row_index);
                write_testsuite(writer, &suite_name, &iter.collection_result)?;
            }
        }
    }

    writeln!(writer, r#"</testsuites>"#)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{RunOutcome, RunResult};
    use std::collections::HashMap;
    use strex_core::{
        AssertionFailure, AssertionType, Collection, CollectionResult, DataRunResult,
        IterationResult, Request, RequestError, RequestOutcome, RequestResult,
    };

    fn make_collection(req_name: &str) -> Collection {
        Collection {
            name: "My Collection".to_string(),
            version: "1.0".to_string(),
            environment: HashMap::new(),
            variables: HashMap::new(),
            requests: vec![Request {
                name: req_name.to_string(),
                method: "GET".to_string(),
                url: "https://example.com".to_string(),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: None,
                assertions: vec![],
                timeout: None,
            }],
        }
    }

    fn make_result(col: Collection, outcome: RunOutcome) -> RunResult {
        RunResult {
            collection: col,
            outcome,
        }
    }

    #[test]
    fn passed_run_produces_testsuite_with_zero_failures() {
        let col = make_collection("Create User");
        let col_result = CollectionResult {
            request_results: vec![RequestResult {
                name: "Create User".to_string(),
                outcome: RequestOutcome::Passed,
                duration_ms: 10,
                response: None,
            }],
        };
        let result = make_result(col, RunOutcome::Single(col_result));
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(
            out.contains(r#"failures="0""#),
            "expected failures=0 in:\n{out}"
        );
        assert!(
            out.contains(r#"errors="0""#),
            "expected errors=0 in:\n{out}"
        );
        assert!(
            out.contains(r#"<testcase name="#),
            "expected testcase in:\n{out}"
        );
        assert!(!out.contains("<failure"), "unexpected <failure in:\n{out}");
        assert!(!out.contains("<error"), "unexpected <error in:\n{out}");
    }

    #[test]
    fn failed_assertion_produces_failure_element_and_count() {
        let col = make_collection("Authenticate");
        let col_result = CollectionResult {
            request_results: vec![RequestResult {
                name: "Authenticate".to_string(),
                outcome: RequestOutcome::AssertionsFailed(vec![AssertionFailure {
                    assertion_type: AssertionType::Status,
                    expected: "200".to_string(),
                    actual: "401".to_string(),
                }]),
                duration_ms: 5,
                response: None,
            }],
        };
        let result = make_result(col, RunOutcome::Single(col_result));
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(
            out.contains(r#"failures="1""#),
            "expected failures=1 in:\n{out}"
        );
        assert!(
            out.contains(r#"<failure message="#),
            "expected <failure message in:\n{out}"
        );
    }

    #[test]
    fn special_chars_in_names_are_xml_escaped() {
        let col = Collection {
            name: "My Collection".to_string(),
            version: "1.0".to_string(),
            environment: HashMap::new(),
            variables: HashMap::new(),
            requests: vec![Request {
                name: "Request <&> Test".to_string(),
                method: "GET".to_string(),
                url: "https://example.com".to_string(),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: None,
                assertions: vec![],
                timeout: None,
            }],
        };
        let col_result = CollectionResult {
            request_results: vec![RequestResult {
                name: "Request <&> Test".to_string(),
                outcome: RequestOutcome::Passed,
                duration_ms: 5,
                response: None,
            }],
        };
        let result = make_result(col, RunOutcome::Single(col_result));
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(
            out.contains("Request &lt;&amp;&gt; Test"),
            "expected escaped chars in:\n{out}"
        );
        assert!(
            !out.contains("Request <&>"),
            "raw special chars must not appear in:\n{out}"
        );
    }

    #[test]
    fn xml_has_declaration_and_testsuites_root() {
        let col = make_collection("Ping");
        let col_result = CollectionResult {
            request_results: vec![RequestResult {
                name: "Ping".to_string(),
                outcome: RequestOutcome::Passed,
                duration_ms: 1,
                response: None,
            }],
        };
        let result = make_result(col, RunOutcome::Single(col_result));
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(
            out.starts_with("<?xml"),
            "output must start with <?xml, got:\n{out}"
        );
        assert!(
            out.contains("<testsuites>"),
            "expected <testsuites> in:\n{out}"
        );
    }

    #[test]
    fn error_request_produces_error_element_and_count() {
        let col = make_collection("Create User");
        let col_result = CollectionResult {
            request_results: vec![RequestResult {
                name: "Create User".to_string(),
                outcome: RequestOutcome::Error(RequestError::Network {
                    cause: "connection reset".to_string(),
                }),
                duration_ms: 0,
                response: None,
            }],
        };
        let result = make_result(col, RunOutcome::Single(col_result));
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(
            out.contains(r#"errors="1""#),
            "expected errors=1 in:\n{out}"
        );
        assert!(
            out.contains(r#"<error message="#),
            "expected <error message in:\n{out}"
        );
    }

    #[test]
    fn data_driven_produces_one_testsuite_per_iteration() {
        let col = make_collection("Create User");
        let iter0 = IterationResult {
            row_index: 0,
            row: [("email".to_string(), "alice@example.com".to_string())].into(),
            collection_result: CollectionResult {
                request_results: vec![RequestResult {
                    name: "Create User".to_string(),
                    outcome: RequestOutcome::Passed,
                    duration_ms: 5,
                    response: None,
                }],
            },
        };
        let iter1 = IterationResult {
            row_index: 1,
            row: [("email".to_string(), "bob@example.com".to_string())].into(),
            collection_result: CollectionResult {
                request_results: vec![RequestResult {
                    name: "Create User".to_string(),
                    outcome: RequestOutcome::AssertionsFailed(vec![AssertionFailure {
                        assertion_type: AssertionType::Status,
                        expected: "200".to_string(),
                        actual: "409".to_string(),
                    }]),
                    duration_ms: 5,
                    response: None,
                }],
            },
        };
        let result = make_result(
            col,
            RunOutcome::DataDriven(DataRunResult {
                iterations: vec![iter0, iter1],
                passed: 1,
                failed: 1,
            }),
        );
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(
            out.contains("My Collection row 0"),
            "expected row 0 suite in:\n{out}"
        );
        assert!(
            out.contains("My Collection row 1"),
            "expected row 1 suite in:\n{out}"
        );
        assert!(
            out.contains(r#"failures="1""#),
            "expected failures=1 in:\n{out}"
        );
    }
}
