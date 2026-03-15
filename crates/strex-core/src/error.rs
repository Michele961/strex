use std::path::PathBuf;

/// All errors that can be produced by `strex-core` operations.
///
/// Callers should match on specific variants to present user-friendly messages.
#[derive(thiserror::Error, Debug)]
pub enum CollectionError {
    /// The collection file could not be read from disk.
    #[error("Could not read file {path}: {cause}")]
    FileRead { path: PathBuf, cause: String },

    /// The collection file exceeds the 10 MB size limit.
    #[error("Collection file too large: {size_bytes} bytes (limit: 10MB)")]
    FileTooLarge { size_bytes: usize },

    /// A YAML anchor (`&name`) was found — not allowed in Strex collections.
    #[error("Anchors are not allowed in Strex collections")]
    AnchorsNotAllowed,

    /// A YAML alias (`*name`) was found — not allowed in Strex collections.
    #[error("Aliases are not allowed in Strex collections")]
    AliasesNotAllowed,

    /// A key appears more than once at the same YAML level.
    #[error("Duplicate key '{key}' found")]
    DuplicateKey { key: String },

    /// The raw YAML could not be parsed.
    #[error("YAML parse error: {cause}")]
    YamlParse { cause: String },

    /// The YAML document is nested more than 20 levels deep.
    #[error("Maximum nesting depth exceeded ({max} levels)")]
    NestingTooDeep { max: usize },

    /// An unknown field was found during deserialization.
    ///
    /// When a close match exists among valid fields, `suggestion` contains it.
    #[error("Unknown field '{field}'{}", .suggestion.as_deref().map(|s| format!(". Did you mean '{s}'?")).unwrap_or_default())]
    UnknownField {
        field: String,
        suggestion: Option<String>,
    },

    /// A required field is absent from the collection YAML.
    #[error("Missing required field '{field}'")]
    MissingField { field: String },

    /// A `{{variable}}` placeholder was used but no matching variable is defined.
    #[error("Variable '{variable}' not found. Available: {available:?}")]
    VariableNotFound {
        variable: String,
        available: Vec<String>,
    },

    /// A `{{` was opened but never closed in the template string.
    #[error("Malformed variable placeholder in template '{template}' at position {position}")]
    VariableSyntaxError { template: String, position: usize },
}

/// Identifies which assertion type produced a failure.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum AssertionType {
    /// A `status` assertion.
    Status,
    /// A `jsonPath` assertion.
    JsonPath,
    /// A `header` assertion.
    Header,
}

/// A single assertion failure collected in phase 6 of the request lifecycle.
///
/// Failures are non-fatal — all failures for a request are collected before recording.
/// SP5 (CLI) formats these for display.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct AssertionFailure {
    /// Which kind of assertion failed.
    pub assertion_type: AssertionType,
    /// What value was expected.
    pub expected: String,
    /// What value was actually observed.
    pub actual: String,
}

/// A stopping error for a single request (phases 1 or 3 of the request lifecycle).
///
/// When a `RequestError` occurs the request is marked `Error` and execution continues
/// to the next request in the collection. No `RequestError` stops the whole collection.
#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum RequestError {
    /// Variable interpolation failed. Carries the original `CollectionError` to preserve
    /// structured fields (variable name, available variables, position) for user display.
    #[error("Variable interpolation failed: {0}")]
    Interpolation(#[source] CollectionError),

    /// DNS resolution failed. Mapped from reqwest connect errors whose message contains "dns".
    #[error("DNS resolution failed for '{domain}': {cause}")]
    DnsResolution { domain: String, cause: String },

    /// TLS handshake failed. Mapped from reqwest connect errors whose message contains
    /// "tls" or "certificate".
    #[error("TLS handshake failed for '{domain}': {cause}")]
    TlsHandshake { domain: String, cause: String },

    /// TCP connection refused. Mapped from reqwest connect errors whose message contains
    /// "connection refused".
    #[error("Connection refused to '{url}'")]
    ConnectionRefused { url: String },

    /// Request timed out. SP2 collapses ADR-0002's `ConnectionTimeout` and `HttpTimeout`
    /// into a single variant — phase distinction is deferred post-MVP.
    #[error("Request to '{url}' timed out after {timeout_ms}ms")]
    Timeout { url: String, timeout_ms: u64 },

    /// Redirect limit exceeded. `max_redirects` is reqwest's default (10); not configurable in SP2.
    #[error("Too many redirects for '{url}' (max: {max_redirects})")]
    TooManyRedirects { url: String, max_redirects: u32 },

    /// HTTP response could not be read or decoded.
    #[error("Invalid HTTP response: {cause}")]
    InvalidResponse { cause: String },

    /// Catch-all for reqwest errors not matched by the rules above.
    #[error("Network error: {cause}")]
    Network { cause: String },

    /// An assertion map in the YAML is malformed (unknown key or missing operator).
    #[error("Invalid assertion definition: {cause}")]
    InvalidAssertion { cause: String },

    /// A request body has structurally invalid content (e.g. non-scalar Form/Text value).
    /// Distinct from `InvalidAssertion` to prevent confusing error messages for body problems.
    #[error("Invalid request body: {cause}")]
    InvalidBody { cause: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_field_with_suggestion_formats_correctly() {
        let err = CollectionError::UnknownField {
            field: "metod".to_string(),
            suggestion: Some("method".to_string()),
        };
        assert_eq!(
            err.to_string(),
            "Unknown field 'metod'. Did you mean 'method'?"
        );
    }

    #[test]
    fn unknown_field_without_suggestion_formats_correctly() {
        let err = CollectionError::UnknownField {
            field: "foobar".to_string(),
            suggestion: None,
        };
        assert_eq!(err.to_string(), "Unknown field 'foobar'");
    }

    #[test]
    fn variable_not_found_includes_variable_name() {
        let err = CollectionError::VariableNotFound {
            variable: "userId".to_string(),
            available: vec!["baseUrl".to_string(), "token".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("userId"), "message: {msg}");
        assert!(msg.contains("baseUrl"), "message: {msg}");
    }

    #[test]
    fn nesting_too_deep_includes_max() {
        let err = CollectionError::NestingTooDeep { max: 20 };
        assert!(err.to_string().contains("20"), "{}", err);
    }

    #[test]
    fn request_error_interpolation_displays_source() {
        let source = CollectionError::VariableNotFound {
            variable: "token".to_string(),
            available: vec![],
        };
        let err = RequestError::Interpolation(source);
        assert!(err.to_string().contains("interpolation failed"), "{err}");
    }

    #[test]
    fn request_error_timeout_includes_url_and_ms() {
        let err = RequestError::Timeout {
            url: "https://api.example.com".to_string(),
            timeout_ms: 5000,
        };
        let s = err.to_string();
        assert!(s.contains("api.example.com"), "{s}");
        assert!(s.contains("5000"), "{s}");
    }

    #[test]
    fn request_error_too_many_redirects_includes_url() {
        let err = RequestError::TooManyRedirects {
            url: "https://api.example.com".to_string(),
            max_redirects: 10,
        };
        assert!(err.to_string().contains("api.example.com"), "{}", err);
    }

    #[test]
    fn assertion_failure_fields_are_accessible() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Status,
            expected: "200".to_string(),
            actual: "404".to_string(),
        };
        assert_eq!(f.assertion_type, AssertionType::Status);
        assert_eq!(f.expected, "200");
        assert_eq!(f.actual, "404");
    }
}
