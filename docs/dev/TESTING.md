# Testing Conventions

## Test Levels

### 1. Unit Tests

**Location:** `#[cfg(test)]` module at the bottom of the same file being tested.

**Purpose:** Test one function or struct in isolation. Mock or stub dependencies at the boundary.

```rust
// crates/strex-core/src/interpolation.rs

pub fn interpolate(
    template: &str,
    variables: &HashMap<String, String>,
) -> Result<String, CollectionError> {
    // ...
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolates_known_variable() {
        let mut vars = HashMap::new();
        vars.insert("token".into(), "abc123".into());
        let result = interpolate("Bearer {{token}}", &vars).unwrap();
        assert_eq!(result, "Bearer abc123");
    }

    #[test]
    fn test_returns_error_for_unknown_variable() {
        let vars = HashMap::new();
        let err = interpolate("{{missing}}", &vars).unwrap_err();
        assert!(matches!(err, CollectionError::VariableNotFound { variable, .. } if variable == "missing"));
    }

    #[test]
    fn test_leaves_plain_string_unchanged() {
        let vars = HashMap::new();
        let result = interpolate("no variables here", &vars).unwrap();
        assert_eq!(result, "no variables here");
    }
}
```

### 2. Integration Tests

**Location:** `tests/` directory inside the relevant crate (e.g., `crates/strex-core/tests/`).

**Purpose:** Test a full crate boundary end-to-end. For example: read a YAML fixture file → parse → validate → produce a `Collection` struct.

```rust
// crates/strex-core/tests/parser_integration.rs

#[test]
fn test_parses_valid_collection_file() {
    let collection = strex_core::parse_collection(
        std::path::Path::new("tests/fixtures/valid.yaml")
    ).expect("should parse valid collection");

    assert_eq!(collection.name, "GitHub API Tests");
    assert_eq!(collection.requests.len(), 2);
    assert_eq!(collection.requests[0].method, "GET");
}

#[test]
fn test_rejects_collection_with_yaml_anchors() {
    let err = strex_core::parse_collection(
        std::path::Path::new("tests/fixtures/anchors.yaml")
    ).unwrap_err();

    assert!(matches!(err, strex_core::CollectionError::AnchorsNotAllowed { .. }));
}

#[test]
fn test_rejects_unknown_field_with_suggestion() {
    let err = strex_core::parse_collection(
        std::path::Path::new("tests/fixtures/typo_field.yaml")
    ).unwrap_err();

    assert!(matches!(err, strex_core::CollectionError::UnknownField { field, .. } if field == "metod"));
}
```

Add YAML fixture files to `crates/strex-core/tests/fixtures/`. Each fixture should be minimal — only what the test needs.

### 3. End-to-End Tests (E2E)

**Location:** `crates/strex-cli/tests/`

**Purpose:** Spin up a `wiremock` mock HTTP server, run the compiled `strex` binary, assert on exit code and stdout/stderr output. This is the primary way to validate functional behavior.

```rust
// crates/strex-cli/tests/e2e_run.rs

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_successful_get_request_exits_zero() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/users/1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": 1, "login": "octocat"}))
        )
        .mount(&server)
        .await;

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_strex"))
        .args([
            "run",
            "tests/fixtures/get_user.yaml",
            "--env",
            &format!("BASE_URL={}", server.uri()),
        ])
        .status()
        .unwrap();

    assert_eq!(status.code(), Some(0));
}

#[tokio::test]
async fn test_status_assertion_failure_exits_one() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_strex"))
        .args([
            "run",
            "tests/fixtures/get_user.yaml",
            "--env",
            &format!("BASE_URL={}", server.uri()),
        ])
        .status()
        .unwrap();

    assert_eq!(status.code(), Some(1));
}

#[tokio::test]
async fn test_network_error_exits_two() {
    // Point to a port where nothing is listening
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_strex"))
        .args([
            "run",
            "tests/fixtures/get_user.yaml",
            "--env",
            "BASE_URL=http://127.0.0.1:1",  // port 1 is never open
        ])
        .status()
        .unwrap();

    assert_eq!(status.code(), Some(2));
}
```

## Mock HTTP Server Convention

- Use the `wiremock` crate.
- Each test creates its own `MockServer::start().await` — no shared server state between tests.
- Servers are automatically cleaned up when dropped at the end of the test scope.
- Add `wiremock` and `tokio` to `[dev-dependencies]` only, never `[dependencies]`:

```toml
# crates/strex-cli/Cargo.toml
[dev-dependencies]
# Mock HTTP server for E2E tests
wiremock = "0.5"
# Async test runtime for E2E tests
tokio = { version = "1", features = ["full"] }
```

## Test Naming

Use descriptive names that read as sentences describing the behavior under test:

```rust
// Good
fn test_dns_resolution_failure_returns_exit_code_2() {}
fn test_variable_set_in_post_script_is_available_in_next_request() {}
fn test_concurrent_iterations_do_not_share_collection_variable_state() {}
fn test_missing_required_field_reports_field_name_in_error() {}

// Bad
fn test_dns() {}
fn test_variables() {}
fn test_error() {}
fn test1() {}
```

## What Must Be Tested for Every Feature

1. **Happy path** — the expected successful behavior with valid input.
2. **Error cases** — at least one test per error variant defined in [ADR-0002](../adr/0002-execution-model-and-error-taxonomy.md) that the feature can produce.
3. **Variable isolation** — if the feature touches variable scoping, add a test that runs two iterations and asserts no state leaks from one to the other.

## What NOT to Test

- Do not test Rust stdlib behavior (e.g. that `HashMap::get` returns `None` for missing keys).
- Do not test `serde_yaml`, `reqwest`, or `wiremock` internals — trust the libraries.
- Test your code's behavior (inputs → outputs → errors), not its internal implementation details.

## Running Tests

```bash
# All tests across all crates
cargo test

# Tests in a specific crate only
cargo test -p strex-core

# A specific test by name (substring match)
cargo test test_parses_valid_collection_file

# Integration tests only in a crate
cargo test -p strex-core --test '*'

# E2E tests (requires compiled binary — build first)
cargo build && cargo test -p strex-cli --test '*'

# Show stdout/stderr from tests (useful for debugging)
cargo test -- --nocapture
```

E2E tests use `#[tokio::test]` and require the `tokio` feature in `[dev-dependencies]`.

## Coverage Expectation

Every `pub` function in `strex-core` and `strex-script` must have at least one test. This is a manual convention enforced during code review, not by automated tooling at MVP.

To check coverage locally:

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --open
```
