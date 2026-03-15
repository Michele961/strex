use std::collections::HashMap;

/// The top-level container parsed from a Strex YAML collection file.
#[derive(Debug, Clone, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Deserialize)]
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
    /// JavaScript to run before the HTTP request (Phase 2). `response` is not available.
    pub pre_script: Option<String>,

    /// JavaScript to run after the HTTP response is captured (Phase 5).
    ///
    /// The YAML key `script:` is accepted as an alias for `post_script:` for backward
    /// compatibility. Having both `script:` and `post_script:` in the same request block
    /// is a `CollectionError::YamlParse` (duplicate field).
    #[serde(alias = "script")]
    pub post_script: Option<String>,
    /// Declarative assertions to evaluate against the response.
    /// Stored as raw maps — evaluated by the runner in sub-project 2.
    #[serde(default)]
    pub assertions: Vec<HashMap<String, serde_yaml::Value>>,
    /// Per-request timeout in milliseconds. Overrides the runner's global default.
    pub timeout: Option<u64>,
}

/// The body to send with a request.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    /// How the body content should be serialised before sending.
    #[serde(rename = "type")]
    pub body_type: BodyType,
    /// The body content — may be a YAML scalar, mapping, or sequence.
    pub content: serde_yaml::Value,
}

/// The serialisation format for a request body.
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
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
            pre_script: None,
            post_script: None,
            assertions: vec![],
            timeout: None,
        };
    }

    #[test]
    fn collection_request_body_body_type_implement_clone() {
        let col = Collection {
            name: "c".to_string(),
            version: "1.0".to_string(),
            environment: HashMap::new(),
            variables: HashMap::new(),
            requests: vec![Request {
                name: "r".to_string(),
                method: "GET".to_string(),
                url: "https://example.com".to_string(),
                headers: HashMap::new(),
                body: Some(Body {
                    body_type: BodyType::Json,
                    content: serde_yaml::Value::Null,
                }),
                pre_script: None,
                post_script: None,
                assertions: vec![],
                timeout: None,
            }],
        };
        let cloned = col.clone();
        assert_eq!(cloned.name, "c");
        assert_eq!(cloned.requests[0].name, "r");
        assert!(matches!(
            cloned.requests[0].body.as_ref().unwrap().body_type,
            BodyType::Json
        ));
    }
}
