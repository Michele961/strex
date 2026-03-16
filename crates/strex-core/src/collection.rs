use std::collections::HashMap;

/// Action to take when a request fails (any assertion fails or a script throws).
///
/// Deserializes from two YAML shapes:
/// - `on_failure: stop` — plain string scalar
/// - `on_failure: {skip_to: "name"}` — mapping with a `skip_to` key
#[derive(Debug, Clone, PartialEq)]
pub enum OnFailure {
    /// Abort the collection run — all subsequent requests are skipped.
    Stop,
    /// Skip requests until the named request is reached, then resume execution there.
    SkipTo(String),
}

impl<'de> serde::Deserialize<'de> for OnFailure {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct OnFailureVisitor;

        impl<'de> Visitor<'de> for OnFailureVisitor {
            type Value = OnFailure;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(r#"string "stop" or mapping {skip_to: <name>}"#)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<OnFailure, E> {
                if v == "stop" {
                    Ok(OnFailure::Stop)
                } else {
                    Err(E::unknown_variant(v, &["stop"]))
                }
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OnFailure, A::Error> {
                let key: String = map
                    .next_key()?
                    .ok_or_else(|| de::Error::custom("expected `skip_to` key"))?;
                if key != "skip_to" {
                    return Err(de::Error::unknown_field(&key, &["skip_to"]));
                }
                let value: String = map.next_value()?;
                // Reject extra keys
                if map.next_key::<String>()?.is_some() {
                    return Err(de::Error::custom("unexpected extra key after `skip_to`"));
                }
                Ok(OnFailure::SkipTo(value))
            }
        }

        d.deserialize_any(OnFailureVisitor)
    }
}

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
    /// Action to take when this request fails. `None` means log and continue.
    pub on_failure: Option<OnFailure>,
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
            on_failure: None,
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
                on_failure: None,
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
