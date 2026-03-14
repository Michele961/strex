# Coding Standards

## Formatting

Use `rustfmt` with default settings. No manual formatting decisions. Do not add a `rustfmt.toml` — the defaults are intentional. Run before every commit:

```bash
cargo fmt --check   # verify (CI uses this)
cargo fmt           # fix
```

## Linting

Clippy is mandatory and warnings are treated as errors. Do not add a `.clippy.toml` without team discussion. Add this to the top of each crate's `lib.rs` or `main.rs`:

```rust
#![deny(clippy::all)]
```

Run before every commit:

```bash
cargo clippy -- -D warnings
```

## Error Handling

### Rule: `thiserror` in library crates, `anyhow` in CLI

| Crate | Approach | Why |
|-------|----------|-----|
| `strex-core` | `thiserror` | Callers need to match on specific error variants |
| `strex-script` | `thiserror` | Callers need to match on specific error variants |
| `strex-cli` | `anyhow` | Top-level: display errors and exit, no typed matching needed |

Define errors matching the taxonomy in [ADR-0002](../adr/0002-execution-model-and-error-taxonomy.md):

```rust
// crates/strex-core/src/error.rs
#[derive(thiserror::Error, Debug)]
pub enum CollectionError {
    #[error("DNS resolution failed for {domain}: {cause}")]
    DnsResolution { domain: String, cause: String },

    #[error("Assertion failed in '{request}': expected {expected}, got {actual}")]
    AssertionFailed {
        request: String,
        expected: String,
        actual: String,
    },

    #[error("Variable '{variable}' not found. Available: {available:?}")]
    VariableNotFound {
        variable: String,
        available: Vec<String>,
    },
    // ... see ADR-0002 for the full taxonomy
}
```

```rust
// crates/strex-cli/src/main.rs
use anyhow::Result;

fn main() -> Result<()> {
    let collection = strex_core::parse("collection.yaml")?;
    // anyhow surfaces errors with Display automatically
    Ok(())
}
```

### Never `unwrap()` or `expect()` in non-test code

```rust
// Wrong
let value = map.get("key").unwrap();

// Right
let value = map.get("key").ok_or_else(|| CollectionError::VariableNotFound {
    variable: "key".into(),
    available: map.keys().cloned().collect(),
})?;
```

Test code may use `unwrap()` freely.

## Module Organization

- One concept per module.
- Keep files under ~300 lines. If a file grows past that, it is a signal to split it.
- Group by responsibility, not by technical layer.

Expected layout for `strex-core`:

```
crates/strex-core/src/
  lib.rs              # public API surface only — re-exports, no logic
  error.rs            # CollectionError enum (thiserror)
  parser.rs           # YAML parsing and strict subset validation
  runner.rs           # collection execution orchestration (7-phase lifecycle)
  interpolation.rs    # {{variable}} template resolution
  http.rs             # reqwest HTTP client wrapper
  assertions.rs       # declarative assertion evaluation (status, jsonPath, headers)
```

## Naming Conventions

Follow standard Rust conventions:

| Kind | Convention | Example |
|------|-----------|---------|
| Functions / variables | `snake_case` | `parse_collection`, `base_url` |
| Types / structs / enums | `PascalCase` | `Collection`, `HttpResponse` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_TIMEOUT_MS` |
| Error types | `PascalCase` + `Error` suffix | `ScriptError`, `CollectionError` |
| Crates | `kebab-case` | `strex-core` |
| Modules | `snake_case` | `strex_core::parser` |

## Documentation

All `pub` items must have a `///` doc comment. Include what the function does, what it returns, and what errors it can produce:

```rust
/// Parses a Strex YAML collection file and returns a validated [`Collection`].
///
/// # Errors
///
/// Returns [`CollectionError::CollectionValidation`] if required fields are missing,
/// unknown fields are present, or forbidden YAML constructs (anchors, aliases) are used.
/// Returns [`CollectionError::DnsResolution`] if the environment file references a
/// hostname that cannot be resolved. See ADR-0002 for the complete error taxonomy.
pub fn parse_collection(path: &Path) -> Result<Collection, CollectionError> {
    // ...
}
```

Internal (`pub(crate)` or private) items do not require docs unless the logic is non-obvious.

## Dependencies

Every new dependency added to `Cargo.toml` must include a comment explaining why:

```toml
[dependencies]
# Async HTTP client — reqwest chosen over hyper for ergonomics; HTTP/1.1 only for MVP
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Typed error definitions for library crates (strex-core, strex-script)
thiserror = "1"

# YAML deserialization — serde_yaml chosen for serde compatibility
serde_yaml = "0.9"
serde = { version = "1", features = ["derive"] }
```

Do not add a new crate if an existing dependency already covers the need.

## Async Rules

- All async code uses the Tokio runtime.
- Scripts (rquickjs / QuickJS) are CPU-bound and **must not** run on Tokio's async executor threads. Doing so blocks all unrelated I/O.
- Use `tokio::task::spawn_blocking` for all script execution:

```rust
// crates/strex-script/src/runner.rs
use tokio::task;
use std::time::Duration;

pub async fn execute_script(
    script: String,
    context: ScriptContext,
) -> Result<ScriptResult, ScriptError> {
    let handle = task::spawn_blocking(move || {
        execute_script_blocking(script, context)
    });

    // Hard timeout enforced at Tokio level — 30s default
    // .and_then flattens the nested Result<Result<_, ScriptError>, JoinError>
    // before applying ?, so both error types are mapped to ScriptError first.
    tokio::time::timeout(Duration::from_secs(30), handle)
        .await
        .map_err(|_| ScriptError::Timeout { limit_seconds: 30 })
        .and_then(|r| r.map_err(|e| ScriptError::RuntimeError { message: e.to_string() }))?
}
```

See [ADR-0004](../adr/0004-script-safety-model.md) for the full worker thread architecture, memory limits, and sandboxed API design.

## Unsafe Code

Do not use `unsafe` without an explicit justification comment at the call site explaining why it is necessary and why it is sound:

```rust
// Safety: the buffer has been initialised by the OS and is guaranteed to be
// at least `len` bytes — see the contract documented in write(2).
let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
```

If you find yourself reaching for `unsafe`, stop and ask whether a safe abstraction already exists in the ecosystem. The bar for `unsafe` in this codebase is high.
