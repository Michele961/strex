#![deny(clippy::all)]

mod collection;
mod error;

pub use collection::{Body, BodyType, Collection, Request};
pub use error::CollectionError;
