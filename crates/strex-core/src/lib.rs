#![deny(clippy::all)]

mod collection;
mod context;
mod error;
mod interpolation;
mod parser;

pub use collection::{Body, BodyType, Collection, Request};
pub use context::ExecutionContext;
pub use error::CollectionError;
pub use interpolation::interpolate;
pub use parser::parse_collection;
