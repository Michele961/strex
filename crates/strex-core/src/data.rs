use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::collection::Collection;
use crate::context::ExecutionContext;
use crate::runner::{execute_collection_with_opts, CollectionResult, RunnerOpts};

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
    /// Milliseconds to sleep before each iteration after the first one.
    /// Default: `0` (no delay).
    pub delay_between_iterations_ms: u64,
    /// Options forwarded to each `execute_collection_with_opts` call.
    pub runner_opts: RunnerOpts,
}

impl Default for DataRunOpts {
    fn default() -> Self {
        Self {
            concurrency: 1,
            fail_fast: false,
            delay_between_iterations_ms: 0,
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
pub fn parse_csv(content: &str) -> Result<Vec<DataRow>, DataError> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(content.as_bytes());

    let headers: Vec<String> = reader.headers()?.iter().map(|h| h.to_string()).collect();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let row: DataRow = headers
            .iter()
            .zip(record.iter())
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
        rows.push(row);
    }
    Ok(rows)
}

/// Parse JSON content into rows.
///
/// The top-level value must be an array of objects. Each object's values are
/// coerced to strings: `String` values are used directly; all other types use
/// `.to_string()`. Returns `Ok(vec![])` for an empty array.
pub fn parse_json(content: &str) -> Result<Vec<DataRow>, DataError> {
    let value: serde_json::Value = serde_json::from_str(content)?;

    let array = value.as_array().ok_or(DataError::JsonNotArray)?;

    let mut rows = Vec::with_capacity(array.len());
    for (index, element) in array.iter().enumerate() {
        let obj = element
            .as_object()
            .ok_or(DataError::JsonRowNotObject { index })?;

        let row: DataRow = obj
            .iter()
            .map(|(k, v)| {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (k.clone(), s)
            })
            .collect();
        rows.push(row);
    }
    Ok(rows)
}

/// Run `collection` once per row, with bounded concurrency and optional fail-fast.
///
/// Returns `Ok(DataRunResult)` with all iteration results (including failures).
/// Returns `Err(DataError)` only for invalid options or infrastructure failures.
pub async fn run_collection_with_data(
    collection: Collection,
    rows: Vec<DataRow>,
    opts: DataRunOpts,
) -> Result<DataRunResult, DataError> {
    if opts.concurrency == 0 {
        return Err(DataError::InvalidConcurrency);
    }
    if rows.is_empty() {
        return Ok(DataRunResult {
            iterations: vec![],
            passed: 0,
            failed: 0,
        });
    }

    let arc_col = Arc::new(collection);
    let semaphore = Arc::new(Semaphore::new(opts.concurrency));
    let fail_flag = Arc::new(AtomicBool::new(false));
    let fail_fast = opts.fail_fast;

    let mut join_set: JoinSet<IterationResult> = JoinSet::new();

    for (idx, row) in rows.into_iter().enumerate() {
        // Check before blocking on semaphore acquisition.
        if fail_fast && fail_flag.load(Ordering::Acquire) {
            break;
        }

        if idx > 0 && opts.delay_between_iterations_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(
                opts.delay_between_iterations_ms,
            ))
            .await;
        }

        let col = Arc::clone(&arc_col);
        let sem = Arc::clone(&semaphore);
        let flag = Arc::clone(&fail_flag);
        let runner_opts = opts.runner_opts.clone();

        // Block until a slot opens. AcquireError is structurally unreachable because
        // we never explicitly close the semaphore, but we propagate via ? to comply
        // with the no-expect-in-non-test-code rule.
        let permit = sem
            .acquire_owned()
            .await
            .map_err(|e| DataError::Internal(e.to_string()))?;

        // Re-check after potentially blocking on acquire: a task that ran during the
        // wait may have set the flag (critical for concurrency=1 correctness).
        if fail_fast && fail_flag.load(Ordering::Acquire) {
            break;
        }

        join_set.spawn(async move {
            let _permit = permit; // auto-released on drop, freeing a semaphore slot
            let ctx = ExecutionContext::new_with_data(&col, &row);
            let collection_result = execute_collection_with_opts(&col, ctx, runner_opts).await;
            if fail_fast && !collection_result.passed() {
                flag.store(true, Ordering::Release);
            }
            IterationResult {
                row_index: idx,
                row,
                collection_result,
            }
        });
    }

    let mut iteration_results: Vec<IterationResult> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(iter_result) => iteration_results.push(iter_result),
            Err(join_err) => return Err(DataError::TaskPanic(join_err.to_string())),
        }
    }

    iteration_results.sort_by_key(|r| r.row_index);

    let passed = iteration_results
        .iter()
        .filter(|r| r.collection_result.passed())
        .count();
    let failed = iteration_results.len() - passed;

    Ok(DataRunResult {
        iterations: iteration_results,
        passed,
        failed,
    })
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
