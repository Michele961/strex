use super::*;
use std::io::Write as IoWrite;
use tempfile::NamedTempFile;

fn make_collection_file(yaml: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(yaml.as_bytes()).unwrap();
    f
}

#[tokio::test]
async fn valid_collection_with_no_variables_exits_zero() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
requests:
  - name: Get
    method: GET
    url: "https://example.com/api"
"#,
    );
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn valid_collection_with_declared_variable_exits_zero() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
variables:
  token: "abc123"
requests:
  - name: Get
    method: GET
    url: "https://example.com/{{token}}"
"#,
    );
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn unresolved_variable_in_url_exits_nonzero() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
requests:
  - name: Get
    method: GET
    url: "https://{{baseUrl}}/api"
"#,
    );
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unresolved variable"), "msg: {}", msg);
    assert!(msg.contains("baseUrl"), "msg: {}", msg);
}

#[tokio::test]
async fn unresolved_variable_in_header_exits_nonzero() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
requests:
  - name: Get
    method: GET
    url: "https://example.com/api"
    headers:
      Authorization: "Bearer {{authToken}}"
"#,
    );
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unresolved variable"), "msg: {}", msg);
    assert!(msg.contains("authToken"), "msg: {}", msg);
}

#[tokio::test]
async fn unresolved_variable_in_body_exits_nonzero() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
requests:
  - name: Post
    method: POST
    url: "https://example.com/api"
    body:
      type: json
      content:
        id: "{{userId}}"
"#,
    );
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unresolved variable"), "msg: {}", msg);
    assert!(msg.contains("userId"), "msg: {}", msg);
}

#[tokio::test]
async fn declared_variables_listed_in_error_message() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
variables:
  alpha: "a"
  beta: "b"
requests:
  - name: Get
    method: GET
    url: "https://{{missing}}/api"
"#,
    );
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("alpha"), "msg: {}", msg);
    assert!(msg.contains("beta"), "msg: {}", msg);
    assert!(msg.contains("missing"), "msg: {}", msg);
}

#[tokio::test]
async fn multiple_requests_first_unresolved_reported() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
requests:
  - name: First
    method: GET
    url: "https://example.com/ok"
  - name: Second
    method: GET
    url: "https://example.com/{{secretVar}}"
"#,
    );
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("secretVar"), "msg: {}", msg);
    assert!(msg.contains("requests[1]"), "msg: {}", msg);
}

#[tokio::test]
async fn collection_parse_error_returns_error() {
    let f = make_collection_file("this is: not: valid: yaml: collection: format:\n  - bad");
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn valid_collection_reports_request_count() {
    let f = make_collection_file(
        r#"
name: Test
version: "1.0"
requests:
  - name: First
    method: GET
    url: "https://example.com/one"
  - name: Second
    method: GET
    url: "https://example.com/two"
"#,
    );
    // Capture that execute returns Ok(0) for a 2-request collection;
    // the "2 requests" text goes to stdout which we cannot easily capture
    // here, but we verify the function succeeds (it only prints on success).
    let result = execute(ValidateArgs {
        collection: f.path().to_path_buf(),
    })
    .await;
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), 0);
}
