use std::path::PathBuf;
use strex_core::CollectionError;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn valid_yaml_parses_to_collection() {
    let col = strex_core::parse_collection(&fixture("valid.yaml"))
        .expect("valid.yaml should parse without error");
    assert_eq!(col.name, "Example Collection");
    assert_eq!(col.version, "1.0");
    assert_eq!(col.requests.len(), 1);
    assert_eq!(col.requests[0].name, "Get users");
    assert_eq!(col.requests[0].method, "GET");
}

#[test]
fn anchors_yaml_returns_anchors_not_allowed() {
    let err = strex_core::parse_collection(&fixture("anchors.yaml"))
        .expect_err("anchors.yaml must be rejected");
    assert!(
        matches!(err, CollectionError::AnchorsNotAllowed),
        "expected AnchorsNotAllowed, got: {err}"
    );
}

#[test]
fn duplicate_keys_yaml_returns_duplicate_key_method() {
    let err = strex_core::parse_collection(&fixture("duplicate_keys.yaml"))
        .expect_err("duplicate_keys.yaml must be rejected");
    assert!(
        matches!(err, CollectionError::DuplicateKey { ref key } if key == "method"),
        "expected DuplicateKey {{ key: \"method\" }}, got: {err}"
    );
}

#[test]
fn unknown_field_yaml_returns_unknown_field_with_suggestion() {
    let err = strex_core::parse_collection(&fixture("unknown_field.yaml"))
        .expect_err("unknown_field.yaml must be rejected");
    match err {
        CollectionError::UnknownField { field, suggestion } => {
            assert_eq!(field, "metod");
            assert_eq!(
                suggestion.as_deref(),
                Some("method"),
                "expected suggestion 'method' for typo 'metod'"
            );
        }
        other => panic!("expected UnknownField, got: {other}"),
    }
}

#[test]
fn valid_with_body_yaml_parses_body_type_json() {
    let col = strex_core::parse_collection(&fixture("valid_with_body.yaml"))
        .expect("valid_with_body.yaml should parse without error");
    let body = col.requests[0]
        .body
        .as_ref()
        .expect("body should be present");
    assert!(
        matches!(body.body_type, strex_core::BodyType::Json),
        "expected BodyType::Json"
    );
    // Content should have the 'name' key
    let content_map = body
        .content
        .as_mapping()
        .expect("content should be a mapping");
    assert!(
        content_map
            .get(serde_yaml::Value::String("name".to_string()))
            .is_some(),
        "content should have 'name' key"
    );
}

#[test]
fn nesting_too_deep_yaml_returns_nesting_too_deep() {
    let err = strex_core::parse_collection(&fixture("nesting_too_deep.yaml"))
        .expect_err("nesting_too_deep.yaml must be rejected");
    assert!(
        matches!(err, CollectionError::NestingTooDeep { .. }),
        "expected NestingTooDeep, got: {err}"
    );
}
