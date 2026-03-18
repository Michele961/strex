#![deny(missing_docs)]
#![deny(clippy::all)]
//! Conversion utilities that generate Strex YAML collections from external sources.

mod curl;
mod error;
mod openapi;

pub use error::ImportError;

/// Whether to include assertions in the generated collection.
#[derive(Clone, Copy)]
pub enum ImportMode {
    /// Generate method, URL, headers, and body only — no assertions.
    Scaffold,
    /// Generate requests plus basic assertions derived from the source.
    WithTests,
}

/// Parse a curl command and return a Strex YAML collection string.
///
/// Sensitive header and body values are replaced with `{{variable}}` placeholders.
///
/// # Errors
///
/// Returns [`ImportError::CurlParse`] if the input is not a valid curl command
/// (e.g. does not start with `curl`, contains no URL).
pub fn from_curl(input: &str, mode: ImportMode) -> Result<String, ImportError> {
    curl::convert(input, mode)
}

/// Convert an OpenAPI/Swagger spec (as a YAML or JSON string) and return a Strex YAML collection string.
///
/// Accepts both YAML and JSON input — `serde_yaml::from_str` handles both formats
/// since JSON is a valid subset of YAML; no separate JSON branch is required.
///
/// # Errors
///
/// Returns [`ImportError::OpenApiParse`] if the input cannot be parsed as YAML/JSON.
/// Returns [`ImportError::UnrecognisedFormat`] if neither `openapi` nor `swagger` key is found.
pub fn from_openapi(spec: &str, mode: ImportMode) -> Result<String, ImportError> {
    openapi::convert(spec, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_curl_basic() {
        let result = from_curl("curl https://example.com", ImportMode::Scaffold);
        assert!(result.is_ok());
        let yaml = result.unwrap();
        assert!(yaml.contains("GET"));
        assert!(yaml.contains("https://example.com"));
    }

    #[test]
    fn from_openapi_stub_returns_error() {
        assert!(from_openapi("openapi: \"3.0.0\"", ImportMode::Scaffold).is_err());
    }
}
