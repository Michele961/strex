//! Run history persistence — save, list, and load past UI runs.
//!
//! Files are stored in `.strex-history/` next to the process working directory.
//! Each file is named `YYYY-MM-DDTHH-MM-SS-<collection_stem>.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Metadata summary returned by `list_runs`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RunSummary {
    /// Filename used as the opaque run identifier for `load_run`.
    pub id: String,
    /// ISO-8601 timestamp extracted from the filename.
    pub timestamp: String,
    /// Collection name extracted from the filename.
    pub collection: String,
    /// Number of requests that passed.
    pub passed: usize,
    /// Number of requests that failed.
    pub failed: usize,
    /// Number of requests that were skipped.
    pub skipped: usize,
}

fn history_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(".strex-history")
}

/// Save a run payload to disk. Returns the filename on success.
pub(crate) fn save_run(
    collection_name: &str,
    passed: usize,
    failed: usize,
    skipped: usize,
    payload: &serde_json::Value,
) -> anyhow::Result<String> {
    let dir = history_dir();
    std::fs::create_dir_all(&dir)?;

    let now = chrono::Utc::now();
    let timestamp = now.format("%Y-%m-%dT%H-%M-%S").to_string();

    let stem: String = collection_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();

    let filename = format!("{timestamp}-{stem}.json");
    let path = dir.join(&filename);

    let envelope = serde_json::json!({
        "id": filename,
        "timestamp": now.to_rfc3339(),
        "collection": collection_name,
        "passed": passed,
        "failed": failed,
        "skipped": skipped,
        "run": payload,
    });

    std::fs::write(&path, serde_json::to_string_pretty(&envelope)?)?;
    Ok(filename)
}

/// Return a list of all saved runs ordered newest-first.
pub(crate) fn list_runs() -> Vec<RunSummary> {
    let dir = history_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };

    let mut summaries: Vec<(std::time::SystemTime, RunSummary)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == "json")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let path = e.path();
            let content = std::fs::read_to_string(&path).ok()?;
            let v: serde_json::Value = serde_json::from_str(&content).ok()?;
            let summary = RunSummary {
                id: v["id"].as_str()?.to_string(),
                timestamp: v["timestamp"].as_str()?.to_string(),
                collection: v["collection"].as_str()?.to_string(),
                passed: v["passed"].as_u64().unwrap_or(0) as usize,
                failed: v["failed"].as_u64().unwrap_or(0) as usize,
                skipped: v["skipped"].as_u64().unwrap_or(0) as usize,
            };
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((mtime, summary))
        })
        .collect();

    summaries.sort_by(|a, b| b.0.cmp(&a.0));
    summaries.into_iter().map(|(_, s)| s).collect()
}

/// Load the full payload for a single run by its filename id.
pub(crate) fn load_run(id: &str) -> anyhow::Result<serde_json::Value> {
    let path = history_dir().join(id);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Could not read history file '{id}': {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Could not parse history file '{id}': {e}"))?;
    Ok(v)
}
