use std::collections::HashMap;

use anyhow::bail;

use crate::cli::ValidateArgs;

/// Execute the `validate` subcommand.
///
/// Parses the collection at `args.collection`, walks every interpolatable
/// string for `{{placeholder}}` references, and cross-references them
/// against `collection.variables` keys.
///
/// Returns `Ok(0)` when the collection is valid; bails with a descriptive
/// error (→ exit 2 via `main`) when an unresolved variable is found.
pub async fn execute(args: ValidateArgs) -> anyhow::Result<i32> {
    let collection = strex_core::parse_collection(&args.collection)?;

    let declared: std::collections::HashSet<&str> =
        collection.variables.keys().map(String::as_str).collect();

    // Walk requests in order: url → headers (sorted) → body → assertions.
    for (idx, request) in collection.requests.iter().enumerate() {
        // --- url ---
        for placeholder in extract_placeholders(&request.url) {
            if !declared.contains(placeholder.as_str()) {
                let declared_list = sorted_declared_list(&collection.variables);
                bail!(
                    "error: unresolved variable `{}` in requests[{}].url\n  declared variables: {}",
                    placeholder,
                    idx,
                    declared_list,
                );
            }
        }

        // --- headers (sorted by key for determinism) ---
        let mut header_keys: Vec<&str> = request.headers.keys().map(String::as_str).collect();
        header_keys.sort_unstable();
        for key in header_keys {
            let value = &request.headers[key];
            for placeholder in extract_placeholders(value) {
                if !declared.contains(placeholder.as_str()) {
                    let declared_list = sorted_declared_list(&collection.variables);
                    bail!(
                        "error: unresolved variable `{}` in requests[{}].headers\n  declared variables: {}",
                        placeholder,
                        idx,
                        declared_list,
                    );
                }
            }
        }

        // --- body ---
        if let Some(body) = &request.body {
            let mut body_placeholders = Vec::new();
            walk_yaml_value(&body.content, &mut body_placeholders);
            for placeholder in body_placeholders {
                if !declared.contains(placeholder.as_str()) {
                    let declared_list = sorted_declared_list(&collection.variables);
                    bail!(
                        "error: unresolved variable `{}` in requests[{}].body\n  declared variables: {}",
                        placeholder,
                        idx,
                        declared_list,
                    );
                }
            }
        }

        // --- assertions ---
        for assertion_map in &request.assertions {
            let mut assertion_placeholders = Vec::new();
            for value in assertion_map.values() {
                walk_yaml_value(value, &mut assertion_placeholders);
            }
            for placeholder in assertion_placeholders {
                if !declared.contains(placeholder.as_str()) {
                    let declared_list = sorted_declared_list(&collection.variables);
                    bail!(
                        "error: unresolved variable `{}` in requests[{}].assertions\n  declared variables: {}",
                        placeholder,
                        idx,
                        declared_list,
                    );
                }
            }
        }
    }

    let filename = args
        .collection
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("collection");

    println!(
        "valid: {} ({} requests, 0 unresolved variables)",
        filename,
        collection.requests.len(),
    );

    Ok(0)
}

/// Extract all `{{placeholder}}` names from a string.
fn extract_placeholders(s: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut remaining = s;
    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("}}") {
            let name = remaining[..end].trim().to_string();
            if !name.is_empty() {
                placeholders.push(name);
            }
            remaining = &remaining[end + 2..];
        } else {
            break;
        }
    }
    placeholders
}

/// Recursively walk a `serde_yaml::Value`, collecting placeholders from String leaves.
fn walk_yaml_value(value: &serde_yaml::Value, placeholders: &mut Vec<String>) {
    match value {
        serde_yaml::Value::String(s) => {
            placeholders.extend(extract_placeholders(s));
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                walk_yaml_value(item, placeholders);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (_, v) in map {
                walk_yaml_value(v, placeholders);
            }
        }
        // Null, Bool, Number, Tagged — no string content to scan.
        _ => {}
    }
}

/// Build the sorted, comma-joined declared-variables string for error messages.
fn sorted_declared_list(variables: &HashMap<String, Option<String>>) -> String {
    if variables.is_empty() {
        return "(none)".to_string();
    }
    let mut keys: Vec<&str> = variables.keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys.join(", ")
}

#[cfg(test)]
mod tests {
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
}
