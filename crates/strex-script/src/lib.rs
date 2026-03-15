#![deny(clippy::all)]

mod api;
mod context;
mod error;
mod executor;

pub use context::{ConsoleEntry, LogLevel, ScriptContext, ScriptOptions, ScriptResult};
pub use error::ScriptError;
pub use executor::execute_script;
