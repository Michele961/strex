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
