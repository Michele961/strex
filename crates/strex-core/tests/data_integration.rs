//! Integration tests for SP4 data-driven parsing and orchestration.

use strex_core::{parse_csv, parse_json, DataError};

// ── parse_csv ─────────────────────────────────────────────────────────────

#[test]
fn parse_csv_basic() {
    let content = "email,name,age\nalice@example.com,Alice,30\nbob@example.com,Bob,25\n";
    let rows = parse_csv(content).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["email"], "alice@example.com");
    assert_eq!(rows[0]["name"], "Alice");
    assert_eq!(rows[0]["age"], "30");
    assert_eq!(rows[1]["email"], "bob@example.com");
    assert_eq!(rows[1]["name"], "Bob");
}

#[test]
fn parse_csv_empty_rows() {
    // Header only — no data rows.
    let content = "email,name\n";
    let rows = parse_csv(content).unwrap();
    assert!(rows.is_empty());
}

#[test]
fn parse_csv_malformed_returns_error() {
    // Row 2 has 3 fields but header has 2 — csv crate treats this as an error
    // when flexible: false (the default).
    let content = "a,b\n1,2,3\n";
    let result = parse_csv(content);
    assert!(
        matches!(result, Err(DataError::CsvParse(_))),
        "expected CsvParse, got: {result:?}"
    );
}

// ── parse_json ────────────────────────────────────────────────────────────

#[test]
fn parse_json_basic() {
    let content = r#"[{"email":"alice@example.com","score":42,"active":true}]"#;
    let rows = parse_json(content).unwrap();
    assert_eq!(rows.len(), 1);
    // String value used directly (no double-quoting)
    assert_eq!(rows[0]["email"], "alice@example.com");
    // Number coerced to string
    assert_eq!(rows[0]["score"], "42");
    // Bool coerced to string
    assert_eq!(rows[0]["active"], "true");
}

#[test]
fn parse_json_empty_array() {
    let rows = parse_json("[]").unwrap();
    assert!(rows.is_empty());
}

#[test]
fn parse_json_not_array() {
    let result = parse_json(r#"{"email":"a@b.com"}"#);
    assert!(
        matches!(result, Err(DataError::JsonNotArray)),
        "expected JsonNotArray, got: {result:?}"
    );
}

#[test]
fn parse_json_row_not_object() {
    let result = parse_json(r#"["not_an_object", {"key":"val"}]"#);
    assert!(
        matches!(result, Err(DataError::JsonRowNotObject { index: 0 })),
        "expected JsonRowNotObject{{index:0}}, got: {result:?}"
    );
}

// ── run_collection_with_data ──────────────────────────────────────────────

use std::collections::HashMap;
use strex_core::{run_collection_with_data, Collection, DataRunOpts, Request};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a minimal one-request Collection that GETs `url`.
fn simple_collection(url: &str) -> Collection {
    Collection {
        name: "test".to_string(),
        version: "1.0".to_string(),
        environment: HashMap::new(),
        variables: HashMap::new(),
        requests: vec![Request {
            name: "req".to_string(),
            method: "GET".to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
            body: None,
            pre_script: None,
            post_script: None,
            assertions: vec![],
            timeout: None,
        }],
    }
}

/// Build a one-request Collection whose assertion checks for HTTP `code`.
fn collection_with_status_assertion(url: &str, code: u64) -> Collection {
    use serde_yaml::Value as YamlValue;
    use std::collections::HashMap as HM;
    let mut assertion = HM::new();
    assertion.insert(
        "status".to_string(),
        YamlValue::Number(serde_yaml::Number::from(code)),
    );
    Collection {
        name: "test".to_string(),
        version: "1.0".to_string(),
        environment: HashMap::new(),
        variables: HashMap::new(),
        requests: vec![Request {
            name: "req".to_string(),
            method: "GET".to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
            body: None,
            pre_script: None,
            post_script: None,
            assertions: vec![assertion],
            timeout: None,
        }],
    }
}

#[tokio::test]
async fn run_with_data_invalid_concurrency() {
    let col = simple_collection("http://localhost:1");
    let opts = DataRunOpts {
        concurrency: 0,
        ..DataRunOpts::default()
    };
    let result = run_collection_with_data(col, vec![], opts).await;
    assert!(
        matches!(result, Err(strex_core::DataError::InvalidConcurrency)),
        "expected InvalidConcurrency, got: {result:?}"
    );
}

#[tokio::test]
async fn run_with_data_zero_rows() {
    let col = simple_collection("http://localhost:1");
    let result = run_collection_with_data(col, vec![], DataRunOpts::default())
        .await
        .unwrap();
    assert_eq!(result.iterations.len(), 0);
    assert_eq!(result.passed, 0);
    assert_eq!(result.failed, 0);
}

#[tokio::test]
async fn run_with_data_sequential() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&format!("{}/", server.uri()));
    let rows: Vec<strex_core::DataRow> = vec![
        [("user".to_string(), "alice".to_string())].into(),
        [("user".to_string(), "bob".to_string())].into(),
        [("user".to_string(), "carol".to_string())].into(),
    ];
    let opts = DataRunOpts {
        concurrency: 1,
        ..DataRunOpts::default()
    };
    let result = run_collection_with_data(col, rows, opts).await.unwrap();

    assert_eq!(result.iterations.len(), 3);
    // Results are in row order regardless of completion order.
    assert_eq!(result.iterations[0].row_index, 0);
    assert_eq!(result.iterations[1].row_index, 1);
    assert_eq!(result.iterations[2].row_index, 2);
    assert_eq!(result.passed, 3);
    assert_eq!(result.failed, 0);
    // Each row's data was injected into the context.
    assert_eq!(result.iterations[0].row["user"], "alice");
    assert_eq!(result.iterations[1].row["user"], "bob");
    assert_eq!(result.iterations[2].row["user"], "carol");
}

#[tokio::test]
async fn run_with_data_concurrent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&format!("{}/", server.uri()));
    let rows: Vec<strex_core::DataRow> = vec![
        [("id".to_string(), "1".to_string())].into(),
        [("id".to_string(), "2".to_string())].into(),
        [("id".to_string(), "3".to_string())].into(),
    ];
    let opts = DataRunOpts {
        concurrency: 3,
        ..DataRunOpts::default()
    };
    let result = run_collection_with_data(col, rows, opts).await.unwrap();

    assert_eq!(result.iterations.len(), 3);
    // Results must be returned in row order.
    assert_eq!(result.iterations[0].row_index, 0);
    assert_eq!(result.iterations[1].row_index, 1);
    assert_eq!(result.iterations[2].row_index, 2);
    assert_eq!(result.passed, 3);
    assert_eq!(result.failed, 0);
}

#[tokio::test]
async fn run_with_data_fail_fast() {
    let server = MockServer::start().await;
    // Return 500 to force assertion failure when we assert status == 200.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let url = format!("{}/", server.uri());
    // Collection asserts status == 200; server returns 500 → row 0 fails.
    let col = collection_with_status_assertion(&url, 200);
    let rows: Vec<strex_core::DataRow> = vec![
        [("n".to_string(), "1".to_string())].into(),
        [("n".to_string(), "2".to_string())].into(),
        [("n".to_string(), "3".to_string())].into(),
    ];
    let opts = DataRunOpts {
        concurrency: 1,
        fail_fast: true,
        ..DataRunOpts::default()
    };
    let result = run_collection_with_data(col, rows, opts).await.unwrap();

    // With concurrency=1 and fail_fast=true, row 0 fails and rows 1+2 are not launched.
    assert_eq!(result.iterations.len(), 1, "only row 0 should have run");
    assert_eq!(result.failed, 1);
    assert_eq!(result.passed, 0);
}
