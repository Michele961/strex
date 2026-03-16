//! strex-ui — Axum web server and embedded Svelte frontend for `strex ui`.

#![deny(clippy::all)]

mod collections;
mod events;
mod routes;
mod server;
mod ws;

pub use server::{start_server, ServerOpts};
