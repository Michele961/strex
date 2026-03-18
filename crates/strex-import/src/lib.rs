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
