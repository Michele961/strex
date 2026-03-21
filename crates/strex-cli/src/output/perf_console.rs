//! Console (human-readable) formatter for performance test results.

use std::io::Write;

use strex_core::perf::{MetricKind, PerfResult, ThresholdCondition};

/// Write a formatted performance metrics table to `writer`.
///
/// # Errors
///
/// Returns `Err` if any `writeln!` call fails.
pub fn print(result: &PerfResult, writer: &mut impl Write) -> anyhow::Result<()> {
    let m = &result.metrics;

    let separator = "═".repeat(54);

    writeln!(writer, "\nPerformance Test Results")?;
    writeln!(writer, "{separator}")?;

    // ── Run summary ──────────────────────────────────────────────────────────
    writeln!(
        writer,
        "Duration      {:.1} s      Iterations   {}",
        m.elapsed_secs, m.total_iterations
    )?;
    writeln!(writer, "Throughput    {:.1} req/s", m.throughput_rps)?;

    // ── Response time percentiles ────────────────────────────────────────────
    writeln!(writer)?;
    writeln!(writer, "Response Time (ms)")?;
    writeln!(
        writer,
        "  avg   {:>8.1}    p50   {:>8.1}",
        m.avg_response_ms, m.p50_response_ms
    )?;
    writeln!(
        writer,
        "  min   {:>8.1}    p95   {:>8.1}",
        m.min_response_ms, m.p95_response_ms
    )?;
    writeln!(
        writer,
        "  max   {:>8.1}    p99   {:>8.1}",
        m.max_response_ms, m.p99_response_ms
    )?;

    // ── Reliability ──────────────────────────────────────────────────────────
    writeln!(writer)?;
    writeln!(writer, "Reliability")?;
    let pass_pct = if m.total_iterations > 0 {
        m.passed_iterations as f64 / m.total_iterations as f64 * 100.0
    } else {
        0.0
    };
    let fail_pct = 100.0 - pass_pct;
    writeln!(
        writer,
        "  passed  {:>6}  ({:.1}%)",
        m.passed_iterations, pass_pct
    )?;
    writeln!(
        writer,
        "  failed  {:>6}  ({:.1}%)",
        m.failed_iterations, fail_pct
    )?;

    // ── Thresholds ───────────────────────────────────────────────────────────
    if !result.threshold_results.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "Thresholds")?;
        for tr in &result.threshold_results {
            let icon = if tr.passed { "✓" } else { "✗" };
            let metric_label = match tr.threshold.metric {
                MetricKind::AvgResponseMs => "avg_response_ms",
                MetricKind::P95ResponseMs => "p95_response_ms",
                MetricKind::P99ResponseMs => "p99_response_ms",
                MetricKind::ErrorRatePct => "error_rate_pct  ",
                MetricKind::ThroughputRps => "throughput_rps  ",
            };
            let cond_label = match tr.threshold.condition {
                ThresholdCondition::Lt => "<",
                ThresholdCondition::Lte => "<=",
                ThresholdCondition::Gt => ">",
                ThresholdCondition::Gte => ">=",
            };
            writeln!(
                writer,
                "  {icon}  {metric_label} {cond_label} {:.1}   observed: {:.1}",
                tr.threshold.value, tr.observed
            )?;
        }
    }

    // ── Final verdict ────────────────────────────────────────────────────────
    writeln!(writer, "{separator}")?;
    if result.passed() {
        if result.threshold_results.is_empty() {
            writeln!(writer, "Result: PASSED  (no thresholds defined)")?;
        } else {
            writeln!(writer, "Result: PASSED  (all thresholds met)")?;
        }
    } else {
        let failed_count = result
            .threshold_results
            .iter()
            .filter(|t| !t.passed)
            .count();
        writeln!(
            writer,
            "Result: FAILED  ({} threshold{} not met)",
            failed_count,
            if failed_count == 1 { "" } else { "s" }
        )?;
    }
    writeln!(writer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use strex_core::perf::{
        MetricKind, PerfMetrics, PerfResult, Threshold, ThresholdCondition, ThresholdResult,
    };

    fn sample_result(passed_threshold: bool) -> PerfResult {
        PerfResult {
            metrics: PerfMetrics {
                total_iterations: 100,
                passed_iterations: 99,
                failed_iterations: 1,
                avg_response_ms: 142.3,
                min_response_ms: 98.0,
                max_response_ms: 612.0,
                p50_response_ms: 135.0,
                p95_response_ms: 287.4,
                p99_response_ms: 451.2,
                error_rate_pct: 1.0,
                throughput_rps: 20.7,
                elapsed_secs: 60.1,
            },
            threshold_results: vec![ThresholdResult {
                threshold: Threshold {
                    metric: MetricKind::P95ResponseMs,
                    condition: ThresholdCondition::Lt,
                    value: 500.0,
                },
                observed: 287.4,
                passed: passed_threshold,
            }],
        }
    }

    #[test]
    fn console_output_contains_key_sections() {
        let result = sample_result(true);
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(out.contains("Performance Test Results"));
        assert!(out.contains("Response Time"));
        assert!(out.contains("Reliability"));
        assert!(out.contains("Thresholds"));
        assert!(out.contains("PASSED"));
    }

    #[test]
    fn console_output_shows_failed_when_threshold_fails() {
        let result = sample_result(false);
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(out.contains("FAILED"));
        assert!(out.contains("✗"));
    }

    #[test]
    fn console_output_no_thresholds_shows_no_thresholds_defined() {
        let result = PerfResult {
            metrics: PerfMetrics {
                total_iterations: 10,
                passed_iterations: 10,
                failed_iterations: 0,
                avg_response_ms: 50.0,
                min_response_ms: 40.0,
                max_response_ms: 80.0,
                p50_response_ms: 50.0,
                p95_response_ms: 75.0,
                p99_response_ms: 79.0,
                error_rate_pct: 0.0,
                throughput_rps: 5.0,
                elapsed_secs: 2.0,
            },
            threshold_results: vec![],
        };
        let mut buf = Vec::new();
        print(&result, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("no thresholds defined"));
    }
}
