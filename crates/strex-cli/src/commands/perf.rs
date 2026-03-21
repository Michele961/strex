//! Handler for the `strex perf` subcommand.

use std::sync::Arc;
use std::time::Duration;

use strex_core::perf::{LoadProfile, MetricKind, PerfOpts, Threshold, ThresholdCondition};
use strex_core::{parse_collection, parse_csv, parse_json};

use crate::cli::{LoadProfileArg, PerfArgs};
use crate::output::perf as perf_output;

/// Parse a threshold string of the form `METRIC:CONDITION:VALUE`.
///
/// Accepted metrics: `avg_response_ms`, `p95_response_ms`, `p99_response_ms`,
///                   `error_rate_pct`, `throughput_rps`
///
/// Accepted conditions: `lt`, `lte`, `gt`, `gte`
fn parse_threshold(s: &str) -> anyhow::Result<Threshold> {
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    anyhow::ensure!(
        parts.len() == 3,
        "threshold must be METRIC:CONDITION:VALUE, got: {s}"
    );

    let metric = match parts[0] {
        "avg_response_ms" => MetricKind::AvgResponseMs,
        "p95_response_ms" => MetricKind::P95ResponseMs,
        "p99_response_ms" => MetricKind::P99ResponseMs,
        "error_rate_pct" => MetricKind::ErrorRatePct,
        "throughput_rps" => MetricKind::ThroughputRps,
        other => anyhow::bail!(
            "unknown metric '{}'; valid metrics: avg_response_ms, p95_response_ms, \
             p99_response_ms, error_rate_pct, throughput_rps",
            other
        ),
    };

    let condition = match parts[1] {
        "lt" => ThresholdCondition::Lt,
        "lte" => ThresholdCondition::Lte,
        "gt" => ThresholdCondition::Gt,
        "gte" => ThresholdCondition::Gte,
        other => anyhow::bail!(
            "unknown condition '{}'; valid conditions: lt, lte, gt, gte",
            other
        ),
    };

    let value: f64 = parts[2]
        .parse()
        .map_err(|_| anyhow::anyhow!("threshold value must be a number, got: {}", parts[2]))?;

    Ok(Threshold {
        metric,
        condition,
        value,
    })
}

/// Execute the `perf` subcommand.
///
/// Returns `Ok(0)` when all thresholds pass (or no thresholds defined),
/// `Ok(1)` when one or more thresholds fail, and `Err(...)` (exit code 2 in
/// `main`) for infrastructure errors such as missing files or invalid options.
pub async fn execute(args: PerfArgs) -> anyhow::Result<i32> {
    // ── Step 1: Parse collection ──────────────────────────────────────────────
    let collection = parse_collection(&args.collection)?;

    // ── Step 2: Resolve PerfOpts (CLI args > YAML defaults) ──────────────────
    let perf_cfg = collection.performance.clone().unwrap_or_default();

    let vus = args.vus.unwrap_or(perf_cfg.vus).max(1);
    let duration_secs = args.duration.unwrap_or(perf_cfg.duration_secs).max(1);

    let load_profile = args
        .load_profile
        .map(|a| match a {
            LoadProfileArg::Fixed => LoadProfile::Fixed,
            LoadProfileArg::RampUp => LoadProfile::RampUp,
        })
        .unwrap_or(perf_cfg.load_profile);

    let initial_vus = args.initial_vus.unwrap_or(perf_cfg.initial_vus);
    anyhow::ensure!(
        initial_vus <= vus,
        "--initial-vus ({initial_vus}) must be <= --vus ({vus})"
    );

    // Merge CLI --threshold flags on top of collection YAML thresholds.
    let mut thresholds = perf_cfg.thresholds.clone();
    for s in &args.thresholds {
        thresholds.push(parse_threshold(s)?);
    }

    // ── Step 3: Load optional data file ──────────────────────────────────────
    let data_rows = if let Some(ref data_path) = args.data {
        let content = std::fs::read_to_string(data_path)?;
        let ext = data_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "csv" => parse_csv(&content)?,
            "json" => parse_json(&content)?,
            other => anyhow::bail!("unsupported data file extension: {}", other),
        }
    } else {
        vec![]
    };

    // ── Step 4: Build PerfOpts ────────────────────────────────────────────────
    let http_client = reqwest::Client::builder()
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))?;

    let opts = PerfOpts {
        vus,
        duration: Duration::from_secs(duration_secs),
        load_profile,
        initial_vus,
        thresholds,
        data_rows,
        http_client: Arc::new(http_client),
        script_timeout_ms: 30_000,
        progress_tx: None,
    };

    // ── Step 5: Run the performance test ──────────────────────────────────────
    let perf_result = strex_core::run_perf(collection, opts)
        .await
        .map_err(|e| anyhow::anyhow!("performance run failed: {e}"))?;

    // ── Step 6: Write formatted output ────────────────────────────────────────
    let mut writer: Box<dyn std::io::Write> = match &args.output {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };
    perf_output::format(&perf_result, &args.format, &mut writer)?;

    // ── Step 7: Return exit code ──────────────────────────────────────────────
    if perf_result.passed() {
        Ok(0)
    } else {
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::PerfOutputFormat;

    // ── parse_threshold unit tests ────────────────────────────────────────────

    #[test]
    fn parse_threshold_p95_lt_500() {
        let t = parse_threshold("p95_response_ms:lt:500").unwrap();
        assert!(matches!(t.metric, MetricKind::P95ResponseMs));
        assert!(matches!(t.condition, ThresholdCondition::Lt));
        assert!((t.value - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_threshold_error_rate_lte() {
        let t = parse_threshold("error_rate_pct:lte:1.5").unwrap();
        assert!(matches!(t.metric, MetricKind::ErrorRatePct));
        assert!(matches!(t.condition, ThresholdCondition::Lte));
        assert!((t.value - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_threshold_throughput_gte() {
        let t = parse_threshold("throughput_rps:gte:10").unwrap();
        assert!(matches!(t.metric, MetricKind::ThroughputRps));
        assert!(matches!(t.condition, ThresholdCondition::Gte));
        assert!((t.value - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_threshold_missing_parts_returns_err() {
        assert!(parse_threshold("p95_response_ms:lt").is_err());
        assert!(parse_threshold("p95_response_ms").is_err());
        assert!(parse_threshold("").is_err());
    }

    #[test]
    fn parse_threshold_unknown_metric_returns_err() {
        assert!(parse_threshold("unknown_metric:lt:100").is_err());
    }

    #[test]
    fn parse_threshold_unknown_condition_returns_err() {
        assert!(parse_threshold("p95_response_ms:equal:100").is_err());
    }

    #[test]
    fn parse_threshold_non_numeric_value_returns_err() {
        assert!(parse_threshold("p95_response_ms:lt:fast").is_err());
    }

    // ── execute integration tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn perf_run_exits_zero_when_all_thresholds_pass() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let mut col_file = NamedTempFile::new().unwrap();
        write!(
            col_file,
            r#"name: Perf Test
version: "1.0"
requests:
  - name: Get
    method: GET
    url: "{}/ok"
"#,
            server.uri()
        )
        .unwrap();

        let args = PerfArgs {
            collection: col_file.path().to_path_buf(),
            vus: Some(1),
            duration: Some(2),
            load_profile: None,
            initial_vus: None,
            thresholds: vec!["p95_response_ms:lt:60000".to_string()],
            data: None,
            format: PerfOutputFormat::Console,
            output: None,
        };

        let code = execute(args).await.unwrap();
        assert_eq!(code, 0, "all thresholds pass → exit 0");
    }

    #[tokio::test]
    async fn perf_run_exits_one_when_threshold_fails() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        // Assert status 200 so every iteration fails.
        let mut col_file = NamedTempFile::new().unwrap();
        write!(
            col_file,
            r#"name: Perf Test
version: "1.0"
requests:
  - name: Get
    method: GET
    url: "{}/ok"
    assertions:
      - status: 200
"#,
            server.uri()
        )
        .unwrap();

        let args = PerfArgs {
            collection: col_file.path().to_path_buf(),
            vus: Some(1),
            duration: Some(2),
            load_profile: None,
            initial_vus: None,
            thresholds: vec!["error_rate_pct:lt:1".to_string()], // will be violated
            data: None,
            format: PerfOutputFormat::Console,
            output: None,
        };

        let code = execute(args).await.unwrap();
        assert_eq!(code, 1, "threshold fails → exit 1");
    }

    #[tokio::test]
    async fn perf_run_json_output_is_valid_json() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let mut col_file = NamedTempFile::new().unwrap();
        write!(
            col_file,
            "name: T\nversion: \"1.0\"\nrequests:\n  - name: G\n    method: GET\n    url: \"{}/ok\"\n",
            server.uri()
        )
        .unwrap();

        let out_file = NamedTempFile::new().unwrap();
        let out_path = out_file.path().to_path_buf();

        let args = PerfArgs {
            collection: col_file.path().to_path_buf(),
            vus: Some(1),
            duration: Some(2),
            load_profile: None,
            initial_vus: None,
            thresholds: vec![],
            data: None,
            format: PerfOutputFormat::Json,
            output: Some(out_path.clone()),
        };
        execute(args).await.unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("perf JSON output should be valid JSON");
        assert!(parsed.get("metrics").is_some());
        assert!(parsed.get("thresholds").is_some());
        assert!(parsed.get("passed").is_some());
    }

    #[tokio::test]
    async fn perf_run_initial_vus_exceeds_vus_returns_err() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut col_file = NamedTempFile::new().unwrap();
        write!(
            col_file,
            "name: T\nversion: \"1.0\"\nrequests:\n  - name: G\n    method: GET\n    url: \"http://localhost/ok\"\n"
        )
        .unwrap();

        let args = PerfArgs {
            collection: col_file.path().to_path_buf(),
            vus: Some(2),
            duration: Some(1),
            load_profile: Some(LoadProfileArg::RampUp),
            initial_vus: Some(5), // > vus → error
            thresholds: vec![],
            data: None,
            format: PerfOutputFormat::Console,
            output: None,
        };
        let result = execute(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("initial-vus"));
    }

    #[tokio::test]
    async fn perf_run_unsupported_data_extension_returns_err() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut col_file = NamedTempFile::new().unwrap();
        write!(
            col_file,
            "name: T\nversion: \"1.0\"\nrequests:\n  - name: G\n    method: GET\n    url: \"http://localhost/ok\"\n"
        )
        .unwrap();

        let mut data_file = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
        write!(data_file, "id\n1\n").unwrap();

        let args = PerfArgs {
            collection: col_file.path().to_path_buf(),
            vus: Some(1),
            duration: Some(1),
            load_profile: None,
            initial_vus: None,
            thresholds: vec![],
            data: Some(data_file.path().to_path_buf()),
            format: PerfOutputFormat::Console,
            output: None,
        };
        let result = execute(args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unsupported data file extension"));
    }
}
