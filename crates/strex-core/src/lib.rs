#![deny(clippy::all)]

mod collection;
mod error;
mod interpolation;
mod parser;

pub use collection::{Body, BodyType, Collection, Request};
pub use error::CollectionError;
pub use parser::parse_collection;
