use std::collections::HashMap;

/// Per-request timing breakdown exposed to JavaScript scripts.
///
/// Rust fields use `_ms` suffix for clarity; JavaScript exposes them without the
/// suffix (e.g., `response.timing.dns`, `response.timing.total`).
#[derive(Debug, Clone, Default)]
pub struct ScriptTiming {
    /// DNS resolution time in ms.
    pub dns_ms: u64,
    /// TCP connect time in ms.
    pub connect_ms: u64,
    /// TLS handshake time in ms.
    pub tls_ms: u64,
    /// Request body write time in ms.
    pub send_ms: u64,
    /// Time from request send to first response byte in ms.
    pub wait_ms: u64,
    /// Response body read time in ms.
    pub receive_ms: u64,
    /// Total lifecycle duration in ms.
    pub total_ms: u64,
}

/// HTTP response data exposed to JavaScript scripts.
///
/// This is a self-contained type that mirrors the fields of `strex_core::HttpResponse`
/// without importing it — avoiding a circular crate dependency.
#[derive(Debug, Clone)]
pub struct ScriptResponse {
    /// HTTP status code (e.g., 200, 404).
    pub status: u16,
    /// Response headers, lowercased.
    pub headers: HashMap<String, String>,
    /// Response body as a UTF-8 string.
    pub body: String,
    /// Per-phase timing breakdown.
    pub timing: ScriptTiming,
}

/// Input to a single script execution.
#[derive(Debug)]
pub struct ScriptContext {
    /// HTTP response from Phase 4. `None` in Phase 2 (pre-request scripts).
    pub response: Option<ScriptResponse>,
    /// Mutable collection-layer variables, exposed to JS as the writable `variables` global.
    pub variables: HashMap<String, String>,
    /// Environment layer — read-only in JS (`env` global).
    pub environment: HashMap<String, String>,
    /// Data layer from CSV/JSON — read-only in JS (`data` global). Empty until SP4.
    pub data: HashMap<String, String>,
}

/// Output from a single script execution.
#[derive(Debug, Default)]
pub struct ScriptResult {
    /// Keys written via `variables.set(key, value)`.
    pub variable_mutations: HashMap<String, String>,
    /// Keys removed via `variables.delete(key)`.
    pub variable_deletions: Vec<String>,
    /// `true` if `variables.clear()` was called.
    ///
    /// When `true`, `apply_mutations` clears all of `ExecutionContext.variables` before
    /// applying deletions and mutations — so post-clear `set()` calls survive.
    pub variables_cleared: bool,
    /// Assertion failure messages collected when the runner is in continue mode.
    ///
    /// NOTE: `execute_script()` never populates this field. The runner populates it
    /// when it catches `ScriptError::AssertionFailed` in continue mode.
    pub assertion_failures: Vec<String>,
    /// Output from `console.log/warn/error()`, in emission order.
    pub console_logs: Vec<ConsoleEntry>,
}

/// A single console output entry with log level preserved.
#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    /// The log level/severity.
    pub level: LogLevel,
    /// The logged message.
    pub message: String,
}

/// Console output severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// `console.log()`
    Log,
    /// `console.warn()`
    Warn,
    /// `console.error()`
    Error,
}

/// Options controlling the QuickJS runtime limits.
#[derive(Debug, Clone)]
pub struct ScriptOptions {
    /// QuickJS heap memory limit in bytes. Default: 64 * 1024 * 1024 (64 MB).
    pub memory_limit_bytes: usize,
    /// Script CPU time budget in milliseconds.
    ///
    /// Used by both the QuickJS interrupt handler (graceful stop) and the Tokio
    /// timeout in `run_script()` (hard kill backstop).
    pub timeout_ms: u64,
}

impl Default for ScriptOptions {
    fn default() -> Self {
        Self {
            memory_limit_bytes: 64 * 1024 * 1024,
            timeout_ms: 30_000,
        }
    }
}
