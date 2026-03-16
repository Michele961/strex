#![deny(clippy::all)]

mod collections;
mod events;
mod routes;
mod server;
mod ws;

pub use server::{ServerOpts, start_server};
