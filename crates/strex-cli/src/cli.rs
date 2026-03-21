use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// CLI-first API collection runner
#[derive(Parser)]
#[command(name = "strex", about = "CLI-first API collection runner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Available subcommands.
#[derive(Subcommand)]
pub enum Command {
    /// Run a collection, optionally with a data file
    Run(RunArgs),
    /// Validate a collection file without making HTTP requests
    Validate(ValidateArgs),
    /// Start the web UI server and open the browser
    Ui(UiArgs),
    /// Run a performance / load test against a collection
    Perf(PerfArgs),
}

/// Arguments for the `run` subcommand.
#[derive(Args)]
pub struct RunArgs {
    /// Path to the YAML collection file
    pub collection: PathBuf,
    /// Path to a CSV or JSON data file (enables data-driven mode)
    #[arg(long)]
    pub data: Option<PathBuf>,
    /// Maximum number of concurrent data-driven iterations (>= 1)
    #[arg(long, default_value = "1")]
    pub concurrency: usize,
    /// Stop launching new iterations after the first failure
    #[arg(long)]
    pub fail_fast: bool,
    /// Output format: console (default), json, junit
    #[arg(long, default_value = "console")]
    pub format: OutputFormat,
    /// Write output to this file instead of stdout
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Milliseconds to sleep before each request after the first one
    #[arg(long, default_value = "0")]
    pub delay_requests: u64,
    /// Milliseconds to sleep before each iteration after the first one
    #[arg(long, default_value = "0")]
    pub delay_iterations: u64,
}

/// Arguments for the `validate` subcommand.
#[derive(Args)]
pub struct ValidateArgs {
    /// Path to the YAML collection file
    pub collection: PathBuf,
}

/// Arguments for the `ui` subcommand.
#[derive(Args)]
pub struct UiArgs {
    /// TCP port for the UI server
    #[arg(long, default_value = "7878")]
    pub port: u16,
    /// Pre-select a collection file in the UI
    #[arg(long)]
    pub collection: Option<std::path::PathBuf>,
}

/// Output format for the `run` subcommand.
#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    /// Pretty human-readable output (default)
    Console,
    /// JSON object with passed/failed counts and per-request details
    Json,
    /// JUnit XML for CI/CD integration (Jenkins, GitHub Actions, etc.)
    Junit,
}

/// Arguments for the `perf` subcommand.
#[derive(Args)]
pub struct PerfArgs {
    /// Path to the YAML collection file
    pub collection: std::path::PathBuf,
    /// Number of virtual users — overrides `performance.vus` in the collection file
    #[arg(long)]
    pub vus: Option<usize>,
    /// Test duration in seconds — overrides `performance.duration_secs`
    #[arg(long)]
    pub duration: Option<u64>,
    /// Load profile — overrides `performance.load_profile`
    #[arg(long)]
    pub load_profile: Option<LoadProfileArg>,
    /// Starting VU count for `ramp_up` profile — overrides `performance.initial_vus`
    #[arg(long)]
    pub initial_vus: Option<usize>,
    /// Threshold expression `METRIC:CONDITION:VALUE` (repeatable).
    ///
    /// Metrics: avg_response_ms, p95_response_ms, p99_response_ms,
    ///          error_rate_pct, throughput_rps
    ///
    /// Conditions: lt, lte, gt, gte
    ///
    /// Example: --threshold p95_response_ms:lt:500
    #[arg(long = "threshold", value_name = "METRIC:CONDITION:VALUE")]
    pub thresholds: Vec<String>,
    /// Path to a CSV or JSON data file (rows assigned round-robin to VUs)
    #[arg(long)]
    pub data: Option<std::path::PathBuf>,
    /// Output format: console (default) or json
    #[arg(long, default_value = "console")]
    pub format: PerfOutputFormat,
    /// Write output to this file instead of stdout
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,
}

/// Load profile argument for the `perf` subcommand.
#[derive(Clone, ValueEnum)]
pub enum LoadProfileArg {
    /// Maintain a constant number of VUs for the full duration
    Fixed,
    /// Ramp from `initial_vus` up to `vus` over the first half of the duration
    RampUp,
}

/// Output format for the `perf` subcommand (JUnit not included — perf results
/// do not map to test cases).
#[derive(Clone, ValueEnum)]
pub enum PerfOutputFormat {
    /// Pretty human-readable metrics table (default)
    Console,
    /// JSON object containing metrics and threshold results
    Json,
}
