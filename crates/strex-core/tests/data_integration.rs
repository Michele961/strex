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
