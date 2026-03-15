#![deny(clippy::all)]

mod assertions;
mod collection;
mod context;
mod data;
mod error;
mod http;
mod interpolation;
mod parser;
mod runner;

pub use collection::{Body, BodyType, Collection, Request};
pub use context::ExecutionContext;
pub use error::{AssertionFailure, AssertionType, CollectionError, RequestError};
pub use http::{HttpResponse, RequestTiming};
pub use interpolation::interpolate;
pub use parser::parse_collection;
pub use runner::{
    execute_collection, execute_collection_with_opts, CollectionResult, RequestOutcome,
    RequestResult, RunnerOpts,
};
pub use strex_script::ScriptError;
