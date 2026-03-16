use super::*;
use crate::output::{RunOutcome, RunResult};
use std::collections::HashMap;
use strex_core::{
    AssertionFailure, AssertionType, Collection, CollectionResult, DataRunResult, IterationResult,
    Request, RequestError, RequestOutcome, RequestResult,
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

#[test]
fn escape_xml_covers_all_five_chars() {
    // escape_xml is private; test via a round-trip through the formatter
    // or just test the known escaping behavior via a request name
    let col = make_collection(r#"a&b<c>d"e'f"#);
    let col_result = CollectionResult {
        request_results: vec![RequestResult {
            name: r#"a&b<c>d"e'f"#.to_string(),
            outcome: RequestOutcome::Passed,
            duration_ms: 1,
            response: None,
        }],
    };
    let result = make_result(col, RunOutcome::Single(col_result));
    let mut out = Vec::new();
    print(&result, &mut out).unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("a&amp;b&lt;c&gt;d&quot;e&apos;f"), "Got: {}", s);
}
