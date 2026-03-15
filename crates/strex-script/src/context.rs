use std::collections::HashMap;
use strex_core::HttpResponse;

/// Input to a single script execution.
pub struct ScriptContext {
    /// HTTP response from Phase 4. `None` in Phase 2 (pre-request scripts).
    pub response: Option<HttpResponse>,
    /// Flat merged variable map (data > variables > environment).
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
#[derive(Debug)]
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
