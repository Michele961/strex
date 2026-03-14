use std::collections::HashMap;
use strex_core::{interpolate, CollectionError};

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn interpolates_full_url_with_base_url_variable() {
    let result = interpolate(
        "{{baseUrl}}/api/v1/users/{{userId}}",
        &vars(&[("baseUrl", "https://api.example.com"), ("userId", "42")]),
    )
    .unwrap();
    assert_eq!(result, "https://api.example.com/api/v1/users/42");
}

#[test]
fn interpolates_header_value() {
    let result = interpolate("Bearer {{token}}", &vars(&[("token", "abc123")])).unwrap();
    assert_eq!(result, "Bearer abc123");
}

#[test]
fn missing_variable_error_lists_available_keys() {
    let err = interpolate(
        "{{missing}}",
        &vars(&[("token", "abc"), ("baseUrl", "https://x.com")]),
    )
    .unwrap_err();
    match err {
        CollectionError::VariableNotFound {
            variable,
            available,
        } => {
            assert_eq!(variable, "missing");
            assert!(available.contains(&"token".to_string()));
            assert!(available.contains(&"baseUrl".to_string()));
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn malformed_placeholder_reports_position() {
    let template = "prefix{{unclosed";
    let err = interpolate(template, &vars(&[])).unwrap_err();
    match err {
        CollectionError::VariableSyntaxError { position, .. } => {
            // "prefix" is 6 bytes, so `{{` starts at byte index 6
            assert_eq!(position, 6, "position should be the index of opening `{{`");
        }
        other => panic!("unexpected error: {other}"),
    }
}
