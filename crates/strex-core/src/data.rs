#![allow(dead_code)] // Items here will be used in Task 8 for data-driven collection execution

use std::collections::HashMap;

use crate::collection::Collection;
use crate::runner::{CollectionResult, RunnerOpts};

/// A single row of data: column name → string value.
pub type DataRow = HashMap<String, String>;

/// Options controlling data-driven execution.
///
/// Separate from [`RunnerOpts`] to keep per-request concerns out of the data layer.
#[derive(Debug, Clone)]
pub struct DataRunOpts {
    /// Number of concurrent iteration tasks. Must be >= 1.
    pub concurrency: usize,
    /// When `true`, no new iterations are launched after any iteration fails.
    /// In-flight iterations always run to completion.
    pub fail_fast: bool,
    /// Options forwarded to each `execute_collection_with_opts` call.
    pub runner_opts: RunnerOpts,
}

impl Default for DataRunOpts {
    fn default() -> Self {
        Self {
            concurrency: 1,
            fail_fast: false,
            runner_opts: RunnerOpts::default(),
        }
    }
}

/// Result of a single iteration (one data row through the collection).
#[derive(Debug)]
pub struct IterationResult {
    /// Zero-based index of this row in the original input.
    pub row_index: usize,
    /// The data row used for this iteration.
    pub row: DataRow,
    /// Collection execution result for this iteration.
    pub collection_result: CollectionResult,
}

/// Aggregated results from all iterations of a data-driven run.
#[derive(Debug)]
pub struct DataRunResult {
    /// All iteration results, sorted by `row_index` (input order preserved).
    pub iterations: Vec<IterationResult>,
    /// Number of iterations where `collection_result.passed()` is `true`.
    pub passed: usize,
    /// Number of iterations where `collection_result.passed()` is `false`.
    pub failed: usize,
}

/// Errors produced by data parsing or the data-driven orchestrator.
#[derive(thiserror::Error, Debug)]
pub enum DataError {
    /// `DataRunOpts::concurrency` was 0.
    #[error("concurrency must be at least 1")]
    InvalidConcurrency,
    /// CSV content could not be parsed.
    #[error("CSV parse error: {0}")]
    CsvParse(#[from] csv::Error),
    /// JSON content could not be parsed.
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
    /// JSON top-level value is not an array.
    #[error("JSON data file must be a top-level array")]
    JsonNotArray,
    /// A JSON array element is not an object.
    #[error("JSON row {index} is not an object")]
    JsonRowNotObject { index: usize },
    /// A spawned iteration task panicked.
    #[error("iteration task panicked: {0}")]
    TaskPanic(String),
    /// An internal error that should never occur in correct usage.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Parse CSV content into rows.
///
/// The first row is treated as the header (column names). All values are strings.
/// Returns `Ok(vec![])` for content with no data rows (only a header or empty input).
pub fn parse_csv(_content: &str) -> Result<Vec<DataRow>, DataError> {
    todo!()
}

/// Parse JSON content into rows.
///
/// The top-level value must be an array of objects. Each object's values are
/// coerced to strings: `String` values are used directly; all other types use
/// `.to_string()`. Returns `Ok(vec![])` for an empty array.
pub fn parse_json(_content: &str) -> Result<Vec<DataRow>, DataError> {
    todo!()
}

/// Run `collection` once per row, with bounded concurrency and optional fail-fast.
///
/// Returns `Ok(DataRunResult)` with all iteration results (including failures).
/// Returns `Err(DataError)` only for invalid options or infrastructure failures.
pub async fn run_collection_with_data(
    _collection: Collection,
    _rows: Vec<DataRow>,
    _opts: DataRunOpts,
) -> Result<DataRunResult, DataError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_error_invalid_concurrency_display() {
        assert_eq!(
            DataError::InvalidConcurrency.to_string(),
            "concurrency must be at least 1"
        );
    }

    #[test]
    fn data_error_json_not_array_display() {
        assert_eq!(
            DataError::JsonNotArray.to_string(),
            "JSON data file must be a top-level array"
        );
    }

    #[test]
    fn data_error_json_row_not_object_display() {
        let err = DataError::JsonRowNotObject { index: 2 };
        assert_eq!(err.to_string(), "JSON row 2 is not an object");
    }

    #[test]
    fn data_error_task_panic_display() {
        let err = DataError::TaskPanic("oh no".to_string());
        assert_eq!(err.to_string(), "iteration task panicked: oh no");
    }

    #[test]
    fn data_error_internal_display() {
        let err = DataError::Internal("semaphore gone".to_string());
        assert_eq!(err.to_string(), "internal error: semaphore gone");
    }

    #[test]
    fn data_run_opts_default_concurrency_is_1() {
        let opts = DataRunOpts::default();
        assert_eq!(opts.concurrency, 1);
        assert!(!opts.fail_fast);
    }
}
