#![deny(clippy::all)]

mod assertions;
mod collection;
mod context;
mod error;
mod http;
mod interpolation;
mod parser;
mod runner;

pub use collection::{Body, BodyType, Collection, Request};
pub use context::ExecutionContext;
pub use error::{AssertionFailure, AssertionType, CollectionError, RequestError};
pub use http::HttpResponse;
pub use interpolation::interpolate;
pub use parser::parse_collection;
pub use runner::{execute_collection, CollectionResult, RequestOutcome, RequestResult};
