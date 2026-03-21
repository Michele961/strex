//! strex-ui — Axum web server and embedded Svelte frontend for `strex ui`.

#![deny(clippy::all)]

mod collections;
mod events;
mod history;
mod import;
mod perf_history;
mod request_list;
mod routes;
mod server;
mod ws;
mod ws_perf;

pub use server::{start_server, ServerOpts};
