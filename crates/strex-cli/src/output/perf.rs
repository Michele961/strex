//! Dispatcher for performance output formatters.

use std::io::Write;

use strex_core::perf::PerfResult;

use crate::cli::PerfOutputFormat;
use crate::output::{perf_console, perf_json};

/// Format a [`PerfResult`] according to `fmt` and write to `writer`.
///
/// # Errors
///
/// Returns `Err` if the underlying formatter fails to write.
pub fn format(
    result: &PerfResult,
    fmt: &PerfOutputFormat,
    writer: &mut impl Write,
) -> anyhow::Result<()> {
    match fmt {
        PerfOutputFormat::Console => perf_console::print(result, writer),
        PerfOutputFormat::Json => perf_json::print(result, writer),
    }
}
