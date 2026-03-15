use std::collections::HashMap;

use serde_json_path::JsonPath;

use crate::error::{AssertionFailure, AssertionType, RequestError};
use crate::http::HttpResponse;
use crate::interpolation::interpolate;

// Allow dead_code: these types are consumed by evaluate(), which is pub(crate) and
// called from runner.rs (Task 6) — a stub at this stage.
#[allow(dead_code)]
enum Assertion {
    Status(u16),
    JsonPath { path: String, check: JsonPathCheck },
    Header { name: String, check: HeaderCheck },
}

#[allow(dead_code)]
enum JsonPathCheck {
    Exists(bool),
    Equals(String),
    Contains(String),
}

#[allow(dead_code)]
enum HeaderCheck {
    Exists(bool),
    Equals(String),
    Contains(String),
}

/// Evaluate declarative assertions against an HTTP response.
///
/// Each assertion map is parsed from the raw YAML and checked against `response`.
/// String values in assertion maps (e.g. `equals: "{{token}}"`) are interpolated using `vars`
/// — the flat map from `ExecutionContext::resolve_all()`.
///
/// Returns `Ok(failures)` — a possibly-empty list. Execution always continues on `Ok`.
/// Returns `Err(RequestError::InvalidAssertion)` for malformed assertion maps.
/// Returns `Err(RequestError::Interpolation)` if an assertion expected-value variable is unresolved.
// Allow dead_code: called from runner.rs (Task 6) which is a stub at this stage.
#[allow(dead_code)]
pub(crate) fn evaluate(
    raw: &[HashMap<String, serde_yaml::Value>],
    response: &HttpResponse,
    vars: &HashMap<String, String>,
) -> Result<Vec<AssertionFailure>, RequestError> {
    let mut failures = Vec::new();
    for map in raw {
        let assertion = parse_assertion_map(map, vars)?;
        if let Some(failure) = check_assertion(&assertion, response) {
            failures.push(failure);
        }
    }
    Ok(failures)
}

#[allow(dead_code)]
fn parse_assertion_map(
    map: &HashMap<String, serde_yaml::Value>,
    vars: &HashMap<String, String>,
) -> Result<Assertion, RequestError> {
    if let Some(val) = map.get("status") {
        return Ok(Assertion::Status(parse_status_value(val)?));
    }
    if let Some(path_val) = map.get("jsonPath") {
        let path = path_val
            .as_str()
            .ok_or_else(|| RequestError::InvalidAssertion {
                cause: "jsonPath value must be a string".to_string(),
            })?
            .to_string();
        let check = parse_jsonpath_check(map, vars)?;
        return Ok(Assertion::JsonPath { path, check });
    }
    if let Some(name_val) = map.get("header") {
        let name = name_val
            .as_str()
            .ok_or_else(|| RequestError::InvalidAssertion {
                cause: "header value must be a string".to_string(),
            })?
            .to_lowercase();
        let check = parse_header_check(map, vars)?;
        return Ok(Assertion::Header { name, check });
    }
    let keys: Vec<&str> = map.keys().map(String::as_str).collect();
    Err(RequestError::InvalidAssertion {
        cause: format!("unknown assertion key(s): {keys:?}"),
    })
}

#[allow(dead_code)]
fn parse_status_value(val: &serde_yaml::Value) -> Result<u16, RequestError> {
    match val {
        serde_yaml::Value::Number(n) => {
            n.as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or_else(|| RequestError::InvalidAssertion {
                    cause: format!("status value '{n}' is not a valid HTTP status code (0–65535)"),
                })
        }
        serde_yaml::Value::String(s) => {
            s.parse::<u16>()
                .map_err(|_| RequestError::InvalidAssertion {
                    cause: format!("status value '{s}' is not a valid HTTP status code"),
                })
        }
        other => Err(RequestError::InvalidAssertion {
            cause: format!("status value must be a number or numeric string, got: {other:?}"),
        }),
    }
}

#[allow(dead_code)]
fn parse_jsonpath_check(
    map: &HashMap<String, serde_yaml::Value>,
    vars: &HashMap<String, String>,
) -> Result<JsonPathCheck, RequestError> {
    if let Some(v) = map.get("exists") {
        let b = v.as_bool().ok_or_else(|| RequestError::InvalidAssertion {
            cause: "exists value must be a boolean".to_string(),
        })?;
        return Ok(JsonPathCheck::Exists(b));
    }
    if let Some(v) = map.get("equals") {
        let s = yaml_scalar_to_string(v);
        let resolved = interpolate(&s, vars).map_err(RequestError::Interpolation)?;
        return Ok(JsonPathCheck::Equals(resolved));
    }
    if let Some(v) = map.get("contains") {
        let s = yaml_scalar_to_string(v);
        let resolved = interpolate(&s, vars).map_err(RequestError::Interpolation)?;
        return Ok(JsonPathCheck::Contains(resolved));
    }
    Err(RequestError::InvalidAssertion {
        cause: "jsonPath assertion must have one of: exists, equals, contains".to_string(),
    })
}

#[allow(dead_code)]
fn parse_header_check(
    map: &HashMap<String, serde_yaml::Value>,
    vars: &HashMap<String, String>,
) -> Result<HeaderCheck, RequestError> {
    if let Some(v) = map.get("exists") {
        let b = v.as_bool().ok_or_else(|| RequestError::InvalidAssertion {
            cause: "exists value must be a boolean".to_string(),
        })?;
        return Ok(HeaderCheck::Exists(b));
    }
    if let Some(v) = map.get("equals") {
        let s = yaml_scalar_to_string(v);
        let resolved = interpolate(&s, vars).map_err(RequestError::Interpolation)?;
        return Ok(HeaderCheck::Equals(resolved));
    }
    if let Some(v) = map.get("contains") {
        let s = yaml_scalar_to_string(v);
        let resolved = interpolate(&s, vars).map_err(RequestError::Interpolation)?;
        return Ok(HeaderCheck::Contains(resolved));
    }
    Err(RequestError::InvalidAssertion {
        cause: "header assertion must have one of: exists, equals, contains".to_string(),
    })
}

#[allow(dead_code)]
fn yaml_scalar_to_string(val: &serde_yaml::Value) -> String {
    match val {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        other => format!("{other:?}"),
    }
}

#[allow(dead_code)]
fn check_assertion(assertion: &Assertion, response: &HttpResponse) -> Option<AssertionFailure> {
    match assertion {
        Assertion::Status(expected) => {
            if response.status != *expected {
                Some(AssertionFailure {
                    assertion_type: AssertionType::Status,
                    expected: expected.to_string(),
                    actual: response.status.to_string(),
                })
            } else {
                None
            }
        }
        Assertion::JsonPath { path, check } => check_json_path(path, check, response),
        Assertion::Header { name, check } => check_header(name, check, response),
    }
}

#[allow(dead_code)]
fn check_json_path(
    path: &str,
    check: &JsonPathCheck,
    response: &HttpResponse,
) -> Option<AssertionFailure> {
    let json_value: serde_json::Value = match serde_json::from_str(&response.body) {
        Ok(v) => v,
        Err(_) => {
            return Some(AssertionFailure {
                assertion_type: AssertionType::JsonPath,
                expected: "valid JSON body".to_string(),
                actual: "<body is not valid JSON>".to_string(),
            });
        }
    };

    let compiled = match JsonPath::parse(path) {
        Ok(p) => p,
        Err(e) => {
            return Some(AssertionFailure {
                assertion_type: AssertionType::JsonPath,
                expected: format!("valid JSONPath '{path}'"),
                actual: format!("invalid JSONPath: {e}"),
            });
        }
    };

    let node_list = compiled.query(&json_value);

    match check {
        JsonPathCheck::Exists(expected) => {
            let found = !node_list.is_empty();
            if found != *expected {
                Some(AssertionFailure {
                    assertion_type: AssertionType::JsonPath,
                    expected: format!("exists={expected}"),
                    actual: format!("exists={found}"),
                })
            } else {
                None
            }
        }
        JsonPathCheck::Equals(expected) => {
            let actual = node_list
                .first()
                .map(json_value_to_comparable)
                .unwrap_or_default();
            if actual != *expected {
                Some(AssertionFailure {
                    assertion_type: AssertionType::JsonPath,
                    expected: expected.clone(),
                    actual,
                })
            } else {
                None
            }
        }
        JsonPathCheck::Contains(expected) => {
            let first = match node_list.first() {
                Some(v) => v,
                None => {
                    return Some(AssertionFailure {
                        assertion_type: AssertionType::JsonPath,
                        expected: format!("contains '{expected}'"),
                        actual: "<no match>".to_string(),
                    });
                }
            };
            let matches = match first {
                serde_json::Value::String(s) => s.contains(expected.as_str()),
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .any(|el| json_value_to_comparable(el) == *expected),
                _ => false,
            };
            if !matches {
                Some(AssertionFailure {
                    assertion_type: AssertionType::JsonPath,
                    expected: format!("contains '{expected}'"),
                    actual: json_value_to_comparable(first),
                })
            } else {
                None
            }
        }
    }
}

/// Extract a comparable string from a JSON value.
/// For String values, return the raw string (not JSON-encoded).
/// For all other types, use serde_json's Display (to_string).
#[allow(dead_code)]
fn json_value_to_comparable(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[allow(dead_code)]
fn check_header(
    name: &str,
    check: &HeaderCheck,
    response: &HttpResponse,
) -> Option<AssertionFailure> {
    let value = response.headers.get(name);
    match check {
        HeaderCheck::Exists(expected) => {
            let found = value.is_some();
            if found != *expected {
                Some(AssertionFailure {
                    assertion_type: AssertionType::Header,
                    expected: format!("header '{name}' exists={expected}"),
                    actual: format!("header '{name}' exists={found}"),
                })
            } else {
                None
            }
        }
        HeaderCheck::Equals(expected) => {
            let actual = value.map(String::as_str).unwrap_or("");
            if actual != expected.as_str() {
                Some(AssertionFailure {
                    assertion_type: AssertionType::Header,
                    expected: expected.clone(),
                    actual: actual.to_string(),
                })
            } else {
                None
            }
        }
        HeaderCheck::Contains(expected) => {
            let actual = value.map(String::as_str).unwrap_or("");
            if !actual.contains(expected.as_str()) {
                Some(AssertionFailure {
                    assertion_type: AssertionType::Header,
                    expected: format!("contains '{expected}'"),
                    actual: actual.to_string(),
                })
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            headers: HashMap::new(),
            body: body.to_string(),
        }
    }

    fn response_with_header(status: u16, name: &str, value: &str) -> HttpResponse {
        let mut headers = HashMap::new();
        headers.insert(name.to_lowercase(), value.to_string());
        HttpResponse {
            status,
            headers,
            body: String::new(),
        }
    }

    fn status_map(code: u64) -> HashMap<String, serde_yaml::Value> {
        HashMap::from([("status".to_string(), serde_yaml::Value::Number(code.into()))])
    }

    fn jsonpath_map(
        path: &str,
        op: &str,
        val: serde_yaml::Value,
    ) -> HashMap<String, serde_yaml::Value> {
        HashMap::from([
            (
                "jsonPath".to_string(),
                serde_yaml::Value::String(path.to_string()),
            ),
            (op.to_string(), val),
        ])
    }

    fn header_map(
        name: &str,
        op: &str,
        val: serde_yaml::Value,
    ) -> HashMap<String, serde_yaml::Value> {
        HashMap::from([
            (
                "header".to_string(),
                serde_yaml::Value::String(name.to_string()),
            ),
            (op.to_string(), val),
        ])
    }

    fn empty_vars() -> HashMap<String, String> {
        HashMap::new()
    }

    // --- Status ---

    #[test]
    fn status_assertion_passes_on_match() {
        let raw = vec![status_map(200)];
        let resp = response(200, "");
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty());
    }

    #[test]
    fn status_assertion_fails_on_mismatch() {
        let raw = vec![status_map(200)];
        let resp = response(404, "");
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].assertion_type, AssertionType::Status);
        assert_eq!(failures[0].expected, "200");
        assert_eq!(failures[0].actual, "404");
    }

    #[test]
    fn status_string_value_is_accepted() {
        let raw = vec![HashMap::from([(
            "status".to_string(),
            serde_yaml::Value::String("200".to_string()),
        )])];
        let resp = response(200, "");
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty());
    }

    #[test]
    fn status_non_numeric_returns_invalid_assertion_error() {
        let raw = vec![HashMap::from([(
            "status".to_string(),
            serde_yaml::Value::String("ok".to_string()),
        )])];
        let resp = response(200, "");
        let err = evaluate(&raw, &resp, &empty_vars()).unwrap_err();
        assert!(
            matches!(err, RequestError::InvalidAssertion { .. }),
            "{err:?}"
        );
    }

    // --- JsonPath ---

    #[test]
    fn jsonpath_exists_true_passes_when_found() {
        let raw = vec![jsonpath_map(
            "$.id",
            "exists",
            serde_yaml::Value::Bool(true),
        )];
        let resp = response(200, r#"{"id": 1}"#);
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty());
    }

    #[test]
    fn jsonpath_exists_true_fails_when_not_found() {
        let raw = vec![jsonpath_map(
            "$.missing",
            "exists",
            serde_yaml::Value::Bool(true),
        )];
        let resp = response(200, r#"{"id": 1}"#);
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].assertion_type, AssertionType::JsonPath);
    }

    #[test]
    fn jsonpath_equals_number_as_string() {
        // JSON number 1 matches equals: "1"
        let raw = vec![jsonpath_map(
            "$.id",
            "equals",
            serde_yaml::Value::String("1".to_string()),
        )];
        let resp = response(200, r#"{"id": 1}"#);
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn jsonpath_equals_string_value() {
        // JSON string "John" matches equals: "John" (raw string, not JSON-encoded)
        let raw = vec![jsonpath_map(
            "$.name",
            "equals",
            serde_yaml::Value::String("John".to_string()),
        )];
        let resp = response(200, r#"{"name": "John"}"#);
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn jsonpath_equals_fails_on_mismatch() {
        let raw = vec![jsonpath_map(
            "$.id",
            "equals",
            serde_yaml::Value::String("999".to_string()),
        )];
        let resp = response(200, r#"{"id": 1}"#);
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn jsonpath_contains_string_substring() {
        let raw = vec![jsonpath_map(
            "$.email",
            "contains",
            serde_yaml::Value::String("@example.com".to_string()),
        )];
        let resp = response(200, r#"{"email": "user@example.com"}"#);
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn jsonpath_contains_array_element() {
        let raw = vec![jsonpath_map(
            "$.tags",
            "contains",
            serde_yaml::Value::String("admin".to_string()),
        )];
        let resp = response(200, r#"{"tags": ["user", "admin", "editor"]}"#);
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn jsonpath_non_json_body_produces_failure_not_error() {
        let raw = vec![jsonpath_map(
            "$.id",
            "exists",
            serde_yaml::Value::Bool(true),
        )];
        let resp = response(200, "not json");
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert_eq!(failures.len(), 1);
        assert!(
            failures[0].actual.contains("not valid JSON"),
            "{}",
            failures[0].actual
        );
    }

    // --- Header ---

    #[test]
    fn header_contains_passes() {
        let raw = vec![header_map(
            "content-type",
            "contains",
            serde_yaml::Value::String("application/json".to_string()),
        )];
        let resp = response_with_header(200, "content-type", "application/json; charset=utf-8");
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn header_name_is_case_insensitive() {
        // Assertion uses "Content-Type" (mixed case), map stores "content-type" (lowercase)
        let raw = vec![header_map(
            "Content-Type",
            "equals",
            serde_yaml::Value::String("application/json".to_string()),
        )];
        let resp = response_with_header(200, "content-type", "application/json");
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn header_exists_false_passes_when_absent() {
        let raw = vec![header_map(
            "x-rate-limit",
            "exists",
            serde_yaml::Value::Bool(false),
        )];
        let resp = response(200, "");
        let failures = evaluate(&raw, &resp, &empty_vars()).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    // --- Interpolation in assertion values ---

    #[test]
    fn assertion_expected_value_is_interpolated() {
        let raw = vec![jsonpath_map(
            "$.name",
            "equals",
            serde_yaml::Value::String("{{expected_name}}".to_string()),
        )];
        let resp = response(200, r#"{"name": "Alice"}"#);
        let vars = HashMap::from([("expected_name".to_string(), "Alice".to_string())]);
        let failures = evaluate(&raw, &resp, &vars).unwrap();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn missing_interpolation_variable_returns_error() {
        let raw = vec![jsonpath_map(
            "$.name",
            "equals",
            serde_yaml::Value::String("{{missing_var}}".to_string()),
        )];
        let resp = response(200, r#"{"name": "Alice"}"#);
        let err = evaluate(&raw, &resp, &empty_vars()).unwrap_err();
        assert!(matches!(err, RequestError::Interpolation(_)), "{err:?}");
    }

    // --- Unknown assertion key ---

    #[test]
    fn unknown_assertion_key_returns_invalid_assertion_error() {
        let raw = vec![HashMap::from([(
            "badKey".to_string(),
            serde_yaml::Value::String("value".to_string()),
        )])];
        let resp = response(200, "");
        let err = evaluate(&raw, &resp, &empty_vars()).unwrap_err();
        assert!(
            matches!(err, RequestError::InvalidAssertion { .. }),
            "{err:?}"
        );
    }
}
