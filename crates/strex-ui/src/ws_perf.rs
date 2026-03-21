//! WebSocket handler for performance test runs — streams [`PerfWsEvent`]s to the browser.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::Deserialize;

use strex_core::perf::{
    LoadProfile, MetricKind, PerfOpts, PerfTick, Threshold, ThresholdCondition,
};
use strex_core::{parse_collection, parse_csv, parse_json};

use crate::events::PerfWsEvent;

/// Configuration sent by the browser as the first WebSocket message.
#[derive(Debug, Deserialize)]
pub struct PerfRunConfig {
    /// Path to the collection YAML file.
    pub collection: String,
    /// Number of virtual users. Falls back to `performance.vus` in the YAML, then 1.
    #[serde(default)]
    pub vus: Option<usize>,
    /// Test duration in seconds. Falls back to `performance.duration_secs`, then 60.
    #[serde(default)]
    pub duration_secs: Option<u64>,
    /// Load profile: `"fixed"` (default) or `"ramp_up"`.
    #[serde(default)]
    pub load_profile: Option<String>,
    /// Starting VU count for `ramp_up`. Falls back to `performance.initial_vus`, then 1.
    #[serde(default)]
    pub initial_vus: Option<usize>,
    /// Threshold strings in `METRIC:CONDITION:VALUE` format.
    #[serde(default)]
    pub thresholds: Vec<String>,
    /// Optional data file path (.csv or .json).
    #[serde(default)]
    pub data: Option<String>,
}

/// Send a [`PerfWsEvent`] serialized as JSON text over the WebSocket.
async fn send_perf_event(socket: &mut WebSocket, event: PerfWsEvent) {
    if let Ok(json) = serde_json::to_string(&event) {
        let _ = socket.send(Message::Text(json)).await;
    }
}

/// Send a fatal error message and signal that the run failed.
async fn send_perf_error(socket: &mut WebSocket, message: String) {
    let json = serde_json::json!({ "type": "error", "message": message }).to_string();
    let _ = socket.send(Message::Text(json)).await;
}

/// Parse a threshold string `METRIC:CONDITION:VALUE`.
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
        other => anyhow::bail!("unknown metric: {other}"),
    };
    let condition = match parts[1] {
        "lt" => ThresholdCondition::Lt,
        "lte" => ThresholdCondition::Lte,
        "gt" => ThresholdCondition::Gt,
        "gte" => ThresholdCondition::Gte,
        other => anyhow::bail!("unknown condition: {other}"),
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

/// WebSocket upgrade handler — upgrades the HTTP connection and starts `handle_perf_socket`.
pub async fn ws_perf_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_perf_socket)
}

/// Drive the performance run for one WebSocket connection.
async fn handle_perf_socket(mut socket: WebSocket) {
    // Wait for the first text message containing the run configuration.
    let config = loop {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => match serde_json::from_str::<PerfRunConfig>(&text) {
                Ok(cfg) => break cfg,
                Err(e) => {
                    send_perf_error(&mut socket, format!("Invalid perf config: {e}")).await;
                    return;
                }
            },
            Some(Ok(_)) => continue,
            _ => return,
        }
    };

    if let Err(e) = run_perf_and_stream(&mut socket, config).await {
        send_perf_error(&mut socket, e.to_string()).await;
    }
}

/// Execute a performance test and stream [`PerfWsEvent`]s over `socket`.
async fn run_perf_and_stream(socket: &mut WebSocket, config: PerfRunConfig) -> anyhow::Result<()> {
    // ── Parse collection ──────────────────────────────────────────────────────
    let collection = parse_collection(Path::new(&config.collection))?;

    // ── Resolve opts (config overrides YAML performance block) ───────────────
    let perf_cfg = collection.performance.clone().unwrap_or_default();

    let vus = config.vus.unwrap_or(perf_cfg.vus).max(1);
    let duration_secs = config
        .duration_secs
        .unwrap_or(perf_cfg.duration_secs)
        .max(1);

    let load_profile = match config.load_profile.as_deref().unwrap_or("fixed") {
        "ramp_up" => LoadProfile::RampUp,
        _ => LoadProfile::Fixed,
    };
    let load_profile_label = match load_profile {
        LoadProfile::Fixed => "fixed",
        LoadProfile::RampUp => "ramp_up",
    };

    let initial_vus = config.initial_vus.unwrap_or(perf_cfg.initial_vus);
    anyhow::ensure!(
        initial_vus <= vus,
        "initial_vus ({initial_vus}) must be <= vus ({vus})"
    );

    // Merge threshold strings.
    let mut thresholds = perf_cfg.thresholds.clone();
    for s in &config.thresholds {
        thresholds.push(parse_threshold(s)?);
    }

    // ── Load optional data file ───────────────────────────────────────────────
    let data_rows = if let Some(ref data_path) = config.data {
        let content = std::fs::read_to_string(data_path)
            .map_err(|e| anyhow::anyhow!("Could not read data file '{data_path}': {e}"))?;
        if data_path.ends_with(".csv") {
            parse_csv(&content).map_err(|e| anyhow::anyhow!("CSV parse error: {e}"))?
        } else if data_path.ends_with(".json") {
            parse_json(&content).map_err(|e| anyhow::anyhow!("JSON parse error: {e}"))?
        } else {
            anyhow::bail!("Unsupported data file extension (expected .csv or .json): {data_path}");
        }
    } else {
        vec![]
    };

    // ── Set up progress tick channel ──────────────────────────────────────────
    let (tick_tx, mut tick_rx) = tokio::sync::mpsc::channel::<PerfTick>(64);

    let http_client = reqwest::Client::builder()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {e}"))?;

    let opts = PerfOpts {
        vus,
        duration: Duration::from_secs(duration_secs),
        load_profile,
        initial_vus,
        thresholds,
        data_rows,
        http_client: Arc::new(http_client),
        script_timeout_ms: 30_000,
        progress_tx: Some(tick_tx),
    };

    // ── Notify browser that the run is starting ───────────────────────────────
    send_perf_event(
        socket,
        PerfWsEvent::Started {
            vus,
            duration_secs,
            load_profile: load_profile_label.to_string(),
        },
    )
    .await;

    // ── Spawn the perf run in the background ──────────────────────────────────
    let run_handle = tokio::spawn(strex_core::run_perf(collection, opts));

    // ── Forward ticks to the browser as they arrive ───────────────────────────
    while let Some(tick) = tick_rx.recv().await {
        send_perf_event(
            socket,
            PerfWsEvent::Tick {
                elapsed_secs: tick.elapsed_secs,
                total_iterations: tick.total_iterations,
                passed_iterations: tick.passed_iterations,
                failed_iterations: tick.failed_iterations,
                throughput_rps: tick.throughput_rps,
                error_rate_pct: tick.error_rate_pct,
                avg_response_ms: tick.avg_response_ms,
                p95_response_ms: tick.p95_response_ms,
                per_request: tick.per_request,
            },
        )
        .await;
    }

    // ── Collect final result ──────────────────────────────────────────────────
    let perf_result = run_handle
        .await
        .map_err(|e| anyhow::anyhow!("VU task panicked: {e}"))?
        .map_err(|e| anyhow::anyhow!("Performance run failed: {e}"))?;

    // ── Send final summary ────────────────────────────────────────────────────
    let passed = perf_result.passed();
    send_perf_event(
        socket,
        PerfWsEvent::Finished {
            metrics: perf_result.metrics,
            threshold_results: perf_result.threshold_results,
            passed,
        },
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_run_config_deserializes_with_defaults() {
        let json = r#"{"collection":"./col.yaml"}"#;
        let cfg: PerfRunConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.collection, "./col.yaml");
        assert!(cfg.vus.is_none());
        assert!(cfg.duration_secs.is_none());
        assert!(cfg.load_profile.is_none());
        assert!(cfg.data.is_none());
        assert!(cfg.thresholds.is_empty());
    }

    #[test]
    fn perf_run_config_deserializes_full() {
        let json = r#"{
            "collection": "col.yaml",
            "vus": 20,
            "duration_secs": 60,
            "load_profile": "ramp_up",
            "initial_vus": 5,
            "thresholds": ["p95_response_ms:lt:500"],
            "data": "users.csv"
        }"#;
        let cfg: PerfRunConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.vus, Some(20));
        assert_eq!(cfg.duration_secs, Some(60));
        assert_eq!(cfg.load_profile.as_deref(), Some("ramp_up"));
        assert_eq!(cfg.initial_vus, Some(5));
        assert_eq!(cfg.thresholds, vec!["p95_response_ms:lt:500"]);
        assert_eq!(cfg.data.as_deref(), Some("users.csv"));
    }

    #[test]
    fn parse_threshold_valid() {
        let t = parse_threshold("p95_response_ms:lt:500").unwrap();
        assert!(matches!(t.metric, MetricKind::P95ResponseMs));
        assert!(matches!(t.condition, ThresholdCondition::Lt));
        assert!((t.value - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_threshold_invalid_metric_returns_err() {
        assert!(parse_threshold("bad_metric:lt:100").is_err());
    }

    #[test]
    fn parse_threshold_missing_parts_returns_err() {
        assert!(parse_threshold("p95_response_ms:lt").is_err());
    }
}
