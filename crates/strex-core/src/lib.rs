#![deny(clippy::all)]

mod assertions;
mod collection;
mod context;
mod data;
mod error;
mod http;
mod interpolation;
mod parser;
pub mod perf;
mod runner;

pub use collection::{Body, BodyType, Collection, OnFailure, Request};
pub use context::ExecutionContext;
pub use data::{
    parse_csv, parse_json, run_collection_with_data, DataError, DataRow, DataRunOpts,
    DataRunResult, IterationResult,
};
pub use error::{AssertionFailure, AssertionType, CollectionError, RequestError};
pub use http::{HttpResponse, RequestTiming};
pub use interpolation::interpolate;
pub use parser::parse_collection;
pub use perf::{
    evaluate_thresholds, run_perf, LoadProfile, MetricKind, PerfError, PerfMetrics, PerfOpts,
    PerfResult, PerfTick, PerformanceConfig, RequestMetrics, RequestTick, Threshold,
    ThresholdCondition, ThresholdResult,
};
pub use runner::{
    execute_collection, execute_collection_streaming, execute_collection_with_opts,
    CollectionResult, RequestOutcome, RequestResult, RunnerOpts,
};
pub use strex_script::{ConsoleEntry, LogLevel, ScriptError};
