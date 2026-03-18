use serde_yaml::Value;

use crate::{ImportError, ImportMode};

// ── Version detection ──────────────────────────────────────────────────────────

enum SpecVersion {
    OpenApi3,
    Swagger2,
}

fn detect_version(root: &Value) -> Result<SpecVersion, ImportError> {
    if root.get("openapi").is_some() {
        Ok(SpecVersion::OpenApi3)
    } else if root.get("swagger").is_some() {
        Ok(SpecVersion::Swagger2)
    } else {
        Err(ImportError::UnrecognisedFormat)
    }
}

// ── Base URL extraction ────────────────────────────────────────────────────────

const FALLBACK_BASE_URL: &str = "/";
const FALLBACK_COMMENT: &str = "  # TODO: replace baseUrl with your API base URL";

fn base_url_openapi3(root: &Value) -> String {
    root.get("servers")
        .and_then(|s| s.as_sequence())
        .and_then(|seq| seq.first())
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_else(|| FALLBACK_BASE_URL.into())
}

fn base_url_swagger2(root: &Value) -> String {
    let host = root.get("host").and_then(|h| h.as_str());
    let Some(host) = host else {
        return FALLBACK_BASE_URL.into();
    };
    let scheme = root
        .get("schemes")
        .and_then(|s| s.as_sequence())
        .and_then(|seq| seq.first())
        .and_then(|v| v.as_str())
        .unwrap_or("https");
    let base_path = root.get("basePath").and_then(|b| b.as_str()).unwrap_or("/");
    let base_path = base_path.trim_end_matches('/');
    format!("{scheme}://{host}{base_path}")
}

// ── Path parameter conversion ─────────────────────────────────────────────────

/// Convert OpenAPI path params like `{id}` to Strex `{{id}}`.
fn convert_path_params(path: &str) -> String {
    let mut result = String::new();
    for c in path.chars() {
        if c == '{' {
            result.push_str("{{");
        } else if c == '}' {
            result.push_str("}}");
        } else {
            result.push(c);
        }
    }
    result
}

// ── Request generation ─────────────────────────────────────────────────────────

struct RequestEntry {
    name: String,
    method: String,
    url: String,
    content_type: Option<String>,
    assertions: Vec<String>,
}

fn first_2xx_status(operation: &Value) -> u16 {
    if let Some(responses) = operation.get("responses").and_then(|r| r.as_mapping()) {
        for (key, _) in responses {
            if let Some(code_str) = key.as_str() {
                if let Ok(code) = code_str.parse::<u16>() {
                    if (200..300).contains(&code) {
                        return code;
                    }
                }
            }
        }
    }
    200
}

fn required_response_fields(operation: &Value) -> Vec<String> {
    // Try OpenAPI 3 response schema first
    let schema = operation
        .get("responses")
        .and_then(|r| r.as_mapping())
        .and_then(|m| {
            // Find the first 2xx response
            m.iter().find(|(k, _)| {
                k.as_str()
                    .and_then(|s| s.parse::<u16>().ok())
                    .map(|c| (200..300).contains(&c))
                    .unwrap_or(false)
            })
        })
        .and_then(|(_, v)| v.get("content"))
        .and_then(|c| c.get("application/json"))
        .and_then(|j| j.get("schema"));

    if let Some(schema) = schema {
        return extract_required_from_schema(schema);
    }
    Vec::new()
}

fn extract_required_from_schema(schema: &Value) -> Vec<String> {
    schema
        .get("required")
        .and_then(|r| r.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn content_type_from_request_body(operation: &Value) -> Option<String> {
    operation
        .get("requestBody")
        .and_then(|rb| rb.get("content"))
        .and_then(|c| c.as_mapping())
        .and_then(|m| m.keys().next())
        .and_then(|k| k.as_str())
        .map(String::from)
}

fn collect_operations(root: &Value, mode: &ImportMode) -> Vec<RequestEntry> {
    const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

    let Some(paths) = root.get("paths").and_then(|p| p.as_mapping()) else {
        return Vec::new();
    };

    let mut entries = Vec::new();

    for (path_key, path_item) in paths {
        let Some(path_str) = path_key.as_str() else {
            continue;
        };
        let strex_path = convert_path_params(path_str);

        for method in HTTP_METHODS {
            let Some(operation) = path_item.get(method) else {
                continue;
            };

            let name = operation
                .get("operationId")
                .and_then(|id| id.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("{} {path_str}", method.to_uppercase()));

            let url = format!("{{{{baseUrl}}}}{strex_path}");
            let content_type = content_type_from_request_body(operation);

            let assertions = match mode {
                ImportMode::Scaffold => Vec::new(),
                ImportMode::WithTests => {
                    let status = first_2xx_status(operation);
                    let mut a = vec![format!("      - status: {status}")];
                    for field in required_response_fields(operation) {
                        a.push(format!(
                            "      - jsonPath: \"$.{field}\"\n        exists: true"
                        ));
                    }
                    a
                }
            };

            entries.push(RequestEntry {
                name,
                method: method.to_uppercase(),
                url,
                content_type,
                assertions,
            });
        }
    }

    entries
}

// ── YAML output ────────────────────────────────────────────────────────────────

fn render_yaml(base_url: &str, entries: &[RequestEntry], needs_fallback_comment: bool) -> String {
    let base_url_line = if needs_fallback_comment {
        format!("  baseUrl: \"{base_url}\"{FALLBACK_COMMENT}")
    } else {
        format!("  baseUrl: \"{base_url}\"")
    };

    let mut requests_block = String::new();
    for entry in entries {
        requests_block.push_str(&format!(
            "  - name: \"{}\"\n    method: {}\n    url: \"{}\"\n",
            entry.name, entry.method, entry.url
        ));
        if let Some(ct) = &entry.content_type {
            requests_block.push_str(&format!("    headers:\n      Content-Type: \"{ct}\"\n"));
        }
        if !entry.assertions.is_empty() {
            requests_block.push_str("    assertions:\n");
            for a in &entry.assertions {
                requests_block.push_str(a);
                requests_block.push('\n');
            }
        }
    }

    format!(
        "name: \"Imported Collection\"\nversion: \"1.0\"\n\nenvironment:\n{base_url_line}\n\nrequests:\n{requests_block}"
    )
}

// ── Entry point ────────────────────────────────────────────────────────────────

/// Convert an OpenAPI 2.x (Swagger) or OpenAPI 3.x specification to a Strex collection YAML.
///
/// Supports both JSON and YAML input. Detects version automatically, extracts base URL and servers,
/// and generates request entries for each operation. When `mode` is `ImportMode::WithTests`,
/// adds assertions for HTTP status codes and required response fields.
pub(crate) fn convert(spec: &str, mode: ImportMode) -> Result<String, ImportError> {
    let root: Value =
        serde_yaml::from_str(spec).map_err(|e| ImportError::OpenApiParse(e.to_string()))?;

    let version = detect_version(&root)?;

    let (base, needs_comment) = match version {
        SpecVersion::OpenApi3 => {
            let url = base_url_openapi3(&root);
            let fallback = url == FALLBACK_BASE_URL;
            (url, fallback)
        }
        SpecVersion::Swagger2 => {
            let url = base_url_swagger2(&root);
            let fallback = url == FALLBACK_BASE_URL;
            (url, fallback)
        }
    };

    let entries = collect_operations(&root, &mode);
    Ok(render_yaml(&base, &entries, needs_comment))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImportMode;

    const OPENAPI3_SIMPLE: &str = r#"
openapi: "3.0.0"
info:
  title: "Test API"
  version: "1.0"
servers:
  - url: "https://api.example.com"
paths:
  /users:
    get:
      operationId: listUsers
      responses:
        "200":
          description: OK
    post:
      operationId: createUser
      requestBody:
        content:
          application/json:
            schema:
              type: object
      responses:
        "201":
          description: Created
          content:
            application/json:
              schema:
                required: ["id", "name"]
                properties:
                  id:
                    type: string
                  name:
                    type: string
  /users/{id}:
    get:
      operationId: getUser
      responses:
        "200":
          description: OK
"#;

    const SWAGGER2_SIMPLE: &str = r#"
swagger: "2.0"
info:
  title: "Test API"
  version: "1.0"
host: "api.example.com"
basePath: "/v1"
schemes:
  - https
paths:
  /users:
    get:
      operationId: listUsers
      responses:
        200:
          description: OK
"#;

    #[test]
    fn detects_openapi3() {
        let root: Value = serde_yaml::from_str(OPENAPI3_SIMPLE).unwrap();
        assert!(matches!(
            detect_version(&root).unwrap(),
            SpecVersion::OpenApi3
        ));
    }

    #[test]
    fn detects_swagger2() {
        let root: Value = serde_yaml::from_str(SWAGGER2_SIMPLE).unwrap();
        assert!(matches!(
            detect_version(&root).unwrap(),
            SpecVersion::Swagger2
        ));
    }

    #[test]
    fn unrecognised_format_returns_error() {
        let root: Value = serde_yaml::from_str("name: foo").unwrap();
        assert!(matches!(
            detect_version(&root),
            Err(ImportError::UnrecognisedFormat)
        ));
    }

    #[test]
    fn openapi3_base_url() {
        let root: Value = serde_yaml::from_str(OPENAPI3_SIMPLE).unwrap();
        assert_eq!(base_url_openapi3(&root), "https://api.example.com");
    }

    #[test]
    fn openapi3_no_servers_falls_back() {
        let spec = "openapi: \"3.0.0\"\ninfo:\n  title: t\n  version: v\npaths: {}";
        let root: Value = serde_yaml::from_str(spec).unwrap();
        assert_eq!(base_url_openapi3(&root), FALLBACK_BASE_URL);
    }

    #[test]
    fn swagger2_base_url() {
        let root: Value = serde_yaml::from_str(SWAGGER2_SIMPLE).unwrap();
        assert_eq!(base_url_swagger2(&root), "https://api.example.com/v1");
    }

    #[test]
    fn swagger2_no_host_falls_back() {
        let spec = "swagger: \"2.0\"\ninfo:\n  title: t\n  version: v\npaths: {}";
        let root: Value = serde_yaml::from_str(spec).unwrap();
        assert_eq!(base_url_swagger2(&root), FALLBACK_BASE_URL);
    }

    #[test]
    fn path_params_converted() {
        assert_eq!(
            convert_path_params("/users/{id}/posts/{postId}"),
            "/users/{{id}}/posts/{{postId}}"
        );
    }

    #[test]
    fn openapi3_scaffold_generates_requests() {
        let yaml = convert(OPENAPI3_SIMPLE, ImportMode::Scaffold).unwrap();
        assert!(yaml.contains("listUsers"));
        assert!(yaml.contains("createUser"));
        assert!(yaml.contains("getUser"));
        assert!(yaml.contains("{{baseUrl}}/users/{{id}}"));
        assert!(!yaml.contains("assertions:"));
    }

    #[test]
    fn openapi3_with_tests_adds_status_and_fields() {
        let yaml = convert(OPENAPI3_SIMPLE, ImportMode::WithTests).unwrap();
        // createUser has 201 response
        assert!(yaml.contains("status: 201"));
        // createUser response has required fields id, name
        assert!(yaml.contains("$.id"));
        assert!(yaml.contains("$.name"));
    }

    #[test]
    fn openapi3_with_tests_status_fallback_200() {
        // listUsers has 200 response
        let yaml = convert(OPENAPI3_SIMPLE, ImportMode::WithTests).unwrap();
        assert!(yaml.contains("status: 200"));
    }

    #[test]
    fn content_type_header_from_request_body() {
        let yaml = convert(OPENAPI3_SIMPLE, ImportMode::Scaffold).unwrap();
        assert!(yaml.contains("Content-Type: \"application/json\""));
    }

    #[test]
    fn swagger2_scaffold_works() {
        let yaml = convert(SWAGGER2_SIMPLE, ImportMode::Scaffold).unwrap();
        assert!(yaml.contains("listUsers"));
        assert!(yaml.contains("https://api.example.com/v1"));
    }

    #[test]
    fn invalid_yaml_returns_error() {
        let result = convert("{{invalid:", ImportMode::Scaffold);
        assert!(matches!(result, Err(ImportError::OpenApiParse(_))));
    }

    #[test]
    fn fallback_base_url_includes_comment() {
        let spec = "openapi: \"3.0.0\"\ninfo:\n  title: t\n  version: v\npaths:\n  /foo:\n    get:\n      responses:\n        \"200\":\n          description: OK";
        let yaml = convert(spec, ImportMode::Scaffold).unwrap();
        assert!(yaml.contains("# TODO"));
    }

    #[test]
    fn json_spec_is_accepted() {
        let json = r#"{"openapi":"3.0.0","info":{"title":"t","version":"v"},"servers":[{"url":"https://api.example.com"}],"paths":{}}"#;
        let yaml = convert(json, ImportMode::Scaffold).unwrap();
        assert!(yaml.contains("https://api.example.com"));
    }

    #[test]
    fn missing_operation_id_falls_back_to_method_path() {
        let spec = r#"
openapi: "3.0.0"
info:
  title: "Test API"
  version: "1.0"
servers:
  - url: "https://api.example.com"
paths:
  /items:
    get:
      responses:
        "200":
          description: OK
"#;
        let yaml = convert(spec, ImportMode::Scaffold).unwrap();
        assert!(yaml.contains("GET /items"));
    }
}
