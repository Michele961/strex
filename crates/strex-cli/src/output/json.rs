use crate::output::RunResult;
use std::io::Write;

/// Write a JSON report to `writer`.
#[allow(dead_code)]
pub fn print(_result: &RunResult, _writer: &mut impl Write) -> anyhow::Result<()> {
    todo!()
}
