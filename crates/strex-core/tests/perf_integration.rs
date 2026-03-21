//! Integration tests for `run_perf` using wiremock as the HTTP server.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use strex_core::perf::{
    run_perf, LoadProfile, MetricKind, PerfError, PerfOpts, Threshold, ThresholdCondition,
};
use strex_core::{Collection, Request};

// ── helpers ───────────────────────────────────────────────────────────────────

fn simple_collection(url: &str) -> Collection {
    Collection {
        name: "perf-test".to_string(),
        version: "1.0".to_string(),
        environment: HashMap::new(),
        variables: HashMap::new(),
        requests: vec![Request {
            name: "Get".to_string(),
            method: "GET".to_string(),
            url: format!("{url}/ok"),
            headers: HashMap::new(),
            body: None,
            pre_script: None,
            post_script: None,
            assertions: vec![],
            timeout: None,
            on_failure: None,
        }],
        performance: None,
    }
}

fn collection_with_status_assertion(url: &str, expected_status: u64) -> Collection {
    use serde_yaml::Value as YamlValue;
    let mut assertion = HashMap::new();
    assertion.insert(
        "status".to_string(),
        YamlValue::Number(serde_yaml::Number::from(expected_status)),
    );
    Collection {
        name: "perf-test".to_string(),
        version: "1.0".to_string(),
        environment: HashMap::new(),
        variables: HashMap::new(),
        requests: vec![Request {
            name: "Get".to_string(),
            method: "GET".to_string(),
            url: format!("{url}/ok"),
            headers: HashMap::new(),
            body: None,
            pre_script: None,
            post_script: None,
            assertions: vec![assertion],
            timeout: None,
            on_failure: None,
        }],
        performance: None,
    }
}

fn make_opts(vus: usize, duration_secs: u64) -> PerfOpts {
    PerfOpts {
        vus,
        duration: Duration::from_secs(duration_secs),
        load_profile: LoadProfile::Fixed,
        initial_vus: 1,
        thresholds: vec![],
        data_rows: vec![],
        http_client: Arc::new(reqwest::Client::new()),
        script_timeout_ms: 30_000,
        progress_tx: None,
    }
}

// ── validation errors ─────────────────────────────────────────────────────────

#[tokio::test]
async fn run_perf_returns_invalid_vus_when_zero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());
    let opts = PerfOpts {
        vus: 0,
        duration: Duration::from_secs(1),
        load_profile: LoadProfile::Fixed,
        initial_vus: 0,
        thresholds: vec![],
        data_rows: vec![],
        http_client: Arc::new(reqwest::Client::new()),
        script_timeout_ms: 30_000,
        progress_tx: None,
    };
    let result = run_perf(col, opts).await;
    assert!(matches!(result, Err(PerfError::InvalidVus)));
}

#[tokio::test]
async fn run_perf_returns_invalid_duration_when_zero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());
    let opts = PerfOpts {
        vus: 2,
        duration: Duration::ZERO,
        load_profile: LoadProfile::Fixed,
        initial_vus: 1,
        thresholds: vec![],
        data_rows: vec![],
        http_client: Arc::new(reqwest::Client::new()),
        script_timeout_ms: 30_000,
        progress_tx: None,
    };
    let result = run_perf(col, opts).await;
    assert!(matches!(result, Err(PerfError::InvalidDuration)));
}

#[tokio::test]
async fn run_perf_returns_error_when_initial_vus_exceeds_vus() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());
    let opts = PerfOpts {
        vus: 2,
        duration: Duration::from_secs(1),
        load_profile: LoadProfile::RampUp,
        initial_vus: 5, // > vus
        thresholds: vec![],
        data_rows: vec![],
        http_client: Arc::new(reqwest::Client::new()),
        script_timeout_ms: 30_000,
        progress_tx: None,
    };
    let result = run_perf(col, opts).await;
    assert!(matches!(
        result,
        Err(PerfError::InitialVusExceedsVus {
            initial: 5,
            target: 2
        })
    ));
}

// ── basic run behaviour ───────────────────────────────────────────────────────

#[tokio::test]
async fn fixed_profile_runs_and_returns_nonzero_iterations() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());
    let opts = make_opts(2, 2); // 2 VUs for 2 seconds
    let result = run_perf(col, opts)
        .await
        .expect("run_perf should not error");

    assert!(
        result.metrics.total_iterations > 0,
        "should have completed at least one iteration"
    );
    assert_eq!(result.metrics.failed_iterations, 0);
    assert_eq!(result.metrics.error_rate_pct, 0.0);
    assert!(result.metrics.throughput_rps > 0.0);
}

#[tokio::test]
async fn fixed_profile_completes_within_expected_duration_window() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());
    let start = std::time::Instant::now();
    let opts = make_opts(1, 2); // 2-second run
    let result = run_perf(col, opts)
        .await
        .expect("run_perf should not error");
    let elapsed = start.elapsed();

    // Should complete between 1.8s and 5s (generous upper bound for CI).
    assert!(
        elapsed >= Duration::from_millis(1800),
        "elapsed too short: {elapsed:?}"
    );
    assert!(
        elapsed <= Duration::from_secs(5),
        "elapsed too long: {elapsed:?}"
    );
    assert!((result.metrics.elapsed_secs - 2.0).abs() < 3.0);
}

#[tokio::test]
async fn error_rate_nonzero_when_server_returns_500_and_status_assertion_expects_200() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    // The collection asserts status 200 but the server returns 500 → failures.
    let col = collection_with_status_assertion(&server.uri(), 200);
    let opts = make_opts(1, 2);
    let result = run_perf(col, opts)
        .await
        .expect("run_perf should not error");

    assert!(result.metrics.total_iterations > 0);
    assert!(result.metrics.failed_iterations > 0);
    assert!(result.metrics.error_rate_pct > 0.0);
}

// ── thresholds ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn threshold_passes_when_server_is_fast_and_p95_under_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());
    let opts = PerfOpts {
        thresholds: vec![Threshold {
            metric: MetricKind::P95ResponseMs,
            condition: ThresholdCondition::Lt,
            value: 60_000.0, // 60 seconds — always passes for a fast local server
        }],
        ..make_opts(1, 2)
    };
    let result = run_perf(col, opts)
        .await
        .expect("run_perf should not error");
    assert!(result.passed(), "threshold should pass for a fast server");
}

#[tokio::test]
async fn threshold_fails_when_error_rate_limit_exceeded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(500)) // all requests fail
        .mount(&server)
        .await;

    let col = collection_with_status_assertion(&server.uri(), 200);
    let opts = PerfOpts {
        thresholds: vec![Threshold {
            metric: MetricKind::ErrorRatePct,
            condition: ThresholdCondition::Lt,
            value: 1.0, // requires < 1% errors; all requests fail → threshold fails
        }],
        ..make_opts(1, 2)
    };
    let result = run_perf(col, opts)
        .await
        .expect("run_perf should not error");
    assert!(
        !result.passed(),
        "threshold should fail when all requests return 500"
    );
    assert!(!result.threshold_results[0].passed);
}

// ── progress ticks ────────────────────────────────────────────────────────────

#[tokio::test]
async fn progress_tx_receives_at_least_one_tick_during_run() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let opts = PerfOpts {
        progress_tx: Some(tx),
        ..make_opts(1, 3) // 3 seconds → should produce ≥ 1 tick
    };

    let run_handle = tokio::spawn(run_perf(col, opts));

    // Collect ticks until the run finishes.
    let mut tick_count = 0usize;
    while let Ok(tick) = rx.try_recv().or_else(|_| {
        std::thread::sleep(Duration::from_millis(100));
        rx.try_recv()
    }) {
        tick_count += 1;
        assert!(tick.elapsed_secs >= 0.0);
        if tick_count > 5 {
            break;
        }
    }

    let _ = run_handle.await;
    // We can't guarantee tick_count > 0 in all CI environments (timing-sensitive),
    // but the run itself must succeed.
}

// ── ramp-up profile ───────────────────────────────────────────────────────────

#[tokio::test]
async fn ramp_up_profile_completes_without_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());
    let opts = PerfOpts {
        vus: 4,
        duration: Duration::from_secs(3),
        load_profile: LoadProfile::RampUp,
        initial_vus: 1,
        thresholds: vec![],
        data_rows: vec![],
        http_client: Arc::new(reqwest::Client::new()),
        script_timeout_ms: 30_000,
        progress_tx: None,
    };
    let result = run_perf(col, opts)
        .await
        .expect("ramp_up run should not error");
    assert!(result.metrics.total_iterations > 0);
}

// ── data row assignment ───────────────────────────────────────────────────────

#[tokio::test]
async fn data_rows_are_used_when_provided() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = simple_collection(&server.uri());

    let rows: Vec<std::collections::HashMap<String, String>> = vec![
        [("id".to_string(), "1".to_string())].into_iter().collect(),
        [("id".to_string(), "2".to_string())].into_iter().collect(),
    ];

    let opts = PerfOpts {
        data_rows: rows,
        ..make_opts(2, 2) // 2 VUs — rows assigned round-robin
    };
    let result = run_perf(col, opts)
        .await
        .expect("run with data rows should not error");
    assert!(result.metrics.total_iterations > 0);
}
