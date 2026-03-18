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
        if body.is_some() {
            "POST".into()
        } else {
            "GET".into()
        }
    });

    // -u user:pass → Authorization: Basic {{credentials}}
    if user.is_some() {
        headers.push(("Authorization".into(), "Basic {{credentials}}".into()));
    }

    Ok(ParsedCurl {
        method,
        url,
        headers,
        body,
    })
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
            // Skip if already contains a placeholder (e.g. from -u processing)
            if value.contains("{{") {
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
                obj.insert(
                    field.to_string(),
                    serde_json::Value::String(format!("{{{{{field}}}}}")),
                );
            }
        }
    }
    serde_json::to_string(&val).unwrap_or_else(|_| body.to_string())
}

// ── URL decomposition ─────────────────────────────────────────────────────────

fn base_url(url: &str) -> String {
    // Extract scheme://host (strip path/query)
    if let Some(after_scheme) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    {
        let scheme = if url.starts_with("https") {
            "https"
        } else {
            "http"
        };
        let host = after_scheme.split('/').next().unwrap_or(after_scheme);
        format!("{scheme}://{host}")
    } else {
        url.to_string()
    }
}

fn url_path(url: &str) -> String {
    // Extract /path (without query string)
    if let Some(after_scheme) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    {
        let rest = after_scheme.split_once('/').map(|x| x.1).unwrap_or("");
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

/// Parse a curl command string and return a Strex YAML collection string.
///
/// # Errors
/// Returns [`ImportError::CurlParse`] if the input does not start with `curl` or contains no URL.
pub(crate) fn convert(input: &str, mode: ImportMode) -> Result<String, ImportError> {
    let tokens = tokenize(input);
    if tokens.is_empty() || tokens[0].to_lowercase() != "curl" {
        return Err(ImportError::CurlParse(
            "input must start with 'curl'".into(),
        ));
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
        let indented: String = content.lines().map(|l| format!("        {l}\n")).collect();
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
        )
        .unwrap();
        assert!(yaml.contains("method: POST"));
    }

    #[test]
    fn explicit_method_overrides_inference() {
        let yaml = convert(
            r#"curl -X PUT -d '{"name":"Bob"}' https://api.example.com/users/1"#,
            ImportMode::Scaffold,
        )
        .unwrap();
        assert!(yaml.contains("method: PUT"));
    }

    #[test]
    fn scrubs_authorization_header() {
        let yaml = convert(
            r#"curl -H "Authorization: Bearer secret-token" https://api.example.com/me"#,
            ImportMode::Scaffold,
        )
        .unwrap();
        assert!(yaml.contains("{{authorization}}"));
        assert!(!yaml.contains("secret-token"));
    }

    #[test]
    fn scrubs_api_key_header() {
        let yaml = convert(
            r#"curl -H "X-Api-Key: sk-abc123" https://api.example.com/data"#,
            ImportMode::Scaffold,
        )
        .unwrap();
        assert!(yaml.contains("{{api_key}}"));
        assert!(!yaml.contains("sk-abc123"));
    }

    #[test]
    fn non_sensitive_header_is_not_scrubbed() {
        let yaml = convert(
            r#"curl -H "Content-Type: application/json" https://api.example.com/users"#,
            ImportMode::Scaffold,
        )
        .unwrap();
        assert!(yaml.contains("application/json"));
    }

    #[test]
    fn scrubs_password_in_json_body() {
        let yaml = convert(
            r#"curl -d '{"username":"alice","password":"hunter2"}' https://api.example.com/login"#,
            ImportMode::Scaffold,
        )
        .unwrap();
        assert!(yaml.contains("{{password}}"));
        assert!(!yaml.contains("hunter2"));
    }

    #[test]
    fn user_flag_becomes_basic_auth_placeholder() {
        let yaml = convert(
            "curl -u admin:secret https://api.example.com/admin",
            ImportMode::Scaffold,
        )
        .unwrap();
        assert!(yaml.contains("Basic {{credentials}}"));
        assert!(!yaml.contains("secret"));
    }

    #[test]
    fn with_tests_mode_adds_status_assertion() {
        let yaml = convert("curl https://api.example.com/users", ImportMode::WithTests).unwrap();
        assert!(yaml.contains("status: 200"));
    }

    #[test]
    fn scaffold_mode_has_no_assertions() {
        let yaml = convert("curl https://api.example.com/users", ImportMode::Scaffold).unwrap();
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
