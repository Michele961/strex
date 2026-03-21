//! Performance / load-testing orchestration for Strex.
//!
//! This module provides:
//!
//! - [`PerformanceConfig`] — optional YAML block embedded in a [`Collection`] file.
//! - [`PerfOpts`] — resolved runtime options (CLI args override YAML defaults).
//! - [`run_perf`] — the core async function that spawns VU tasks, collects metrics,
//!   and evaluates thresholds.
//! - [`PerfMetrics`] / [`PerfResult`] / [`ThresholdResult`] — output types.
//! - [`PerfTick`] — live progress snapshot emitted every second during a run.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::collection::Collection;
use crate::context::ExecutionContext;
use crate::data::DataRow;
use crate::runner::{execute_collection_with_opts, RequestOutcome, RunnerOpts};

// ─── Load profile ────────────────────────────────────────────────────────────

/// How virtual users are introduced over the test duration.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadProfile {
    /// Keep a constant number of VUs for the full duration.
    #[default]
    Fixed,
    /// Start at [`PerformanceConfig::initial_vus`], ramp linearly to
    /// [`PerformanceConfig::vus`] over the first half of the duration, then
    /// hold at the peak count for the remainder.
    RampUp,
}

// ─── Threshold types ─────────────────────────────────────────────────────────

/// Metric identifier used in a [`Threshold`] expression.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    /// Arithmetic mean of iteration durations in milliseconds.
    AvgResponseMs,
    /// 95th-percentile iteration duration in milliseconds.
    P95ResponseMs,
    /// 99th-percentile iteration duration in milliseconds.
    P99ResponseMs,
    /// Percentage of iterations that produced at least one error or assertion
    /// failure (0.0 – 100.0).
    ErrorRatePct,
    /// Completed iterations per second.
    ThroughputRps,
}

/// Comparison operator for a [`Threshold`] condition.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdCondition {
    /// Metric must be strictly less than `value`.
    Lt,
    /// Metric must be less than or equal to `value`.
    Lte,
    /// Metric must be strictly greater than `value`.
    Gt,
    /// Metric must be greater than or equal to `value`.
    Gte,
}

/// A single pass/fail gate evaluated against the final [`PerfMetrics`].
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Threshold {
    /// Which metric to measure.
    pub metric: MetricKind,
    /// Comparison direction.
    pub condition: ThresholdCondition,
    /// The numeric value to compare against (ms, percentage, or rps depending
    /// on [`MetricKind`]).
    pub value: f64,
}

// ─── YAML config block ────────────────────────────────────────────────────────

fn default_vus() -> usize {
    1
}
fn default_duration_secs() -> u64 {
    60
}
fn default_initial_vus() -> usize {
    1
}

/// Optional `performance:` block embedded in a Strex collection file.
///
/// All fields have sensible defaults so that a minimal block such as
/// `performance: {vus: 20, duration_secs: 60}` is valid.
///
/// The `perf` CLI subcommand uses this as a baseline; CLI flags always
/// override these values.  The `run` and `validate` subcommands ignore this
/// field entirely.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct PerformanceConfig {
    /// Number of virtual users (concurrent collection runners). Default: 1.
    #[serde(default = "default_vus")]
    pub vus: usize,
    /// Total test duration in seconds. Default: 60.
    #[serde(default = "default_duration_secs")]
    pub duration_secs: u64,
    /// Load profile controlling how VUs are introduced. Default: `fixed`.
    #[serde(default)]
    pub load_profile: LoadProfile,
    /// Starting VU count for the `ramp_up` profile. Ignored for `fixed`.
    /// Default: 1.
    #[serde(default = "default_initial_vus")]
    pub initial_vus: usize,
    /// Pass/fail thresholds evaluated after the test completes. Default: empty
    /// (test always passes when no thresholds are defined).
    #[serde(default)]
    pub thresholds: Vec<Threshold>,
}

// ─── Runtime options ──────────────────────────────────────────────────────────

/// Runtime options for a performance test run.
///
/// Constructed from CLI arguments (which override any [`PerformanceConfig`]
/// values embedded in the collection file).
pub struct PerfOpts {
    /// Number of virtual users.
    pub vus: usize,
    /// Total test duration.
    pub duration: Duration,
    /// Load profile.
    pub load_profile: LoadProfile,
    /// Starting VU count for `RampUp`. Ignored for `Fixed`.
    pub initial_vus: usize,
    /// Thresholds to evaluate after the test.
    pub thresholds: Vec<Threshold>,
    /// Data rows assigned round-robin to VUs. Empty means no data file.
    pub data_rows: Vec<DataRow>,
    /// Shared HTTP client — one per run to reuse the connection pool.
    pub http_client: Arc<reqwest::Client>,
    /// Per-script CPU timeout forwarded to [`RunnerOpts`].
    pub script_timeout_ms: u64,
    /// Optional channel for live progress ticks (one per second).
    ///
    /// The CLI leaves this as `None`.  The web UI sets it to receive
    /// [`PerfTick`] values that are forwarded over the WebSocket.
    pub progress_tx: Option<tokio::sync::mpsc::Sender<PerfTick>>,
}

// ─── Live progress tick ───────────────────────────────────────────────────────

/// Per-request rolling stats emitted in each PerfTick.
///
/// p95 is omitted because sorting per-request durations on every tick is too
/// expensive; only the aggregate p95 (over all requests) is available live.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestTick {
    pub name: String,
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub throughput_rps: f64,
    pub avg_response_ms: f64,
    pub error_rate_pct: f64,
}

/// A live progress snapshot emitted approximately once per second during a run.
///
/// Sent over [`PerfOpts::progress_tx`] if present.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PerfTick {
    /// Wall-clock seconds elapsed since the test started.
    pub elapsed_secs: f64,
    /// Total completed iterations so far.
    pub total_iterations: u64,
    /// Iterations that passed.
    pub passed_iterations: u64,
    /// Iterations that failed.
    pub failed_iterations: u64,
    /// Current throughput in iterations per second.
    pub throughput_rps: f64,
    /// Current error rate as a percentage (0.0 – 100.0).
    pub error_rate_pct: f64,
    /// Current mean iteration duration in milliseconds.
    pub avg_response_ms: f64,
    /// Current 95th-percentile iteration duration in milliseconds.
    pub p95_response_ms: f64,
    /// Per-request rolling statistics.
    pub per_request: Vec<RequestTick>,
}

// ─── Metrics & results ────────────────────────────────────────────────────────

/// Per-request sample data extracted from a single request execution.
#[derive(Debug, Clone)]
struct RequestSample {
    /// Request name from the collection.
    name: String,
    /// Duration of this specific request in milliseconds.
    duration_ms: u64,
    /// `true` if this request passed all assertions.
    passed: bool,
}

/// Raw sample produced by one completed collection iteration inside a VU task.
#[derive(Debug, Clone)]
struct IterationSample {
    /// Total duration of all requests in the iteration (ms).
    duration_ms: u64,
    /// `true` if every request in the iteration passed.
    passed: bool,
    /// Per-request samples for this iteration (Skipped requests excluded).
    requests: Vec<RequestSample>,
}

/// Aggregated performance metrics computed after all VU tasks complete.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PerfMetrics {
    /// Total number of completed iterations.
    pub total_iterations: u64,
    /// Iterations where every request passed.
    pub passed_iterations: u64,
    /// Iterations with at least one failure or error.
    pub failed_iterations: u64,
    /// Arithmetic mean of iteration durations in milliseconds.
    pub avg_response_ms: f64,
    /// Minimum observed iteration duration in milliseconds.
    pub min_response_ms: f64,
    /// Maximum observed iteration duration in milliseconds.
    pub max_response_ms: f64,
    /// 50th-percentile (median) iteration duration in milliseconds.
    pub p50_response_ms: f64,
    /// 95th-percentile iteration duration in milliseconds.
    pub p95_response_ms: f64,
    /// 99th-percentile iteration duration in milliseconds.
    pub p99_response_ms: f64,
    /// Error rate as a percentage (0.0 – 100.0).
    pub error_rate_pct: f64,
    /// Throughput in iterations per second.
    pub throughput_rps: f64,
    /// Actual elapsed wall-clock duration in seconds.
    pub elapsed_secs: f64,
    /// Per-request final metrics with full percentile set.
    pub per_request: Vec<RequestMetrics>,
}

/// Per-request final stats emitted in PerfMetrics.
///
/// Full percentile set available because it is computed once at the end.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestMetrics {
    pub name: String,
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub avg_response_ms: f64,
    pub min_response_ms: f64,
    pub max_response_ms: f64,
    pub p50_response_ms: f64,
    pub p95_response_ms: f64,
    pub p99_response_ms: f64,
    pub error_rate_pct: f64,
    pub throughput_rps: f64,
}

/// Result of evaluating a single [`Threshold`] against final [`PerfMetrics`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ThresholdResult {
    /// The threshold that was evaluated.
    pub threshold: Threshold,
    /// The observed metric value.
    pub observed: f64,
    /// `true` if the threshold condition was satisfied.
    pub passed: bool,
}

/// Complete result of a performance test run.
#[derive(Debug)]
pub struct PerfResult {
    /// Aggregate metrics over all completed iterations.
    pub metrics: PerfMetrics,
    /// One entry per threshold in [`PerfOpts::thresholds`]; empty if none were
    /// defined.
    pub threshold_results: Vec<ThresholdResult>,
}

impl PerfResult {
    /// Returns `true` if all thresholds passed (or no thresholds were defined).
    pub fn passed(&self) -> bool {
        self.threshold_results.iter().all(|t| t.passed)
    }
}

// ─── Errors ───────────────────────────────────────────────────────────────────

/// Errors that can occur during performance test orchestration.
#[derive(thiserror::Error, Debug)]
pub enum PerfError {
    /// `PerfOpts::vus` is 0.
    #[error("vus must be at least 1")]
    InvalidVus,
    /// `PerfOpts::initial_vus` exceeds `PerfOpts::vus`.
    #[error("initial_vus ({initial}) must be <= vus ({target})")]
    InitialVusExceedsVus { initial: usize, target: usize },
    /// `PerfOpts::duration` is zero.
    #[error("duration must be at least 1 second")]
    InvalidDuration,
    /// A VU task panicked.
    #[error("VU task panicked: {cause}")]
    TaskPanic { cause: String },
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Return the value at percentile `p` (0.0–1.0) of a **sorted** slice.
///
/// Returns `0.0` for an empty slice.
pub(crate) fn percentile(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p) as usize;
    sorted[idx] as f64
}

fn compute_per_request_tick(samples: &[IterationSample], elapsed_secs: f64) -> Vec<RequestTick> {
    let mut request_groups: Vec<(String, Vec<(u64, bool)>)> = Vec::new();

    for sample in samples {
        for req in &sample.requests {
            if let Some((_, group)) = request_groups
                .iter_mut()
                .find(|(name, _)| name == &req.name)
            {
                group.push((req.duration_ms, req.passed));
            } else {
                request_groups.push((req.name.clone(), vec![(req.duration_ms, req.passed)]));
            }
        }
    }

    request_groups
        .into_iter()
        .map(|(name, entries)| {
            let total = entries.len() as u64;
            let passed = entries.iter().filter(|(_, p)| *p).count() as u64;
            let failed = total - passed;
            let sum: u64 = entries.iter().map(|(d, _)| d).sum();
            let avg_response_ms = if total > 0 {
                sum as f64 / total as f64
            } else {
                0.0
            };
            let error_rate_pct = if total > 0 {
                failed as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            let throughput_rps = if elapsed_secs > 0.0 {
                total as f64 / elapsed_secs
            } else {
                0.0
            };

            RequestTick {
                name,
                total,
                passed,
                failed,
                throughput_rps,
                avg_response_ms,
                error_rate_pct,
            }
        })
        .collect()
}

fn compute_per_request_metrics(
    samples: &[IterationSample],
    elapsed_secs: f64,
) -> Vec<RequestMetrics> {
    let mut request_groups: Vec<(String, Vec<(u64, bool)>)> = Vec::new();

    for sample in samples {
        for req in &sample.requests {
            if let Some((_, group)) = request_groups
                .iter_mut()
                .find(|(name, _)| name == &req.name)
            {
                group.push((req.duration_ms, req.passed));
            } else {
                request_groups.push((req.name.clone(), vec![(req.duration_ms, req.passed)]));
            }
        }
    }

    request_groups
        .into_iter()
        .map(|(name, entries)| {
            let total = entries.len() as u64;
            let passed = entries.iter().filter(|(_, p)| *p).count() as u64;
            let failed = total - passed;

            let mut durations: Vec<u64> = entries.iter().map(|(d, _)| *d).collect();
            durations.sort_unstable();

            let sum: u64 = durations.iter().sum();
            let avg_response_ms = if total > 0 {
                sum as f64 / total as f64
            } else {
                0.0
            };
            let min_response_ms = *durations.first().unwrap_or(&0) as f64;
            let max_response_ms = *durations.last().unwrap_or(&0) as f64;
            let p50_response_ms = percentile(&durations, 0.50);
            let p95_response_ms = percentile(&durations, 0.95);
            let p99_response_ms = percentile(&durations, 0.99);

            let error_rate_pct = if total > 0 {
                failed as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            let throughput_rps = if elapsed_secs > 0.0 {
                total as f64 / elapsed_secs
            } else {
                0.0
            };

            RequestMetrics {
                name,
                total,
                passed,
                failed,
                avg_response_ms,
                min_response_ms,
                max_response_ms,
                p50_response_ms,
                p95_response_ms,
                p99_response_ms,
                error_rate_pct,
                throughput_rps,
            }
        })
        .collect()
}

/// Compute [`PerfMetrics`] from an accumulated sample set.
fn compute_metrics(samples: &[IterationSample], elapsed_secs: f64) -> PerfMetrics {
    if samples.is_empty() {
        return PerfMetrics {
            total_iterations: 0,
            passed_iterations: 0,
            failed_iterations: 0,
            avg_response_ms: 0.0,
            min_response_ms: 0.0,
            max_response_ms: 0.0,
            p50_response_ms: 0.0,
            p95_response_ms: 0.0,
            p99_response_ms: 0.0,
            error_rate_pct: 0.0,
            throughput_rps: 0.0,
            elapsed_secs,
            per_request: vec![],
        };
    }

    let total = samples.len() as u64;
    let passed = samples.iter().filter(|s| s.passed).count() as u64;
    let failed = total - passed;

    let mut durations: Vec<u64> = samples.iter().map(|s| s.duration_ms).collect();
    durations.sort_unstable();

    let sum: u64 = durations.iter().sum();
    let avg = sum as f64 / total as f64;
    let min = *durations.first().unwrap_or(&0) as f64;
    let max = *durations.last().unwrap_or(&0) as f64;
    let p50 = percentile(&durations, 0.50);
    let p95 = percentile(&durations, 0.95);
    let p99 = percentile(&durations, 0.99);

    let error_rate = if total > 0 {
        failed as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let throughput = if elapsed_secs > 0.0 {
        total as f64 / elapsed_secs
    } else {
        0.0
    };

    let per_request = compute_per_request_metrics(samples, elapsed_secs);

    PerfMetrics {
        total_iterations: total,
        passed_iterations: passed,
        failed_iterations: failed,
        avg_response_ms: avg,
        min_response_ms: min,
        max_response_ms: max,
        p50_response_ms: p50,
        p95_response_ms: p95,
        p99_response_ms: p99,
        error_rate_pct: error_rate,
        throughput_rps: throughput,
        elapsed_secs,
        per_request,
    }
}

/// Compute a partial [`PerfTick`] from accumulated samples (called every second).
fn compute_tick(samples: &[IterationSample], elapsed_secs: f64) -> PerfTick {
    let total = samples.len() as u64;
    let passed = samples.iter().filter(|s| s.passed).count() as u64;
    let failed = total - passed;

    let mut durations: Vec<u64> = samples.iter().map(|s| s.duration_ms).collect();
    durations.sort_unstable();

    let avg = if total > 0 {
        durations.iter().sum::<u64>() as f64 / total as f64
    } else {
        0.0
    };
    let p95 = percentile(&durations, 0.95);
    let error_rate = if total > 0 {
        failed as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let throughput = if elapsed_secs > 0.0 {
        total as f64 / elapsed_secs
    } else {
        0.0
    };

    let per_request = compute_per_request_tick(samples, elapsed_secs);

    PerfTick {
        elapsed_secs,
        total_iterations: total,
        passed_iterations: passed,
        failed_iterations: failed,
        throughput_rps: throughput,
        error_rate_pct: error_rate,
        avg_response_ms: avg,
        p95_response_ms: p95,
        per_request,
    }
}

/// Evaluate all thresholds against final metrics and return one result per threshold.
pub fn evaluate_thresholds(
    thresholds: &[Threshold],
    metrics: &PerfMetrics,
) -> Vec<ThresholdResult> {
    thresholds
        .iter()
        .map(|t| {
            let observed = match t.metric {
                MetricKind::AvgResponseMs => metrics.avg_response_ms,
                MetricKind::P95ResponseMs => metrics.p95_response_ms,
                MetricKind::P99ResponseMs => metrics.p99_response_ms,
                MetricKind::ErrorRatePct => metrics.error_rate_pct,
                MetricKind::ThroughputRps => metrics.throughput_rps,
            };
            let passed = match t.condition {
                ThresholdCondition::Lt => observed < t.value,
                ThresholdCondition::Lte => observed <= t.value,
                ThresholdCondition::Gt => observed > t.value,
                ThresholdCondition::Gte => observed >= t.value,
            };
            ThresholdResult {
                threshold: t.clone(),
                observed,
                passed,
            }
        })
        .collect()
}

// ─── VU task ──────────────────────────────────────────────────────────────────

/// The looping body executed by each virtual user.
///
/// Runs the collection repeatedly until `token` is cancelled, pushing an
/// [`IterationSample`] into `samples` after each iteration completes.
async fn run_vu(
    token: CancellationToken,
    col: Arc<Collection>,
    samples: Arc<Mutex<Vec<IterationSample>>>,
    client: Arc<reqwest::Client>,
    script_timeout_ms: u64,
    vu_rows: Vec<DataRow>,
) {
    let mut row_cursor = 0usize;

    loop {
        // Check for cancellation before starting a new iteration.
        if token.is_cancelled() {
            break;
        }

        let row = if vu_rows.is_empty() {
            None
        } else {
            let r = vu_rows[row_cursor % vu_rows.len()].clone();
            row_cursor = row_cursor.wrapping_add(1);
            Some(r)
        };

        let ctx = match &row {
            Some(r) => ExecutionContext::new_with_data(&col, r),
            None => ExecutionContext::new(&col),
        };

        let runner_opts = RunnerOpts {
            http_client: Arc::clone(&client),
            script_timeout_ms,
            ..RunnerOpts::default()
        };

        // Run the collection or break early if cancellation fires mid-run.
        tokio::select! {
            biased;
            _ = token.cancelled() => break,
            col_result = execute_collection_with_opts(&col, ctx, runner_opts) => {
                let duration_ms: u64 = col_result
                    .request_results
                    .iter()
                    .map(|r| r.duration_ms)
                    .sum();
                let passed = col_result.passed();

                let requests: Vec<RequestSample> = col_result.request_results
                    .iter()
                    .filter(|r| !matches!(r.outcome, RequestOutcome::Skipped))
                    .map(|r| RequestSample {
                        name: r.name.clone(),
                        duration_ms: r.duration_ms,
                        passed: matches!(r.outcome, RequestOutcome::Passed),
                    })
                    .collect();

                // Lock is held only briefly — never across an await point.
                if let Ok(mut guard) = samples.lock() {
                    guard.push(IterationSample { duration_ms, passed, requests });
                }
            }
        }
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Run a performance test: spawn VUs, collect metrics, evaluate thresholds.
///
/// VUs execute the collection in a loop until `opts.duration` elapses.
/// A [`CancellationToken`] is broadcast to all VU tasks when the timer fires.
///
/// If `opts.progress_tx` is `Some`, a tick message is sent approximately
/// once per second with live metrics. The channel is not closed by this
/// function — callers must drop their receiver after `run_perf` returns.
///
/// # Errors
///
/// Returns [`PerfError::InvalidVus`] when `opts.vus == 0`.
/// Returns [`PerfError::InvalidDuration`] when `opts.duration` is zero.
/// Returns [`PerfError::InitialVusExceedsVus`] when `opts.initial_vus > opts.vus`.
/// Returns [`PerfError::TaskPanic`] if a VU task panics.
pub async fn run_perf(collection: Collection, opts: PerfOpts) -> Result<PerfResult, PerfError> {
    // ── Validate options ────────────────────────────────────────────────────
    if opts.vus == 0 {
        return Err(PerfError::InvalidVus);
    }
    if opts.duration.is_zero() {
        return Err(PerfError::InvalidDuration);
    }
    if opts.initial_vus > opts.vus {
        return Err(PerfError::InitialVusExceedsVus {
            initial: opts.initial_vus,
            target: opts.vus,
        });
    }

    // ── Shared state ────────────────────────────────────────────────────────
    let token = CancellationToken::new();
    let samples: Arc<Mutex<Vec<IterationSample>>> = Arc::new(Mutex::new(Vec::new()));
    let arc_col = Arc::new(collection);
    let start = Instant::now();

    // ── Pre-distribute data rows round-robin across VUs ─────────────────────
    let vu_data_rows: Vec<Vec<DataRow>> = (0..opts.vus)
        .map(|vu_idx| {
            opts.data_rows
                .iter()
                .enumerate()
                .filter(|(row_idx, _)| row_idx % opts.vus == vu_idx)
                .map(|(_, row)| row.clone())
                .collect()
        })
        .collect();

    // ── Timer task — cancels all VUs after `opts.duration` ──────────────────
    let timer_token = token.clone();
    let duration = opts.duration;
    tokio::spawn(async move {
        tokio::time::sleep(duration).await;
        timer_token.cancel();
    });

    // ── Optional progress tick task — fires every second ────────────────────
    if let Some(tx) = opts.progress_tx.clone() {
        let tick_samples = Arc::clone(&samples);
        let tick_token = token.clone();
        let tick_start = start;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            // Consume the first (immediate) tick so the first send fires at t≈1s.
            interval.tick().await;
            loop {
                tokio::select! {
                    biased;
                    _ = tick_token.cancelled() => break,
                    _ = interval.tick() => {
                        let snap = tick_samples
                            .lock()
                            .map(|g| g.clone())
                            .unwrap_or_default();
                        let elapsed = tick_start.elapsed().as_secs_f64();
                        let tick = compute_tick(&snap, elapsed);
                        // Ignore send error — receiver may have been dropped.
                        let _ = tx.send(tick).await;
                    }
                }
            }
        });
    }

    // ── Spawn VU tasks based on load profile ────────────────────────────────
    let mut join_set: JoinSet<()> = JoinSet::new();

    match opts.load_profile {
        LoadProfile::Fixed => {
            for vu_rows in &vu_data_rows {
                join_set.spawn(run_vu(
                    token.clone(),
                    Arc::clone(&arc_col),
                    Arc::clone(&samples),
                    Arc::clone(&opts.http_client),
                    opts.script_timeout_ms,
                    vu_rows.clone(),
                ));
            }
        }
        LoadProfile::RampUp => {
            // Ramp period = first half of total duration.
            let ramp_duration = opts.duration / 2;
            let ramp_vus = opts.vus.saturating_sub(opts.initial_vus);
            let spawn_interval = if ramp_vus == 0 {
                Duration::ZERO
            } else {
                ramp_duration / ramp_vus as u32
            };

            // Phase 1 — spawn initial_vus immediately.
            for vu_rows in vu_data_rows.iter().take(opts.initial_vus) {
                join_set.spawn(run_vu(
                    token.clone(),
                    Arc::clone(&arc_col),
                    Arc::clone(&samples),
                    Arc::clone(&opts.http_client),
                    opts.script_timeout_ms,
                    vu_rows.clone(),
                ));
            }

            // Phase 2 — ramp up remaining VUs at evenly-spaced intervals.
            for vu_rows in vu_data_rows.iter().skip(opts.initial_vus) {
                if !spawn_interval.is_zero() {
                    tokio::time::sleep(spawn_interval).await;
                }
                // Don't spawn more VUs if the test already ended.
                if token.is_cancelled() {
                    break;
                }
                join_set.spawn(run_vu(
                    token.clone(),
                    Arc::clone(&arc_col),
                    Arc::clone(&samples),
                    Arc::clone(&opts.http_client),
                    opts.script_timeout_ms,
                    vu_rows.clone(),
                ));
            }
        }
    }

    // ── Wait for all VU tasks to finish ─────────────────────────────────────
    while let Some(res) = join_set.join_next().await {
        if let Err(join_err) = res {
            return Err(PerfError::TaskPanic {
                cause: join_err.to_string(),
            });
        }
    }

    // ── Compute final metrics and evaluate thresholds ────────────────────────
    let elapsed_secs = start.elapsed().as_secs_f64();
    let final_samples = samples.lock().map(|g| g.clone()).unwrap_or_default();

    let metrics = compute_metrics(&final_samples, elapsed_secs);
    let threshold_results = evaluate_thresholds(&opts.thresholds, &metrics);

    Ok(PerfResult {
        metrics,
        threshold_results,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── percentile ────────────────────────────────────────────────────────────

    #[test]
    fn percentile_empty_returns_zero() {
        assert_eq!(percentile(&[], 0.5), 0.0);
    }

    #[test]
    fn percentile_single_element_always_returns_that_element() {
        let s = vec![42u64];
        assert_eq!(percentile(&s, 0.0), 42.0);
        assert_eq!(percentile(&s, 0.5), 42.0);
        assert_eq!(percentile(&s, 0.99), 42.0);
    }

    #[test]
    fn percentile_sorted_ten_elements() {
        // [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
        let s: Vec<u64> = (1..=10).map(|x| x * 10).collect();
        // p50 → idx = floor(9 * 0.5) = 4 → 50
        assert_eq!(percentile(&s, 0.50), 50.0);
        // p95 → idx = floor(9 * 0.95) = 8 → 90
        assert_eq!(percentile(&s, 0.95), 90.0);
        // p99 → idx = floor(9 * 0.99) = 8 → 90
        assert_eq!(percentile(&s, 0.99), 90.0);
    }

    // ── evaluate_thresholds ───────────────────────────────────────────────────

    fn zero_metrics() -> PerfMetrics {
        PerfMetrics {
            total_iterations: 100,
            passed_iterations: 100,
            failed_iterations: 0,
            avg_response_ms: 100.0,
            min_response_ms: 50.0,
            max_response_ms: 200.0,
            p50_response_ms: 95.0,
            p95_response_ms: 180.0,
            p99_response_ms: 195.0,
            error_rate_pct: 0.0,
            throughput_rps: 20.0,
            elapsed_secs: 5.0,
        }
    }

    #[test]
    fn evaluate_thresholds_all_pass() {
        let thresholds = vec![
            Threshold {
                metric: MetricKind::P95ResponseMs,
                condition: ThresholdCondition::Lt,
                value: 500.0,
            },
            Threshold {
                metric: MetricKind::ErrorRatePct,
                condition: ThresholdCondition::Lt,
                value: 1.0,
            },
            Threshold {
                metric: MetricKind::ThroughputRps,
                condition: ThresholdCondition::Gt,
                value: 10.0,
            },
        ];
        let results = evaluate_thresholds(&thresholds, &zero_metrics());
        assert!(results.iter().all(|r| r.passed));
    }

    #[test]
    fn evaluate_thresholds_lt_fails_when_equal() {
        let threshold = Threshold {
            metric: MetricKind::ErrorRatePct,
            condition: ThresholdCondition::Lt,
            value: 0.0, // equal to observed (0.0) → strict less-than → FAIL
        };
        let results = evaluate_thresholds(&[threshold], &zero_metrics());
        assert!(!results[0].passed);
    }

    #[test]
    fn evaluate_thresholds_lte_passes_when_equal() {
        let threshold = Threshold {
            metric: MetricKind::ErrorRatePct,
            condition: ThresholdCondition::Lte,
            value: 0.0, // equal → lte → PASS
        };
        let results = evaluate_thresholds(&[threshold], &zero_metrics());
        assert!(results[0].passed);
    }

    #[test]
    fn evaluate_thresholds_gte_fails_when_below() {
        let threshold = Threshold {
            metric: MetricKind::ThroughputRps,
            condition: ThresholdCondition::Gte,
            value: 100.0, // observed 20.0 < 100.0 → FAIL
        };
        let results = evaluate_thresholds(&[threshold], &zero_metrics());
        assert!(!results[0].passed);
        assert!((results[0].observed - 20.0).abs() < f64::EPSILON);
    }

    // ── PerfError display ─────────────────────────────────────────────────────

    #[test]
    fn perf_error_invalid_vus_display() {
        assert_eq!(PerfError::InvalidVus.to_string(), "vus must be at least 1");
    }

    #[test]
    fn perf_error_invalid_duration_display() {
        assert_eq!(
            PerfError::InvalidDuration.to_string(),
            "duration must be at least 1 second"
        );
    }

    #[test]
    fn perf_error_initial_vus_exceeds_vus_display() {
        let e = PerfError::InitialVusExceedsVus {
            initial: 5,
            target: 2,
        };
        assert_eq!(e.to_string(), "initial_vus (5) must be <= vus (2)");
    }

    #[test]
    fn perf_error_task_panic_display() {
        let e = PerfError::TaskPanic {
            cause: "oh no".to_string(),
        };
        assert_eq!(e.to_string(), "VU task panicked: oh no");
    }

    // ── LoadProfile ───────────────────────────────────────────────────────────

    #[test]
    fn load_profile_default_is_fixed() {
        assert_eq!(LoadProfile::default(), LoadProfile::Fixed);
    }

    // ── PerfResult::passed ────────────────────────────────────────────────────

    #[test]
    fn perf_result_passed_when_no_thresholds() {
        let result = PerfResult {
            metrics: zero_metrics(),
            threshold_results: vec![],
        };
        assert!(result.passed());
    }

    #[test]
    fn perf_result_failed_when_any_threshold_fails() {
        let result = PerfResult {
            metrics: zero_metrics(),
            threshold_results: vec![ThresholdResult {
                threshold: Threshold {
                    metric: MetricKind::ErrorRatePct,
                    condition: ThresholdCondition::Lt,
                    value: 0.0,
                },
                observed: 0.0,
                passed: false,
            }],
        };
        assert!(!result.passed());
    }

    // ── compute_metrics ───────────────────────────────────────────────────────

    #[test]
    fn compute_metrics_empty_samples_returns_zeroes() {
        let m = compute_metrics(&[], 10.0);
        assert_eq!(m.total_iterations, 0);
        assert_eq!(m.throughput_rps, 0.0);
        assert_eq!(m.error_rate_pct, 0.0);
    }

    #[test]
    fn compute_metrics_all_passed_zero_error_rate() {
        let samples = vec![
            IterationSample {
                duration_ms: 100,
                passed: true,
                requests: vec![],
            },
            IterationSample {
                duration_ms: 200,
                passed: true,
                requests: vec![],
            },
        ];
        let m = compute_metrics(&samples, 1.0);
        assert_eq!(m.total_iterations, 2);
        assert_eq!(m.passed_iterations, 2);
        assert_eq!(m.failed_iterations, 0);
        assert_eq!(m.error_rate_pct, 0.0);
        assert!((m.avg_response_ms - 150.0).abs() < f64::EPSILON);
        assert_eq!(m.throughput_rps, 2.0);
    }

    #[test]
    fn compute_metrics_zero_elapsed_throughput_is_zero() {
        let samples = vec![IterationSample {
            duration_ms: 50,
            passed: true,
            requests: vec![],
        }];
        let m = compute_metrics(&samples, 0.0);
        assert_eq!(m.throughput_rps, 0.0);
    }
}
