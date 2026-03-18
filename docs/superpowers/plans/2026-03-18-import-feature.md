# Import Feature Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a UI modal that generates a Strex YAML collection from a curl command or OpenAPI/Swagger spec, previews it, and saves it to disk.

**Architecture:** A new `strex-import` crate provides pure conversion functions (`from_curl`, `from_openapi`) that return YAML strings. Two new Axum endpoints (`POST /api/import/generate` and `POST /api/import/save`) in `strex-ui` call into it and handle I/O. A new `ImportModal.svelte` Svelte component implements the 3-step UI (source → input → preview/save), wired into `ConfigPanel.svelte` via an Import button.

**Tech Stack:** Rust (thiserror, serde_yaml), Axum 0.7 (State extractor), reqwest (already a dep), Svelte 5, TypeScript.

---

## File Map

### New files
| File | Responsibility |
|------|---------------|
| `crates/strex-import/Cargo.toml` | Crate manifest |
| `crates/strex-import/src/lib.rs` | Public API: `ImportMode`, `from_curl`, `from_openapi` |
| `crates/strex-import/src/error.rs` | `ImportError` with `thiserror` |
| `crates/strex-import/src/curl.rs` | curl tokenizer, parser, scrubber, YAML serializer |
| `crates/strex-import/src/openapi.rs` | OpenAPI 2.x/3.x converter |
| `crates/strex-ui/src/import.rs` | Axum route handlers for `/api/import/*` |
| `crates/strex-ui/frontend/src/components/ImportModal.svelte` | 3-step import modal |

### Modified files
| File | Change |
|------|--------|
| `Cargo.toml` | Add `strex-import` to workspace members |
| `crates/strex-ui/Cargo.toml` | Add `strex-import` dependency |
| `crates/strex-ui/src/lib.rs` | Declare `mod import` |
| `crates/strex-ui/src/server.rs` | Add `AppState`, wire new routes |
| `crates/strex-ui/frontend/src/lib/api.ts` | Add `importGenerate`, `importSave` |
| `crates/strex-ui/frontend/src/components/ConfigPanel.svelte` | Add Import button + modal binding |

---

## Task 1: Scaffold `strex-import` crate

**Files:**
- Create: `crates/strex-import/Cargo.toml`
- Create: `crates/strex-import/src/error.rs`
- Create: `crates/strex-import/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Create `crates/strex-import/Cargo.toml`**

```toml
[package]
name = "strex-import"
version = "0.1.0"
edition = "2021"

[dependencies]
# Typed error definitions — required by CLAUDE.md for library crates
thiserror = "1"
# YAML/JSON parsing for OpenAPI specs and YAML output generation
serde_yaml = "0.9"
# JSON parsing for curl body scrubbing
serde_json = "1"
# Serde derive macros
serde = { version = "1", features = ["derive"] }
```

- [ ] **Step 2: Create `crates/strex-import/src/error.rs`**

```rust
/// Errors that can occur during collection import.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The curl command could not be parsed (e.g. unclosed quote, no URL).
    #[error("Failed to parse curl command: {0}")]
    CurlParse(String),
    /// The OpenAPI/Swagger spec could not be parsed as YAML or JSON.
    #[error("Failed to parse OpenAPI spec: {0}")]
    OpenApiParse(String),
    /// The spec does not contain a recognisable `openapi:` or `swagger:` key.
    #[error("Unrecognised spec format: expected 'openapi' or 'swagger' key at top level")]
    UnrecognisedFormat,
    /// A remote spec URL could not be fetched within the timeout.
    #[error("Fetch timed out")]
    FetchTimeout,
    /// YAML serialization of the generated collection failed.
    #[error("Failed to serialize collection: {0}")]
    Serialize(String),
}
```

- [ ] **Step 3: Create `crates/strex-import/src/lib.rs` (stub)**

```rust
#![deny(missing_docs)]
#![deny(clippy::all)]
//! Conversion utilities that generate Strex YAML collections from external sources.

mod curl;
mod error;
mod openapi;

pub use error::ImportError;

/// Whether to include assertions in the generated collection.
pub enum ImportMode {
    /// Generate method, URL, headers, and body only — no assertions.
    Scaffold,
    /// Generate requests plus basic assertions derived from the source.
    WithTests,
}

/// Parse a curl command and return a Strex YAML collection string.
///
/// Sensitive header and body values are replaced with `{{variable}}` placeholders.
pub fn from_curl(input: &str, mode: ImportMode) -> Result<String, ImportError> {
    curl::convert(input, mode)
}

/// Convert an OpenAPI/Swagger spec (as a YAML or JSON string) and return a Strex YAML collection string.
///
/// Accepts both YAML and JSON input — `serde_yaml::from_str` handles both formats
/// since JSON is a valid subset of YAML; no separate JSON branch is required.
pub fn from_openapi(spec: &str, mode: ImportMode) -> Result<String, ImportError> {
    openapi::convert(spec, mode)
}
```

- [ ] **Step 4: Add stub modules (so the crate compiles)**

Create `crates/strex-import/src/curl.rs`:
```rust
use crate::{ImportError, ImportMode};

pub(crate) fn convert(_input: &str, _mode: ImportMode) -> Result<String, ImportError> {
    Err(ImportError::CurlParse("not implemented".into()))
}
```

Create `crates/strex-import/src/openapi.rs`:
```rust
use crate::{ImportError, ImportMode};

pub(crate) fn convert(_spec: &str, _mode: ImportMode) -> Result<String, ImportError> {
    Err(ImportError::OpenApiParse("not implemented".into()))
}
```

- [ ] **Step 5: Register crate in workspace `Cargo.toml`**

In `/Users/michele/IdeaProjects/strex/Cargo.toml`, add `"crates/strex-import"` to `members`:

```toml
[workspace]
members = [
    "crates/strex-core",
    "crates/strex-script",
    "crates/strex-cli",
    "crates/strex-ui",
    "crates/strex-import",
]
```

- [ ] **Step 6: Verify the crate compiles**

```bash
cargo check -p strex-import
```
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/strex-import/ Cargo.toml
git commit -m "feat(import): scaffold strex-import crate with error types and stubs"
```

---

## Task 2: curl parser

**Files:**
- Modify: `crates/strex-import/src/curl.rs`

The parser tokenizes the curl command (respecting single/double quotes and `\` line continuations), then walks the token list extracting method, URL, headers, and body.

- [ ] **Step 1: Write failing tests**

Replace the contents of `crates/strex-import/src/curl.rs` with:

```rust
use crate::{ImportError, ImportMode};

// ── Tokenizer ─────────────────────────────────────────────────────────────────

/// Split a shell-style curl command into tokens, respecting quotes and `\` continuations.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '\\' if in_double => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '\\' if !in_single && !in_double => {
                // Line continuation: skip the following newline
                chars.next();
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

// ── Parsed curl ───────────────────────────────────────────────────────────────

struct ParsedCurl {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

/// Parse tokens into a `ParsedCurl`. Handles -X, -H/--header, -d/--data/--data-raw,
/// -u/--user, --url, and a bare URL positional argument.
fn parse_tokens(tokens: &[String]) -> Result<ParsedCurl, ImportError> {
    let mut method: Option<String> = None;
    let mut url: Option<String> = None;
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body: Option<String> = None;
    let mut user: Option<String> = None;

    let mut i = 1; // skip "curl"
    while i < tokens.len() {
        match tokens[i].as_str() {
            "-X" | "--request" => {
                i += 1;
                method = tokens.get(i).map(|s| s.to_uppercase());
            }
            "-H" | "--header" => {
                i += 1;
                if let Some(raw) = tokens.get(i) {
                    if let Some((name, value)) = raw.split_once(':') {
                        headers.push((name.trim().to_string(), value.trim().to_string()));
                    }
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" => {
                i += 1;
                body = tokens.get(i).cloned();
            }
            "-u" | "--user" => {
                i += 1;
                user = tokens.get(i).cloned();
            }
            "--url" => {
                i += 1;
                url = tokens.get(i).cloned();
            }
            // Ignore other flags (--compressed, --silent, etc.)
            flag if flag.starts_with('-') => {}
            // Bare positional argument — treat as URL if we haven't found one yet
            arg => {
                if url.is_none() {
                    url = Some(arg.to_string());
                }
            }
        }
        i += 1;
    }

    let url = url.ok_or_else(|| ImportError::CurlParse("no URL found".into()))?;

    // Infer method: POST if body present and no -X given
    let method = method.unwrap_or_else(|| {
        if body.is_some() { "POST".into() } else { "GET".into() }
    });

    // -u user:pass → Authorization: Basic {{credentials}}
    if let Some(_) = user {
        headers.push(("Authorization".into(), "Basic {{credentials}}".into()));
    }

    Ok(ParsedCurl { method, url, headers, body })
}

// ── Sensitive value scrubbing ─────────────────────────────────────────────────

/// Headers whose values are replaced with a `{{placeholder}}`.
/// This list is exhaustive for MVP.
const SENSITIVE_HEADERS: &[(&str, &str)] = &[
    ("authorization", "{{authorization}}"),
    ("x-api-key", "{{api_key}}"),
    ("x-auth-token", "{{auth_token}}"),
    ("cookie", "{{cookie}}"),
];

/// JSON body field names whose values are replaced with `{{field_name}}`.
/// This list is exhaustive for MVP.
const SENSITIVE_BODY_FIELDS: &[&str] = &["password", "secret", "token", "api_key"];

fn scrub_headers(headers: Vec<(String, String)>) -> Vec<(String, String)> {
    headers
        .into_iter()
        .map(|(name, value)| {
            let lower = name.to_lowercase();
            // Skip if already a placeholder (e.g. from -u processing)
            if value.starts_with("{{") {
                return (name, value);
            }
            for (sensitive, placeholder) in SENSITIVE_HEADERS {
                if lower == *sensitive {
                    return (name, placeholder.to_string());
                }
            }
            (name, value)
        })
        .collect()
}

fn scrub_body(body: &str) -> String {
    // Try to parse as JSON; if not JSON, return as-is
    let Ok(mut val) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };
    if let Some(obj) = val.as_object_mut() {
        for field in SENSITIVE_BODY_FIELDS {
            if obj.contains_key(*field) {
                obj.insert(field.to_string(), serde_json::Value::String(format!("{{{{{field}}}}}")));
            }
        }
    }
    serde_json::to_string(&val).unwrap_or_else(|_| body.to_string())
}

// ── URL decomposition ─────────────────────────────────────────────────────────

fn base_url(url: &str) -> String {
    // Extract scheme://host (strip path/query)
    if let Some(after_scheme) = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")) {
        let scheme = if url.starts_with("https") { "https" } else { "http" };
        let host = after_scheme.split('/').next().unwrap_or(after_scheme);
        format!("{scheme}://{host}")
    } else {
        url.to_string()
    }
}

fn url_path(url: &str) -> String {
    // Extract /path (without query string)
    if let Some(after_scheme) = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")) {
        let rest = after_scheme.splitn(2, '/').nth(1).unwrap_or("");
        let path = rest.split('?').next().unwrap_or(rest);
        format!("/{path}")
    } else {
        "/".into()
    }
}

fn request_name(method: &str, url: &str) -> String {
    format!("{} {}", method, url_path(url))
}

// ── YAML output ───────────────────────────────────────────────────────────────

pub(crate) fn convert(input: &str, mode: ImportMode) -> Result<String, ImportError> {
    let tokens = tokenize(input);
    if tokens.is_empty() || tokens[0].to_lowercase() != "curl" {
        return Err(ImportError::CurlParse("input must start with 'curl'".into()));
    }

    let parsed = parse_tokens(&tokens)?;
    let headers = scrub_headers(parsed.headers);
    let base = base_url(&parsed.url);
    let path = url_path(&parsed.url);
    let name = request_name(&parsed.method, &parsed.url);

    // Build header YAML lines
    let mut header_lines = String::new();
    for (k, v) in &headers {
        header_lines.push_str(&format!("      {k}: \"{v}\"\n"));
    }

    // Build body YAML section
    let body_section = if let Some(raw_body) = &parsed.body {
        let scrubbed = scrub_body(raw_body);
        // Try to pretty-print as JSON for readability, else use raw string
        let content = if let Ok(val) = serde_json::from_str::<serde_json::Value>(&scrubbed) {
            serde_json::to_string_pretty(&val).unwrap_or(scrubbed)
        } else {
            scrubbed
        };
        // Indent JSON content as YAML literal block
        let indented: String = content
            .lines()
            .map(|l| format!("        {l}\n"))
            .collect();
        format!("    body:\n      type: json\n      content: |\n{indented}")
    } else {
        String::new()
    };

    // Build assertions section (WithTests mode only)
    let assertions = match mode {
        ImportMode::WithTests => "    assertions:\n      - status: 200\n".to_string(),
        ImportMode::Scaffold => String::new(),
    };

    let headers_section = if header_lines.is_empty() {
        String::new()
    } else {
        format!("    headers:\n{header_lines}")
    };

    let yaml = format!(
        r#"name: "Imported Collection"
version: "1.0"

environment:
  baseUrl: "{base}"

requests:
  - name: "{name}"
    method: {method}
    url: "{{{{baseUrl}}}}{path}"
{headers_section}{body_section}{assertions}"#,
        method = parsed.method,
    );

    Ok(yaml)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImportMode;

    #[test]
    fn tokenize_simple() {
        let tokens = tokenize("curl https://example.com");
        assert_eq!(tokens, vec!["curl", "https://example.com"]);
    }

    #[test]
    fn tokenize_quoted_header() {
        let tokens = tokenize(r#"curl -H "Authorization: Bearer abc123" https://example.com"#);
        assert_eq!(tokens[1], "-H");
        assert_eq!(tokens[2], "Authorization: Bearer abc123");
    }

    #[test]
    fn tokenize_line_continuation() {
        let tokens = tokenize("curl \\\n  https://example.com");
        assert_eq!(tokens, vec!["curl", "https://example.com"]);
    }

    #[test]
    fn parse_get_no_flags() {
        let yaml = convert("curl https://api.example.com/users", ImportMode::Scaffold).unwrap();
        assert!(yaml.contains("method: GET"));
        assert!(yaml.contains("GET /users"));
        assert!(yaml.contains("baseUrl: \"https://api.example.com\""));
    }

    #[test]
    fn parse_infers_post_when_data_present() {
        let yaml = convert(
            r#"curl -d '{"name":"Alice"}' https://api.example.com/users"#,
            ImportMode::Scaffold,
        ).unwrap();
        assert!(yaml.contains("method: POST"));
    }

    #[test]
    fn explicit_method_overrides_inference() {
        let yaml = convert(
            r#"curl -X PUT -d '{"name":"Bob"}' https://api.example.com/users/1"#,
            ImportMode::Scaffold,
        ).unwrap();
        assert!(yaml.contains("method: PUT"));
    }

    #[test]
    fn scrubs_authorization_header() {
        let yaml = convert(
            r#"curl -H "Authorization: Bearer secret-token" https://api.example.com/me"#,
            ImportMode::Scaffold,
        ).unwrap();
        assert!(yaml.contains("{{authorization}}"));
        assert!(!yaml.contains("secret-token"));
    }

    #[test]
    fn scrubs_api_key_header() {
        let yaml = convert(
            r#"curl -H "X-Api-Key: sk-abc123" https://api.example.com/data"#,
            ImportMode::Scaffold,
        ).unwrap();
        assert!(yaml.contains("{{api_key}}"));
        assert!(!yaml.contains("sk-abc123"));
    }

    #[test]
    fn non_sensitive_header_is_not_scrubbed() {
        let yaml = convert(
            r#"curl -H "Content-Type: application/json" https://api.example.com/users"#,
            ImportMode::Scaffold,
        ).unwrap();
        assert!(yaml.contains("application/json"));
    }

    #[test]
    fn scrubs_password_in_json_body() {
        let yaml = convert(
            r#"curl -d '{"username":"alice","password":"hunter2"}' https://api.example.com/login"#,
            ImportMode::Scaffold,
        ).unwrap();
        assert!(yaml.contains("{{password}}"));
        assert!(!yaml.contains("hunter2"));
    }

    #[test]
    fn user_flag_becomes_basic_auth_placeholder() {
        let yaml = convert(
            "curl -u admin:secret https://api.example.com/admin",
            ImportMode::Scaffold,
        ).unwrap();
        assert!(yaml.contains("{{credentials}}"));
        assert!(!yaml.contains("secret"));
    }

    #[test]
    fn with_tests_mode_adds_status_assertion() {
        let yaml = convert(
            "curl https://api.example.com/users",
            ImportMode::WithTests,
        ).unwrap();
        assert!(yaml.contains("status: 200"));
    }

    #[test]
    fn scaffold_mode_has_no_assertions() {
        let yaml = convert(
            "curl https://api.example.com/users",
            ImportMode::Scaffold,
        ).unwrap();
        assert!(!yaml.contains("assertions:"));
    }

    #[test]
    fn missing_url_returns_error() {
        let result = convert("curl -X GET", ImportMode::Scaffold);
        assert!(result.is_err());
    }

    #[test]
    fn non_curl_input_returns_error() {
        let result = convert("wget https://example.com", ImportMode::Scaffold);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run full curl test suite**

```bash
cargo test -p strex-import
```
Expected: all tests pass.

- [ ] **Step 4: Run clippy**

```bash
cargo clippy -p strex-import -- -D warnings
```
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/strex-import/src/curl.rs
git commit -m "feat(import): implement curl parser with scrubbing and YAML output"
```

---

## Task 3: OpenAPI converter

**Files:**
- Modify: `crates/strex-import/src/openapi.rs`

Parses OpenAPI 3.x and Swagger 2.x specs (as YAML or JSON strings) and generates a Strex YAML collection.

- [ ] **Step 1: Write the full `openapi.rs` with tests**

Replace `crates/strex-import/src/openapi.rs` with:

```rust
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
const FALLBACK_COMMENT: &str =
    "  # TODO: replace baseUrl with your API base URL";

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
    let host = root
        .get("host")
        .and_then(|h| h.as_str());
    let Some(host) = host else {
        return FALLBACK_BASE_URL.into();
    };
    let scheme = root
        .get("schemes")
        .and_then(|s| s.as_sequence())
        .and_then(|seq| seq.first())
        .and_then(|v| v.as_str())
        .unwrap_or("https");
    let base_path = root
        .get("basePath")
        .and_then(|b| b.as_str())
        .unwrap_or("/");
    let base_path = base_path.trim_end_matches('/');
    format!("{scheme}://{host}{base_path}")
}

// ── Path parameter conversion ─────────────────────────────────────────────────

/// Convert OpenAPI path params like `{id}` to Strex `{{id}}`.
fn convert_path_params(path: &str) -> String {
    let mut result = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
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

fn collect_operations(root: &Value, base_url: &str, mode: &ImportMode) -> Vec<RequestEntry> {
    const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

    let Some(paths) = root.get("paths").and_then(|p| p.as_mapping()) else {
        return Vec::new();
    };

    let mut entries = Vec::new();

    for (path_key, path_item) in paths {
        let Some(path_str) = path_key.as_str() else { continue };
        let strex_path = convert_path_params(path_str);

        for method in HTTP_METHODS {
            let Some(operation) = path_item.get(method) else { continue };

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
                        a.push(format!("      - jsonPath: \"$.{field}\"\n        exists: true"));
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

pub(crate) fn convert(spec: &str, mode: ImportMode) -> Result<String, ImportError> {
    let root: Value = serde_yaml::from_str(spec)
        .map_err(|e| ImportError::OpenApiParse(e.to_string()))?;

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

    let entries = collect_operations(&root, &base, &mode);
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
        assert!(matches!(detect_version(&root).unwrap(), SpecVersion::OpenApi3));
    }

    #[test]
    fn detects_swagger2() {
        let root: Value = serde_yaml::from_str(SWAGGER2_SIMPLE).unwrap();
        assert!(matches!(detect_version(&root).unwrap(), SpecVersion::Swagger2));
    }

    #[test]
    fn unrecognised_format_returns_error() {
        let root: Value = serde_yaml::from_str("name: foo").unwrap();
        assert!(matches!(detect_version(&root), Err(ImportError::UnrecognisedFormat)));
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
        assert_eq!(convert_path_params("/users/{id}/posts/{postId}"), "/users/{{id}}/posts/{{postId}}");
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
}
```

- [ ] **Step 2: Run the tests**

```bash
cargo test -p strex-import
```
Expected: all tests pass.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -p strex-import -- -D warnings
```
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/strex-import/src/openapi.rs
git commit -m "feat(import): implement OpenAPI 2.x/3.x converter"
```

---

## Task 4: Backend — AppState, import module, and route wiring

**Files:**
- Modify: `crates/strex-ui/Cargo.toml`
- Modify: `crates/strex-ui/src/lib.rs`
- Create: `crates/strex-ui/src/import.rs`
- Modify: `crates/strex-ui/src/server.rs`

- [ ] **Step 1: Add `strex-import` dependency to `crates/strex-ui/Cargo.toml`**

Add after the `strex-core` line:
```toml
# Import conversion — generates Strex YAML from curl and OpenAPI sources
strex-import = { path = "../strex-import" }
```

- [ ] **Step 2: Declare `mod import` in `crates/strex-ui/src/lib.rs`**

Add `mod import;` to the module list:
```rust
mod collections;
mod events;
mod history;
mod import;
mod request_list;
mod routes;
mod server;
mod ws;
```

- [ ] **Step 3: Create `crates/strex-ui/src/import.rs` with handler stubs**

```rust
//! Route handlers for `POST /api/import/generate` and `POST /api/import/save`.

use std::fs::OpenOptions;
use std::io::Write;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::server::AppState;

// ── Request / response types ───────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportSource {
    Curl,
    Openapi,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportMode {
    Scaffold,
    WithTests,
}

impl From<ImportMode> for strex_import::ImportMode {
    fn from(m: ImportMode) -> Self {
        match m {
            ImportMode::Scaffold => strex_import::ImportMode::Scaffold,
            ImportMode::WithTests => strex_import::ImportMode::WithTests,
        }
    }
}

#[derive(Deserialize)]
pub struct GenerateRequest {
    source: ImportSource,
    input: String,
    mode: ImportMode,
}

#[derive(Serialize)]
struct GenerateOk {
    yaml: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Deserialize)]
pub struct SaveRequest {
    yaml: String,
    filename: String,
}

#[derive(Serialize)]
struct SaveOk {
    filename: String,
}

// ── Handlers ───────────────────────────────────────────────────────────────────

/// `POST /api/import/generate` — convert a curl command or OpenAPI spec to a Strex YAML string.
pub async fn generate(
    State(state): State<AppState>,
    Json(body): Json<GenerateRequest>,
) -> impl IntoResponse {
    let mode: strex_import::ImportMode = body.mode.into();

    let result = match body.source {
        ImportSource::Curl => strex_import::from_curl(&body.input, mode),
        ImportSource::Openapi => {
            // Detect URL vs file path
            let spec = if body.input.starts_with("http://") || body.input.starts_with("https://") {
                fetch_url(&state.http_client, &body.input).await
            } else {
                std::fs::read_to_string(&body.input)
                    .map_err(|e| strex_import::ImportError::OpenApiParse(e.to_string()))
            };
            spec.and_then(|s| strex_import::from_openapi(&s, mode))
        }
    };

    match result {
        Ok(yaml) => (StatusCode::OK, Json(serde_json::json!({ "yaml": yaml }))).into_response(),
        Err(strex_import::ImportError::FetchTimeout) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Request timed out fetching the spec URL" })),
        ).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn fetch_url(
    client: &reqwest::Client,
    url: &str,
) -> Result<String, strex_import::ImportError> {
    client
        .get(url)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                strex_import::ImportError::FetchTimeout
            } else {
                strex_import::ImportError::OpenApiParse(e.to_string())
            }
        })?
        .text()
        .await
        .map_err(|e| strex_import::ImportError::OpenApiParse(e.to_string()))
}

/// `POST /api/import/save` — write generated YAML to a file in the current working directory.
pub async fn save(Json(body): Json<SaveRequest>) -> impl IntoResponse {
    // Validate filename
    if !body.filename.ends_with(".yaml") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Filename must end in .yaml" })),
        ).into_response();
    }
    if body.filename.contains('/') || body.filename.contains('\\') || body.filename.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Filename must not contain path separators or .." })),
        ).into_response();
    }

    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Could not determine working directory: {e}") })),
            ).into_response()
        }
    };

    let path = cwd.join(&body.filename);

    // Atomically create — fails if file already exists (no TOCTOU race)
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": format!("File already exists: {}", body.filename) })),
            ).into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Could not write file: {e}") })),
            ).into_response()
        }
    };

    if let Err(e) = file.write_all(body.yaml.as_bytes()) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Write error: {e}") })),
        ).into_response();
    }

    (StatusCode::OK, Json(serde_json::json!({ "filename": body.filename }))).into_response()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use crate::server::build_router;

    async fn post_json(router: axum::Router, path: &str, body: serde_json::Value) -> axum::response::Response {
         router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn generate_curl_returns_yaml() {
        let app = build_router().unwrap(); // unwrap allowed in test code
        let res = post_json(
            app,
            "/api/import/generate",
            serde_json::json!({
                "source": "curl",
                "input": "curl https://api.example.com/users",
                "mode": "scaffold"
            }),
        ).await;
        assert_eq!(res.status(), 200);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["yaml"].as_str().unwrap().contains("GET"));
    }

    #[tokio::test]
    async fn generate_invalid_curl_returns_400() {
        let app = build_router().unwrap(); // unwrap allowed in test code
        let res = post_json(
            app,
            "/api/import/generate",
            serde_json::json!({
                "source": "curl",
                "input": "wget https://example.com",
                "mode": "scaffold"
            }),
        ).await;
        assert_eq!(res.status(), 400);
    }

    #[tokio::test]
    async fn save_rejects_traversal() {
        let app = build_router().unwrap(); // unwrap allowed in test code
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "../evil.yaml" }),
        ).await;
        assert_eq!(res.status(), 400);
    }

    #[tokio::test]
    async fn save_rejects_non_yaml_extension() {
        let app = build_router().unwrap(); // unwrap allowed in test code
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "collection.json" }),
        ).await;
        assert_eq!(res.status(), 400);
    }

    #[tokio::test]
    async fn save_writes_file_and_returns_filename() {
        let dir = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let app = build_router().unwrap(); // unwrap allowed in test code
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "my-import.yaml" }),
        ).await;

        std::env::set_current_dir(original).unwrap();

        assert_eq!(res.status(), 200);
        assert!(dir.path().join("my-import.yaml").exists());
    }

    #[tokio::test]
    async fn save_returns_409_if_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        std::fs::write(dir.path().join("dupe.yaml"), "existing").unwrap();

        let app = build_router().unwrap(); // unwrap allowed in test code
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "dupe.yaml" }),
        ).await;

        std::env::set_current_dir(original).unwrap();
        assert_eq!(res.status(), 409);
    }
}
```

- [ ] **Step 4: Add `AppState` and `build_router` to `crates/strex-ui/src/server.rs`**

Add `AppState` and extract the router construction into a `pub(crate) fn build_router()` function so tests can access it. Replace the contents of `server.rs`:

```rust
//! Axum server setup — router, bind, browser open.

use std::path::PathBuf;
use std::time::Duration;

use axum::{
    routing::{any, get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::{import, request_list, routes, ws};

/// Shared application state threaded through all route handlers.
#[derive(Clone)]
pub struct AppState {
    /// HTTP client with a 10-second timeout, shared across all import requests.
    pub http_client: reqwest::Client,
}

/// Build the Axum router with all routes and state attached.
///
/// Extracted so integration tests can call `build_router()` directly.
pub(crate) fn build_router() -> anyhow::Result<Router> {
    let state = AppState {
        http_client: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?,
    };

    Ok(Router::new()
        .route("/", get(routes::serve_index))
        .route(
            "/assets/*path",
            get(|axum::extract::Path(p): axum::extract::Path<String>| {
                routes::serve_asset(format!("assets/{p}"))
            }),
        )
        .route("/api/collections", get(routes::list_collections))
        .route(
            "/api/collection-requests",
            get(request_list::list_collection_requests),
        )
        .route("/api/data-preview", get(routes::data_preview))
        .route(
            "/api/history",
            post(routes::save_history).get(routes::list_history),
        )
        .route("/api/history/:id", get(routes::get_history))
        .route("/api/import/generate", post(import::generate))
        .route("/api/import/save", post(import::save))
        .route("/ws", any(ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state))
}

/// Options for starting the strex UI server.
pub struct ServerOpts {
    /// TCP port to bind on. Default: 7878.
    pub port: u16,
    /// Optional collection path to pre-select in the UI.
    pub collection: Option<PathBuf>,
}

/// Start the Axum server, print the URL, open the browser, and block until shutdown.
pub async fn start_server(opts: ServerOpts) -> anyhow::Result<()> {
    let _ = opts.collection;
    let app = build_router()?;

    let addr = format!("127.0.0.1:{}", opts.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let url = format!("http://{}", addr);
    println!("strex UI running at {url}");
    println!("Press Ctrl+C to stop.");

    let _ = open::that(&url);

    axum::serve(listener, app).await?;
    Ok(())
}
```

Note: existing route handlers in `routes.rs` don't use `State`, so they remain unchanged and are compatible with the new `with_state(state)` call.

- [ ] **Step 5: Add `tower` to dev-dependencies in `crates/strex-ui/Cargo.toml`**

The test helpers use `tower::ServiceExt`. Add:
```toml
[dev-dependencies]
tempfile = "3"
# Test utilities for calling Axum routers without binding a port
tower = { version = "0.4", features = ["util"] }
```

- [ ] **Step 6: Run the backend tests**

```bash
cargo test -p strex-ui
```
Expected: all tests pass.

- [ ] **Step 7: Run clippy on the whole workspace**

```bash
cargo clippy -- -D warnings
```
Expected: no warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/strex-ui/ crates/strex-import/
git commit -m "feat(ui): add /api/import/generate and /api/import/save endpoints"
```

---

## Task 5: Frontend API functions

**Files:**
- Modify: `crates/strex-ui/frontend/src/lib/api.ts`

- [ ] **Step 1: Add `importGenerate` and `importSave` to `api.ts`**

Append to the bottom of `crates/strex-ui/frontend/src/lib/api.ts`:

```typescript
export async function importGenerate(payload: {
  source: 'curl' | 'openapi'
  input: string
  mode: 'scaffold' | 'with_tests'
}): Promise<{ yaml: string }> {
  const res = await fetch('/api/import/generate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }))
    throw new Error(body.error ?? `Import failed: ${res.status}`)
  }
  return res.json()
}

export async function importSave(payload: {
  yaml: string
  filename: string
}): Promise<{ filename: string }> {
  const res = await fetch('/api/import/save', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }))
    throw new Error(body.error ?? `Save failed: ${res.status}`)
  }
  return res.json()
}
```

- [ ] **Step 2: Verify TypeScript compiles**

```bash
cd crates/strex-ui/frontend && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/strex-ui/frontend/src/lib/api.ts
git commit -m "feat(ui/frontend): add importGenerate and importSave API functions"
```

---

## Task 6: ImportModal Svelte component

**Files:**
- Create: `crates/strex-ui/frontend/src/components/ImportModal.svelte`

This component manages the 3-step state machine internally and calls back to the parent on save success.

- [ ] **Step 1: Create `ImportModal.svelte`**

```svelte
<script lang="ts">
  import { importGenerate, importSave } from '../lib/api'

  interface Props {
    onSaved: (filename: string) => void
    onClose: () => void
  }

  let { onSaved, onClose }: Props = $props()

  type Step = 'source-select' | 'input-form' | 'preview'
  type Source = 'curl' | 'openapi'
  type Mode = 'scaffold' | 'with_tests'

  let step = $state<Step>('source-select')
  let source = $state<Source>('curl')
  let input = $state('')
  let mode = $state<Mode>('scaffold')
  let generatedYaml = $state('')
  let filename = $state('imported-collection.yaml')
  let generating = $state(false)
  let saving = $state(false)
  let error = $state<string | null>(null)

  function selectSource(s: Source) {
    source = s
    step = 'input-form'
    error = null
  }

  async function handleGenerate() {
    if (!input.trim()) {
      error = source === 'curl' ? 'Paste a curl command first.' : 'Enter a file path or URL.'
      return
    }
    generating = true
    error = null
    try {
      const result = await importGenerate({ source, input: input.trim(), mode })
      generatedYaml = result.yaml
      step = 'preview'
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      generating = false
    }
  }

  function handleBack() {
    generatedYaml = ''
    step = 'input-form'
    error = null
  }

  async function handleSave() {
    if (!filename.trim()) {
      error = 'Filename is required.'
      return
    }
    saving = true
    error = null
    try {
      const result = await importSave({ yaml: generatedYaml, filename: filename.trim() })
      onSaved(result.filename)
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      saving = false
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose()
  }
</script>

<!-- Backdrop -->
<div class="backdrop" onclick={handleBackdropClick} role="dialog" aria-modal="true">
  <div class="modal">
    <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>

    {#if step === 'source-select'}
      <h2 class="modal-title">Import Collection</h2>
      <p class="modal-subtitle">Generate a Strex YAML collection from an existing source</p>

      <div class="source-list">
        <button class="source-tile" onclick={() => selectSource('curl')}>
          <span class="tile-name">curl command</span>
          <span class="tile-desc">Paste a curl snippet</span>
        </button>
        <button class="source-tile" onclick={() => selectSource('openapi')}>
          <span class="tile-name">OpenAPI / Swagger</span>
          <span class="tile-desc">File path or URL</span>
        </button>
        <button class="source-tile" disabled>
          <span class="tile-name">Postman collection</span>
          <span class="tile-desc">Coming soon</span>
        </button>
      </div>

    {:else if step === 'input-form'}
      <button class="back-link" onclick={() => { step = 'source-select'; error = null }}>← Back</button>
      <h2 class="modal-title">{source === 'curl' ? 'curl command' : 'OpenAPI / Swagger'}</h2>

      {#if source === 'curl'}
        <label class="field">
          <span>Paste your curl command</span>
          <textarea
            class="code-input"
            placeholder="curl -X POST https://api.example.com/users -H &quot;Authorization: Bearer ...&quot; -d '{...}'"
            bind:value={input}
            rows={5}
          ></textarea>
        </label>
      {:else}
        <label class="field">
          <span>File path or URL</span>
          <input
            class="text-input"
            type="text"
            placeholder="./openapi.yaml  or  https://api.example.com/openapi.json"
            bind:value={input}
          />
        </label>
      {/if}

      <fieldset class="mode-toggle">
        <legend>Output mode</legend>
        <label class="mode-option">
          <input type="radio" bind:group={mode} value="scaffold" />
          Quick scaffold
        </label>
        <label class="mode-option">
          <input type="radio" bind:group={mode} value="with_tests" />
          Generate tests
        </label>
      </fieldset>

      {#if error}
        <p class="error-msg">{error}</p>
      {/if}

      <button class="primary-btn" onclick={handleGenerate} disabled={generating}>
        {generating ? 'Generating…' : 'Generate →'}
      </button>

    {:else if step === 'preview'}
      <button class="back-link" onclick={handleBack}>← Back</button>
      <h2 class="modal-title">Preview</h2>

      <pre class="yaml-preview">{generatedYaml}</pre>

      <label class="field">
        <span>Save as</span>
        <input class="text-input" type="text" bind:value={filename} />
      </label>

      {#if error}
        <p class="error-msg">{error}</p>
      {/if}

      <button class="primary-btn" onclick={handleSave} disabled={saving}>
        {saving ? 'Saving…' : 'Save'}
      </button>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: #1a1a2e;
    border: 1px solid #2a2a4a;
    border-radius: 10px;
    padding: 28px;
    width: min(520px, 92vw);
    max-height: 90vh;
    overflow-y: auto;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 16px;
    color: #e0e0e0;
  }

  .close-btn {
    position: absolute;
    top: 14px;
    right: 16px;
    background: none;
    border: none;
    color: #666;
    font-size: 1rem;
    cursor: pointer;
    line-height: 1;
  }
  .close-btn:hover { color: #fff; }

  .modal-title {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
    color: #e0e0e0;
  }

  .modal-subtitle {
    margin: 0;
    font-size: 0.8rem;
    color: #666;
  }

  .back-link {
    background: none;
    border: none;
    color: #ff6b35;
    font-size: 0.8rem;
    cursor: pointer;
    padding: 0;
    text-align: left;
  }

  .source-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .source-tile {
    background: #0f0f23;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
    padding: 12px 16px;
    text-align: left;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 2px;
    transition: border-color 0.15s;
  }
  .source-tile:hover:not(:disabled) { border-color: #ff6b35; }
  .source-tile:disabled { opacity: 0.4; cursor: not-allowed; }

  .tile-name { color: #e0e0e0; font-weight: 600; font-size: 0.9rem; }
  .tile-desc { color: #888; font-size: 0.78rem; }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.85rem;
    color: #bbb;
  }

  .code-input, .text-input {
    background: #0f0f23;
    border: 1px solid #333;
    border-radius: 4px;
    color: #e0e0e0;
    padding: 8px 10px;
    font-size: 0.82rem;
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
  }
  .code-input { font-family: monospace; }

  .mode-toggle {
    border: 1px solid #2a2a4a;
    border-radius: 4px;
    padding: 10px 14px;
    display: flex;
    gap: 20px;
  }
  .mode-toggle legend { color: #888; font-size: 0.8rem; padding: 0 4px; }
  .mode-option { display: flex; align-items: center; gap: 6px; font-size: 0.85rem; color: #ccc; cursor: pointer; }

  .yaml-preview {
    background: #0f0f23;
    border: 1px solid #2a2a4a;
    border-radius: 4px;
    padding: 12px;
    font-size: 0.75rem;
    font-family: monospace;
    color: #ccc;
    max-height: 240px;
    overflow-y: auto;
    white-space: pre;
    margin: 0;
  }

  .primary-btn {
    padding: 10px 16px;
    background: #ff6b35;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }
  .primary-btn:hover:not(:disabled) { background: #ff8555; }
  .primary-btn:disabled { background: #444; cursor: not-allowed; }

  .error-msg {
    margin: 0;
    font-size: 0.8rem;
    color: #f87171;
  }
</style>
```

- [ ] **Step 2: Check TypeScript via build**

```bash
cd crates/strex-ui/frontend && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/strex-ui/frontend/src/components/ImportModal.svelte
git commit -m "feat(ui/frontend): add ImportModal Svelte component"
```

---

## Task 7: Wire ImportModal into ConfigPanel and rebuild frontend

**Files:**
- Modify: `crates/strex-ui/frontend/src/components/ConfigPanel.svelte`

- [ ] **Step 1: Add Import button and modal to `ConfigPanel.svelte`**

At the top of the `<script>` block, add the import:
```ts
import ImportModal from './ImportModal.svelte'
```

After the existing state declarations (after `let dataPreviewLoading`), add:
```ts
let showImportModal = $state(false)
```

In `handleRun` there's no change needed.

Add a new function after `handleRun`:
```ts
function handleImportSaved(savedFilename: string) {
  showImportModal = false
  fetchCollections()
    .then((files) => {
      collections = files
      selectedCollection = savedFilename
    })
    .catch((e: unknown) => console.error('Failed to refresh collections:', e))
}
```

In the template, after the closing `</button>` of the Run button (around line 237), add:
```svelte
<button
  class="import-button"
  onclick={() => (showImportModal = true)}
  disabled={running}
>
  + Import
</button>

{#if showImportModal}
  <ImportModal
    onSaved={handleImportSaved}
    onClose={() => (showImportModal = false)}
  />
{/if}
```

In the `<style>` block, add:
```css
.import-button {
  margin-top: 4px;
  padding: 8px 12px;
  background: transparent;
  color: #aaa;
  border: 1px dashed #444;
  border-radius: 6px;
  font-size: 0.85rem;
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s;
}

.import-button:hover:not(:disabled) {
  border-color: #ff6b35;
  color: #ff6b35;
}

.import-button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
```

- [ ] **Step 2: Build the frontend**

```bash
cd crates/strex-ui/frontend && npm run build
```
Expected: build succeeds, `dist/` is updated.

- [ ] **Step 3: Verify the full workspace still compiles (picks up new embedded assets)**

```bash
cargo build -p strex-ui
```
Expected: no errors.

- [ ] **Step 4: Smoke test manually**

```bash
cargo run -p strex-cli -- ui
```

Open http://localhost:7878 in a browser. Verify:
- `+ Import` button appears below `Run` in the sidebar
- Clicking it opens a modal with 3 source tiles (Postman disabled)
- Selecting curl shows a textarea; selecting OpenAPI shows a text input
- Pasting `curl https://httpbin.org/get` and clicking Generate shows a YAML preview
- Clicking Save writes the file and auto-selects it in the collection dropdown
- Clicking ← Back from the preview returns to the form with the input still populated

- [ ] **Step 5: Run the full test suite one final time**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/strex-ui/frontend/src/components/ConfigPanel.svelte crates/strex-ui/frontend/dist/
git commit -m "feat(ui/frontend): wire ImportModal into ConfigPanel and rebuild dist"
```
