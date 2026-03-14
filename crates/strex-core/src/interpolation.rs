use std::collections::HashMap;

use crate::error::CollectionError;

/// Resolves `{{variable}}` placeholders in `template` using `variables`.
///
/// Iterates through the template character by character. When `{{` is encountered,
/// reads until `}}`, trims the key, and looks it up in `variables`.
///
/// # Errors
///
/// Returns [`CollectionError::VariableNotFound`] if a placeholder key is absent
/// from `variables`, with a sorted list of available keys.
///
/// Returns [`CollectionError::VariableSyntaxError`] if a `{{` is opened but
/// never closed before the end of the template.
pub fn interpolate(
    template: &str,
    variables: &HashMap<String, String>,
) -> Result<String, CollectionError> {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();

    while let Some((i, ch)) = chars.next() {
        if ch == '{' && chars.peek().map(|(_, c)| *c == '{').unwrap_or(false) {
            let open_pos = i;
            chars.next(); // consume second '{'

            // Collect key characters until `}}`
            let mut key = String::new();
            let mut closed = false;

            while let Some((_, kch)) = chars.next() {
                if kch == '}' {
                    if chars.peek().map(|(_, c)| *c == '}').unwrap_or(false) {
                        chars.next(); // consume second '}'
                        closed = true;
                        break;
                    }
                    // Single '}' — part of the key (unusual but handle it)
                    key.push(kch);
                } else {
                    key.push(kch);
                }
            }

            if !closed {
                return Err(CollectionError::VariableSyntaxError {
                    template: template.to_string(),
                    position: open_pos,
                });
            }

            let key = key.trim();
            match variables.get(key) {
                Some(val) => result.push_str(val),
                None => {
                    let mut available: Vec<String> = variables.keys().cloned().collect();
                    available.sort();
                    return Err(CollectionError::VariableNotFound {
                        variable: key.to_string(),
                        available,
                    });
                }
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn plain_string_passes_through_unchanged() {
        let result = interpolate("hello world", &vars(&[])).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn known_variable_is_replaced() {
        let result = interpolate(
            "{{baseUrl}}/users",
            &vars(&[("baseUrl", "https://api.example.com")]),
        )
        .unwrap();
        assert_eq!(result, "https://api.example.com/users");
    }

    #[test]
    fn multiple_variables_are_replaced() {
        let result = interpolate(
            "{{scheme}}://{{host}}/{{path}}",
            &vars(&[
                ("scheme", "https"),
                ("host", "api.example.com"),
                ("path", "users"),
            ]),
        )
        .unwrap();
        assert_eq!(result, "https://api.example.com/users");
    }

    #[test]
    fn unknown_variable_returns_variable_not_found() {
        let err = interpolate("{{missing}}", &vars(&[("other", "val")])).unwrap_err();
        match err {
            crate::error::CollectionError::VariableNotFound {
                variable,
                available,
            } => {
                assert_eq!(variable, "missing");
                assert!(available.contains(&"other".to_string()));
            }
            other => panic!("expected VariableNotFound, got: {other}"),
        }
    }

    #[test]
    fn variable_not_found_available_list_is_sorted() {
        let err = interpolate("{{x}}", &vars(&[("z", "1"), ("a", "2"), ("m", "3")])).unwrap_err();
        if let crate::error::CollectionError::VariableNotFound { available, .. } = err {
            let mut sorted = available.clone();
            sorted.sort();
            assert_eq!(available, sorted, "available list must be sorted");
        }
    }

    #[test]
    fn unclosed_placeholder_returns_variable_syntax_error() {
        let err = interpolate("hello {{unclosed", &vars(&[])).unwrap_err();
        assert!(
            matches!(
                err,
                crate::error::CollectionError::VariableSyntaxError { .. }
            ),
            "expected VariableSyntaxError, got: {err}"
        );
    }

    #[test]
    fn placeholder_with_whitespace_around_key_is_trimmed() {
        let result = interpolate("{{ baseUrl }}", &vars(&[("baseUrl", "https://x.com")])).unwrap();
        assert_eq!(result, "https://x.com");
    }

    #[test]
    fn empty_string_returns_empty_string() {
        let result = interpolate("", &vars(&[])).unwrap();
        assert_eq!(result, "");
    }
}
