/// Errors that can occur during JavaScript script execution.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    /// JavaScript syntax error caught at compile time.
    #[error("script compilation error at {line}:{column}: {message}")]
    Compilation {
        line: u32,
        column: u32,
        message: String,
    },

    /// Unhandled JS exception during execution (not an assert failure).
    #[error("script runtime error: {message}")]
    Runtime {
        message: String,
        stack: Option<String>,
    },

    /// Script exceeded its CPU time limit.
    ///
    /// May be raised by the QuickJS interrupt handler (graceful) or by the Tokio
    /// timeout in `run_script()` (hard kill).
    #[error("script exceeded {limit_ms}ms time limit")]
    Timeout { limit_ms: u64 },

    /// QuickJS heap memory limit exceeded during script execution.
    #[error("script exceeded {limit_mb}MB memory limit")]
    MemoryLimit { limit_mb: u64 },

    /// `assert()` / `assertEqual()` / etc. called with a failing condition.
    ///
    /// Only raised in stop-on-assert mode (default). The runner catches this and
    /// decides whether to stop the request or collect it as an assertion failure.
    #[error("assertion failed: {message}")]
    AssertionFailed { message: String },

    /// QuickJS runtime or context could not be initialized (system OOM or internal error).
    #[error("script runtime initialization failed: {cause}")]
    RuntimeInit { cause: String },

    /// The `spawn_blocking` worker thread panicked (Rust panic, not a JS error).
    #[error("script worker thread panicked: {cause}")]
    ThreadPanic { cause: String },
}
