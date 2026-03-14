use std::collections::HashMap;

/// The top-level container parsed from a Strex YAML collection file.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    /// Human-readable collection name.
    pub name: String,
    /// Schema version string (e.g. `"1.0"`).
    pub version: String,
    /// Inline environment variables — merged lowest-priority in variable resolution.
    #[serde(default)]
    pub environment: HashMap<String, String>,
    /// Collection-level variables. `None` values declare a required variable with no default.
    #[serde(default)]
    pub variables: HashMap<String, Option<String>>,
    /// Ordered list of HTTP requests to execute.
    pub requests: Vec<Request>,
}

/// A single HTTP request definition within a collection.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Unique name for this request (used in output and error messages).
    pub name: String,
    /// HTTP method — `GET`, `POST`, `PUT`, `DELETE`, etc.
    pub method: String,
    /// Target URL; may contain `{{variable}}` placeholders.
    pub url: String,
    /// Request headers — keys and values may contain `{{variable}}` placeholders.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Optional request body.
    pub body: Option<Body>,
    /// Optional inline JavaScript to run after the response is received.
    pub script: Option<String>,
    /// Declarative assertions to evaluate against the response.
    /// Stored as raw maps — evaluated by the runner in sub-project 2.
    #[serde(default)]
    pub assertions: Vec<HashMap<String, serde_yaml::Value>>,
    /// Per-request timeout in milliseconds. Overrides the runner's global default.
    pub timeout: Option<u64>,
}

/// The body to send with a request.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    /// How the body content should be serialised before sending.
    #[serde(rename = "type")]
    pub body_type: BodyType,
    /// The body content — may be a YAML scalar, mapping, or sequence.
    pub content: serde_yaml::Value,
}

/// The serialisation format for a request body.
#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BodyType {
    /// Serialise as JSON (`Content-Type: application/json`).
    Json,
    /// Send as plain text (`Content-Type: text/plain`).
    Text,
    /// URL-encode as form data (`Content-Type: application/x-www-form-urlencoded`).
    Form,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_type_json_is_distinct_from_text() {
        assert_ne!(BodyType::Json, BodyType::Text);
    }

    #[test]
    fn request_has_optional_body() {
        // Confirm the struct compiles and body is Option
        let _r = Request {
            name: "test".to_string(),
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            headers: Default::default(),
            body: None,
            script: None,
            assertions: vec![],
            timeout: None,
        };
    }
}
