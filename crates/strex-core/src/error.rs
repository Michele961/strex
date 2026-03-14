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
}
