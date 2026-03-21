//! JSON formatter for performance test results.

use std::io::Write;

use strex_core::perf::PerfResult;

/// Write performance results as a JSON object to `writer`.
///
/// Schema:
/// ```json
/// {
///   "metrics": { ... },
///   "thresholds": [ { "metric": ..., "condition": ..., "value": ..., "observed": ..., "passed": ... } ],
///   "passed": true
/// }
/// ```
///
/// # Errors
///
/// Returns `Err` if JSON serialization or the write fails.
pub fn print(result: &PerfResult, writer: &mut impl Write) -> anyhow::Result<()> {
    let json = serde_json::json!({
        "metrics": result.metrics,
        "thresholds": result.threshold_results,
        "passed": result.passed(),
    });
    serde_json::to_writer_pretty(writer, &json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use strex_core::perf::{
        MetricKind, PerfMetrics, PerfResult, Threshold, ThresholdCondition, ThresholdResult,
    };

    fn minimal_result() -> PerfResult {
        PerfResult {
            metrics: PerfMetrics {
                total_iterations: 50,
                passed_iterations: 50,
                failed_iterations: 0,
                avg_response_ms: 100.0,
                min_response_ms: 80.0,
                max_response_ms: 150.0,
                p50_response_ms: 98.0,
                p95_response_ms: 140.0,
                p99_response_ms: 148.0,
                error_rate_pct: 0.0,
                throughput_rps: 25.0,
                elapsed_secs: 2.0,
            },
            threshold_results: vec![ThresholdResult {
                threshold: Threshold {
                    metric: MetricKind::ErrorRatePct,
                    condition: ThresholdCondition::Lt,
                    value: 1.0,
                },
                observed: 0.0,
                passed: true,
            }],
        }
    }

    #[test]
    fn json_output_is_valid_json() {
        let result = minimal_result();
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let content = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("should be valid JSON");
        assert!(parsed.is_object());
    }

    #[test]
    fn json_output_contains_metrics_thresholds_passed_keys() {
        let result = minimal_result();
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let content = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("metrics").is_some());
        assert!(parsed.get("thresholds").is_some());
        assert!(parsed.get("passed").is_some());
    }

    #[test]
    fn json_output_passed_field_is_true_when_all_pass() {
        let result = minimal_result();
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let content = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["passed"], serde_json::Value::Bool(true));
    }

    #[test]
    fn json_output_passed_field_is_false_when_threshold_fails() {
        let mut result = minimal_result();
        result.threshold_results[0].passed = false;
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let content = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["passed"], serde_json::Value::Bool(false));
    }

    #[test]
    fn json_metrics_contains_throughput_rps() {
        let result = minimal_result();
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let content = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["metrics"]["throughput_rps"].is_number());
    }
}
