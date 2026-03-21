//! Performance run history persistence — save, list, and load past perf runs.
//!
//! Files are stored in `.strex-perf-history/` next to the process working directory.
//! Each file is named `YYYY-MM-DDTHH-MM-SS-<collection_stem>.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Request body for saving a completed performance run.
///
/// All fields are required — axum's `Json` extractor returns `400` on missing fields.
#[derive(Debug, Deserialize)]
pub struct SavePerfRunRequest {
    pub collection: String,
    pub vus: usize,
    pub duration_secs: u64,
    pub load_profile: String,
    pub metrics: serde_json::Value,
    pub threshold_results: serde_json::Value,
    pub passed: bool,
    pub ticks: serde_json::Value,
}

/// Metadata summary returned by `list_perf_runs`.
#[derive(Debug, Serialize, Deserialize)]
pub struct PerfRunSummary {
    /// Filename used as the opaque run identifier for `load_perf_run`.
    pub id: String,
    /// ISO-8601 timestamp extracted from the filename.
    pub timestamp: String,
    /// Collection name extracted from the filename.
    pub collection: String,
    /// Number of virtual users.
    pub vus: usize,
    /// Duration in seconds.
    pub duration_secs: u64,
    /// Load profile: "fixed" or "ramp_up".
    pub load_profile: String,
    /// Total iterations across all VUs.
    pub total_iterations: u64,
    /// Average requests per second.
    pub throughput_rps: f64,
    /// Average response time in milliseconds.
    pub avg_response_ms: f64,
    /// 95th percentile response time in milliseconds.
    pub p95_response_ms: f64,
    /// Error rate percentage.
    pub error_rate_pct: f64,
    /// Whether all thresholds passed.
    pub passed: bool,
}

fn perf_history_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(".strex-perf-history")
}

/// Save a completed performance run to disk. Returns the filename on success.
pub(crate) fn save_perf_run(req: &SavePerfRunRequest) -> anyhow::Result<String> {
    let dir = perf_history_dir();
    std::fs::create_dir_all(&dir)?;

    let now = chrono::Utc::now();
    let timestamp = now.format("%Y-%m-%dT%H-%M-%S").to_string();

    let stem: String = req
        .collection
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let filename = format!("{timestamp}-{stem}.json");
    let path = dir.join(&filename);

    let envelope = serde_json::json!({
        "id": filename,
        "timestamp": now.to_rfc3339(),
        "collection": req.collection,
        "vus": req.vus,
        "duration_secs": req.duration_secs,
        "load_profile": req.load_profile,
        "metrics": req.metrics,
        "threshold_results": req.threshold_results,
        "passed": req.passed,
        "ticks": req.ticks,
    });

    std::fs::write(&path, serde_json::to_string_pretty(&envelope)?)?;
    Ok(filename)
}

/// Return a list of all saved performance runs ordered newest-first.
pub(crate) fn list_perf_runs() -> Vec<PerfRunSummary> {
    let dir = perf_history_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };

    let mut summaries: Vec<(std::time::SystemTime, PerfRunSummary)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .filter_map(|e| {
            let path = e.path();
            let content = std::fs::read_to_string(&path).ok()?;
            let v: serde_json::Value = serde_json::from_str(&content).ok()?;
            
            // Extract metrics for the summary
            let metrics = &v["metrics"];
            let summary = PerfRunSummary {
                id: v["id"].as_str()?.to_string(),
                timestamp: v["timestamp"].as_str()?.to_string(),
                collection: v["collection"].as_str()?.to_string(),
                vus: v["vus"].as_u64().unwrap_or(0) as usize,
                duration_secs: v["duration_secs"].as_u64().unwrap_or(0),
                load_profile: v["load_profile"].as_str()?.to_string(),
                total_iterations: metrics["total_iterations"].as_u64().unwrap_or(0),
                throughput_rps: metrics["throughput_rps"].as_f64().unwrap_or(0.0),
                avg_response_ms: metrics["avg_response_ms"].as_f64().unwrap_or(0.0),
                p95_response_ms: metrics["p95_response_ms"].as_f64().unwrap_or(0.0),
                error_rate_pct: metrics["error_rate_pct"].as_f64().unwrap_or(0.0),
                passed: v["passed"].as_bool().unwrap_or(false),
            };
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((mtime, summary))
        })
        .collect();

    summaries.sort_by(|a, b| b.0.cmp(&a.0));
    summaries.into_iter().map(|(_, s)| s).collect()
}

/// Load the full payload for a single performance run by its filename id.
pub(crate) fn load_perf_run(id: &str) -> anyhow::Result<serde_json::Value> {
    let path = perf_history_dir().join(id);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Could not read perf history file '{id}': {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Could not parse perf history file '{id}': {e}"))?;
    Ok(v)
}
