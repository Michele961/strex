use std::path::Path;

use crate::collection::Collection;
use crate::error::CollectionError;

const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10 MB
const MAX_NESTING_DEPTH: usize = 20;

/// Parses a Strex YAML collection file into a validated [`Collection`].
///
/// # Errors
///
/// Returns [`CollectionError::FileRead`] if the file cannot be opened.
/// Returns [`CollectionError::FileTooLarge`] if the file exceeds 10 MB.
/// Returns [`CollectionError::AnchorsNotAllowed`] or [`CollectionError::AliasesNotAllowed`]
/// if YAML anchors or aliases are present.
/// Returns [`CollectionError::DuplicateKey`] if a YAML key appears twice at the same level.
/// Returns [`CollectionError::YamlParse`] if the YAML is syntactically invalid.
/// Returns [`CollectionError::NestingTooDeep`] if the document exceeds 20 nesting levels.
/// Returns [`CollectionError::UnknownField`] or [`CollectionError::MissingField`] for
/// schema violations — with a "did you mean?" suggestion when the field name is close.
pub fn parse_collection(path: &Path) -> Result<Collection, CollectionError> {
    // Stage 1: raw string pre-validation
    let raw = std::fs::read_to_string(path).map_err(|e| CollectionError::FileRead {
        path: path.to_owned(),
        cause: e.to_string(),
    })?;

    validate_file_size(&raw)?;
    scan_for_anchors_aliases(&raw)?;
    scan_for_duplicate_keys(&raw)?;

    // Stage 2: parse to Value + depth check
    let value: serde_yaml::Value =
        serde_yaml::from_str(&raw).map_err(|e| CollectionError::YamlParse {
            cause: e.to_string(),
        })?;

    validate_max_depth(&value, 0)?;

    // Stages 3 + 4: typed deserialization + error enrichment
    serde_yaml::from_value(value).map_err(enrich_serde_error)
}

/// Rejects files larger than [`MAX_FILE_SIZE`] bytes.
fn validate_file_size(raw: &str) -> Result<(), CollectionError> {
    let size = raw.len();
    if size > MAX_FILE_SIZE {
        Err(CollectionError::FileTooLarge { size_bytes: size })
    } else {
        Ok(())
    }
}

/// Scans the raw YAML string for anchor (`&name`) or alias (`*name`) tokens.
///
/// Splits each non-comment line on whitespace and checks whether any token
/// starts with `&` or `*` followed by at least one character. This approach
/// correctly ignores `*` that appears in the middle of a URL token.
///
/// # Limitations
///
/// Inline comments (`# text` after a value on the same line) and `&`/`*` characters
/// inside quoted strings are not stripped before scanning. Avoid placing `&` or `*`
/// characters in inline comments or unquoted glob patterns in collection files.
///
/// Lines within YAML block scalars (`|`, `>`) are not detected as scalar body
/// content and may produce false-positive errors if the block content contains
/// lines starting with `&` or `*` (e.g., `&copy; 2024` in a text body).
/// Avoid `&` and `*` at the start of block-scalar content lines.
fn scan_for_anchors_aliases(raw: &str) -> Result<(), CollectionError> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        for token in trimmed.split_whitespace() {
            // Strip leading list/flow indicators that can prefix a token in YAML
            let token = token.trim_start_matches(['-', '[', '{']);
            if token.starts_with('&') && token.len() > 1 {
                return Err(CollectionError::AnchorsNotAllowed);
            }
            if token.starts_with('*') && token.len() > 1 {
                return Err(CollectionError::AliasesNotAllowed);
            }
        }
    }
    Ok(())
}

/// Scans the raw YAML for duplicate mapping keys at the same indentation level.
///
/// Uses an indent-based stack to track which keys have been seen at each level.
///
/// # Limitations
///
/// Block scalar content (lines following `|` or `>`) is not detected as scalar body
/// and may produce false-positive duplicate-key errors if the block content contains
/// repeated colon-containing lines at the same indentation. Current collection fields
/// do not use multi-line block scalars in practice.
fn scan_for_duplicate_keys(raw: &str) -> Result<(), CollectionError> {
    // Stack entries: (indent_level, keys_seen_at_this_level)
    let mut stack: Vec<(usize, Vec<String>)> = vec![(0, Vec::new())];

    for line in raw.lines() {
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        // Pop stack levels that are strictly deeper than current indent
        while stack.len() > 1 {
            match stack.last() {
                Some((lvl, _)) if *lvl > indent => {
                    stack.pop();
                }
                _ => break,
            }
        }

        if let Some(key) = extract_mapping_key(trimmed) {
            if let Some(entry) = stack.iter_mut().find(|(lvl, _)| *lvl == indent) {
                if entry.1.contains(&key) {
                    return Err(CollectionError::DuplicateKey { key });
                }
                entry.1.push(key);
            } else {
                stack.push((indent, vec![key]));
            }
        }
    }
    Ok(())
}

/// Extracts a bare mapping key from a YAML line, or `None` for non-key lines.
///
/// Handles list items (`- key: value`) by stripping the `- ` prefix first.
/// Ignores colons inside single- or double-quoted strings.
fn extract_mapping_key(trimmed: &str) -> Option<String> {
    let content = if let Some(stripped) = trimmed.strip_prefix("- ") {
        stripped.trim()
    } else if trimmed == "-" {
        return None;
    } else {
        trimmed
    };

    let mut in_single = false;
    let mut in_double = false;
    for (i, ch) in content.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ':' if !in_single && !in_double => {
                if i == 0 {
                    return None;
                }
                let key = content[..i]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if key.is_empty() {
                    return None;
                }
                return Some(key);
            }
            _ => {}
        }
    }
    None
}

/// Recursively validates that no node in `value` is reached at a depth greater
/// than [`MAX_NESTING_DEPTH`].
///
/// `depth` is 0 at the document root. The guard fires for any value type —
/// including scalar leaves — so a scalar at depth 21 inside a mapping at depth 20
/// is correctly rejected.
fn validate_max_depth(value: &serde_yaml::Value, depth: usize) -> Result<(), CollectionError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(CollectionError::NestingTooDeep {
            max: MAX_NESTING_DEPTH,
        });
    }
    match value {
        serde_yaml::Value::Mapping(m) => {
            for (_, v) in m {
                validate_max_depth(v, depth + 1)?;
            }
        }
        serde_yaml::Value::Sequence(s) => {
            for v in s {
                validate_max_depth(v, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Converts a serde deserialization error into a typed [`CollectionError`].
///
/// Parses the serde error message for known patterns (`unknown field`, `missing field`)
/// and enriches unknown-field errors with a Jaro-Winkler "did you mean?" suggestion.
fn enrich_serde_error(e: serde_yaml::Error) -> CollectionError {
    let msg = e.to_string();

    if let Some(field) = extract_unknown_field(&msg) {
        let valid = extract_valid_fields(&msg);
        let suggestion = find_closest_field(&field, &valid);
        return CollectionError::UnknownField { field, suggestion };
    }

    if let Some(field) = extract_missing_field(&msg) {
        return CollectionError::MissingField { field };
    }

    CollectionError::YamlParse { cause: msg }
}

/// Extracts the unknown field name from a serde error message.
///
/// Looks for the pattern `unknown field \`fieldname\``.
fn extract_unknown_field(msg: &str) -> Option<String> {
    let prefix = "unknown field `";
    let start = msg.find(prefix)? + prefix.len();
    let end = start + msg[start..].find('`')?;
    Some(msg[start..end].to_string())
}

/// Extracts the list of valid field names from a serde error message.
///
/// Looks for the pattern `expected one of \`f1\`, \`f2\`, ...`.
fn extract_valid_fields(msg: &str) -> Vec<String> {
    let prefix = "expected one of ";
    let Some(pos) = msg.find(prefix) else {
        return vec![];
    };
    let rest = &msg[pos + prefix.len()..];
    rest.split(", ")
        .filter_map(|s| {
            let s = s.trim().trim_matches('`');
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
        .collect()
}

/// Extracts the missing field name from a serde error message.
///
/// Looks for the pattern `missing field \`fieldname\``.
fn extract_missing_field(msg: &str) -> Option<String> {
    let prefix = "missing field `";
    let start = msg.find(prefix)? + prefix.len();
    let end = start + msg[start..].find('`')?;
    Some(msg[start..end].to_string())
}

/// Returns the closest match in `valid_fields` to `field` using Jaro-Winkler similarity.
///
/// Returns `None` if no field exceeds the 0.8 similarity threshold.
fn find_closest_field(field: &str, valid_fields: &[String]) -> Option<String> {
    valid_fields
        .iter()
        .map(|f| (f, strsim::jaro_winkler(field, f.as_str())))
        .filter(|(_, score)| *score > 0.8)
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(f, _)| f.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_file_size ---

    #[test]
    fn file_size_ok_when_under_limit() {
        let small = "x".repeat(100);
        assert!(validate_file_size(&small).is_ok());
    }

    #[test]
    fn file_size_rejected_when_over_10mb() {
        let big = "x".repeat(10 * 1024 * 1024 + 1);
        let err = validate_file_size(&big).unwrap_err();
        assert!(matches!(
            err,
            crate::error::CollectionError::FileTooLarge { .. }
        ));
    }

    // --- scan_for_anchors_aliases ---

    #[test]
    fn anchor_token_is_rejected() {
        let yaml = "key: &anchor value\n";
        let err = scan_for_anchors_aliases(yaml).unwrap_err();
        assert!(matches!(
            err,
            crate::error::CollectionError::AnchorsNotAllowed
        ));
    }

    #[test]
    fn alias_token_is_rejected() {
        let yaml = "key: *alias\n";
        let err = scan_for_anchors_aliases(yaml).unwrap_err();
        assert!(matches!(
            err,
            crate::error::CollectionError::AliasesNotAllowed
        ));
    }

    #[test]
    fn url_with_wildcard_is_not_rejected() {
        // A URL like https://api.example.com/* must NOT trigger alias detection
        // because the token "https://api.example.com/*" does not START with '*'
        let yaml = "url: \"https://api.example.com/*\"\n";
        assert!(scan_for_anchors_aliases(yaml).is_ok());
    }

    #[test]
    fn comment_line_is_skipped() {
        let yaml = "# &not_an_anchor\nkey: value\n";
        assert!(scan_for_anchors_aliases(yaml).is_ok());
    }

    // --- scan_for_duplicate_keys ---

    #[test]
    fn duplicate_key_at_same_indent_is_rejected() {
        let yaml = "name: foo\nname: bar\n";
        let err = scan_for_duplicate_keys(yaml).unwrap_err();
        assert!(matches!(
            err,
            crate::error::CollectionError::DuplicateKey { key } if key == "name"
        ));
    }

    #[test]
    fn same_key_at_different_indent_levels_is_ok() {
        // 'name' appears at the top-level and inside a nested mapping — not a duplicate
        let yaml = "name: outer\nchild:\n  name: inner\n";
        assert!(scan_for_duplicate_keys(yaml).is_ok());
    }

    // --- validate_max_depth ---

    #[test]
    fn shallow_value_is_ok() {
        let value: serde_yaml::Value = serde_yaml::from_str("key: value").unwrap();
        assert!(validate_max_depth(&value, 0).is_ok());
    }

    #[test]
    fn value_at_exactly_max_depth_is_ok() {
        // 19 nested mappings (l0..l18): root mapping at depth 0, leaf scalar at depth 20.
        // The guard is `depth > 20`, so depth 20 is allowed.
        let mut yaml = String::new();
        for i in 0..19 {
            yaml.push_str(&"  ".repeat(i));
            yaml.push_str(&format!("l{i}:\n"));
        }
        yaml.push_str(&"  ".repeat(19));
        yaml.push_str("leaf: value\n");
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        assert!(validate_max_depth(&value, 0).is_ok());
    }

    #[test]
    fn value_exceeding_max_depth_is_rejected() {
        // 20 nested mappings (l0..l19): deepest container at depth 20, scalar "value" at depth 21.
        // Mirrors the nesting_too_deep.yaml fixture — the scalar leaf must be caught.
        let mut yaml = String::new();
        for i in 0..20 {
            yaml.push_str(&"  ".repeat(i));
            yaml.push_str(&format!("l{i}:\n"));
        }
        yaml.push_str(&"  ".repeat(20));
        yaml.push_str("leaf: value\n");
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        let err = validate_max_depth(&value, 0).unwrap_err();
        assert!(matches!(
            err,
            crate::error::CollectionError::NestingTooDeep { .. }
        ));
    }

    // --- error enrichment helpers ---

    #[test]
    fn extract_unknown_field_returns_field_name() {
        let msg = "unknown field `metod`, expected one of `name`, `method`, `url`";
        assert_eq!(extract_unknown_field(msg), Some("metod".to_string()));
    }

    #[test]
    fn extract_valid_fields_returns_all_fields() {
        let msg = "unknown field `metod`, expected one of `name`, `method`, `url`";
        assert_eq!(
            extract_valid_fields(msg),
            vec!["name".to_string(), "method".to_string(), "url".to_string()]
        );
    }

    #[test]
    fn find_closest_field_returns_suggestion_above_threshold() {
        let valid = vec!["method".to_string(), "url".to_string()];
        assert_eq!(
            find_closest_field("metod", &valid),
            Some("method".to_string())
        );
    }

    #[test]
    fn find_closest_field_returns_none_below_threshold() {
        let valid = vec!["method".to_string(), "url".to_string()];
        assert!(find_closest_field("zzz", &valid).is_none());
    }
}
