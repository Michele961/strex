use std::io::Write;

use strex_core::{AssertionFailure, AssertionType, Collection, CollectionResult, DataRunResult};

use crate::cli::OutputFormat;

pub mod console;
pub mod json;
pub mod junit;

/// Aggregated result passed to all output formatters.
///
/// Carries both the parsed collection (for request metadata like HTTP method)
/// and the execution outcome.
pub struct RunResult {
    /// The parsed collection — used by formatters to access request methods.
    pub collection: Collection,
    /// The execution outcome: single-collection or data-driven.
    pub outcome: RunOutcome,
}

impl RunResult {
    /// Returns `true` if all requests/iterations passed.
    pub fn passed(&self) -> bool {
        match &self.outcome {
            RunOutcome::Single(r) => r.passed(),
            RunOutcome::DataDriven(r) => r.failed == 0,
        }
    }
}

/// The execution outcome variant.
pub enum RunOutcome {
    /// A single collection run (no data file).
    Single(CollectionResult),
    /// A data-driven run (one iteration per data row).
    DataDriven(DataRunResult),
}

/// Format an [`AssertionFailure`] as a human-readable string.
///
/// Used consistently by all output formatters (console, JSON, JUnit).
/// - `Status`   → `"status expected {expected}, got {actual}"`
/// - `JsonPath` → `"jsonPath {path} expected {expected}, got {actual}"`
/// - `Header`   → `"header expected {expected}, got {actual}"`
/// - `Script`   → `"{expected}"` (`actual` is always empty for script assertions)
pub fn format_failure(failure: &AssertionFailure) -> String {
    match failure.assertion_type {
        AssertionType::Status => {
            format!(
                "status expected {}, got {}",
                failure.expected, failure.actual
            )
        }
        AssertionType::JsonPath => {
            let path = failure.path.as_deref().unwrap_or("?");
            format!(
                "jsonPath {} expected {}, got {}",
                path, failure.expected, failure.actual
            )
        }
        AssertionType::Header => {
            format!(
                "header expected {}, got {}",
                failure.expected, failure.actual
            )
        }
        AssertionType::Script => failure.expected.clone(),
    }
}

/// Dispatch formatting to the selected output formatter, writing to `writer`.
pub fn format(
    result: &RunResult,
    fmt: &OutputFormat,
    writer: &mut impl Write,
) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Console => console::print(result, writer),
        OutputFormat::Json => json::print(result, writer),
        OutputFormat::Junit => junit::print(result, writer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use strex_core::{
        AssertionFailure, AssertionType, Collection, CollectionResult, Request, RequestOutcome,
        RequestResult,
    };

    fn one_request_collection() -> Collection {
        Collection {
            name: "Test".to_string(),
            version: "1.0".to_string(),
            environment: HashMap::new(),
            variables: HashMap::new(),
            requests: vec![Request {
                name: "Get".to_string(),
                method: "GET".to_string(),
                url: "https://example.com".to_string(),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: None,
                assertions: vec![],
                timeout: None,
                on_failure: None,
            }],
        }
    }

    #[test]
    fn run_result_passed_when_single_passed() {
        let col_result = CollectionResult {
            request_results: vec![RequestResult {
                name: "Get".to_string(),
                outcome: RequestOutcome::Passed,
                duration_ms: 5,
                response: None,
                logs: vec![],
                passed_assertions: vec![],
            }],
        };
        let result = RunResult {
            collection: one_request_collection(),
            outcome: RunOutcome::Single(col_result),
        };
        assert!(result.passed());
    }

    #[test]
    fn run_result_failed_when_assertion_fails() {
        let col_result = CollectionResult {
            request_results: vec![RequestResult {
                name: "Get".to_string(),
                outcome: RequestOutcome::AssertionsFailed(vec![AssertionFailure {
                    assertion_type: AssertionType::Status,
                    expected: "200".to_string(),
                    actual: "404".to_string(),
                    path: None,
                }]),
                duration_ms: 5,
                response: None,
                logs: vec![],
                passed_assertions: vec![],
            }],
        };
        let result = RunResult {
            collection: one_request_collection(),
            outcome: RunOutcome::Single(col_result),
        };
        assert!(!result.passed());
    }

    #[test]
    fn format_failure_status_shows_expected_and_actual() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Status,
            expected: "200".to_string(),
            actual: "404".to_string(),
            path: None,
        };
        assert_eq!(format_failure(&f), "status expected 200, got 404");
    }

    #[test]
    fn format_failure_jsonpath_shows_expected_and_actual() {
        let f = AssertionFailure {
            assertion_type: AssertionType::JsonPath,
            expected: "$.name exists".to_string(),
            actual: "null".to_string(),
            path: Some("$.name".to_string()),
        };
        assert_eq!(
            format_failure(&f),
            "jsonPath $.name expected $.name exists, got null"
        );
    }

    #[test]
    fn format_failure_header_shows_expected_and_actual() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Header,
            expected: "application/json".to_string(),
            actual: "text/plain".to_string(),
            path: None,
        };
        assert_eq!(
            format_failure(&f),
            "header expected application/json, got text/plain"
        );
    }

    #[test]
    fn format_failure_script_shows_message_only() {
        let f = AssertionFailure {
            assertion_type: AssertionType::Script,
            expected: "must have items".to_string(),
            actual: String::new(),
            path: None,
        };
        assert_eq!(format_failure(&f), "must have items");
    }

    #[test]
    fn run_result_passed_when_data_driven_no_failures() {
        use strex_core::DataRunResult;
        let result = RunResult {
            collection: one_request_collection(),
            outcome: RunOutcome::DataDriven(DataRunResult {
                iterations: vec![],
                passed: 3,
                failed: 0,
            }),
        };
        assert!(result.passed());
    }

    #[test]
    fn run_result_failed_when_data_driven_has_failures() {
        use strex_core::DataRunResult;
        let result = RunResult {
            collection: one_request_collection(),
            outcome: RunOutcome::DataDriven(DataRunResult {
                iterations: vec![],
                passed: 2,
                failed: 1,
            }),
        };
        assert!(!result.passed());
    }
}
