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
}

/// Arguments for the `validate` subcommand.
#[derive(Args)]
pub struct ValidateArgs {
    /// Path to the YAML collection file
    pub collection: PathBuf,
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
