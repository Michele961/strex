use crate::context::{ScriptContext, ScriptOptions, ScriptResult};
use crate::error::ScriptError;

/// Execute a JavaScript script synchronously.
///
/// Must be called inside `tokio::task::spawn_blocking` — never on an async thread.
pub fn execute_script(
    _script: &str,
    _context: ScriptContext,
    _opts: &ScriptOptions,
) -> Result<ScriptResult, ScriptError> {
    todo!("Implemented in Task 4")
}
