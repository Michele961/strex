//! Axum server setup — router, bind, browser open.

use std::path::PathBuf;
use std::time::Duration;

use axum::{
    routing::{any, get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::{import, request_list, routes, ws, ws_perf};

/// Shared application state threaded through all route handlers.
#[derive(Clone)]
pub struct AppState {
    /// HTTP client with a 10-second timeout, shared across all import requests.
    pub http_client: reqwest::Client,
}

/// Build the Axum router with all routes and state attached.
///
/// Extracted so integration tests can call `build_router()` directly.
///
/// # Errors
/// Returns an error if the reqwest client cannot be built.
pub(crate) fn build_router() -> anyhow::Result<Router> {
    let state = AppState {
        http_client: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?,
    };

    Ok(Router::new()
        .route("/", get(routes::serve_index))
        .route(
            "/assets/*path",
            get(|axum::extract::Path(p): axum::extract::Path<String>| {
                routes::serve_asset(format!("assets/{p}"))
            }),
        )
        .route("/api/collections", get(routes::list_collections))
        .route(
            "/api/collection-requests",
            get(request_list::list_collection_requests),
        )
        .route("/api/data-preview", get(routes::data_preview))
        .route(
            "/api/history",
            post(routes::save_history).get(routes::list_history),
        )
        .route("/api/history/:id", get(routes::get_history))
        .route(
            "/api/perf-history",
            post(routes::save_perf_history).get(routes::list_perf_history),
        )
        .route("/api/perf-history/:id", get(routes::get_perf_history))
        .route("/api/import/generate", post(import::generate))
        .route("/api/import/save", post(import::save))
        .route("/ws", any(ws::ws_handler))
        .route("/ws/perf", any(ws_perf::ws_perf_handler))
        .layer(CorsLayer::permissive())
        .with_state(state))
}

/// Options for starting the strex UI server.
pub struct ServerOpts {
    /// TCP port to bind on. Default: 7878.
    pub port: u16,
    /// Optional collection path to pre-select in the UI.
    pub collection: Option<PathBuf>,
}

/// Start the Axum server, print the URL, open the browser, and block until shutdown.
///
/// # Errors
/// Returns an error if the router cannot be built, the port is unavailable, or the server fails.
pub async fn start_server(opts: ServerOpts) -> anyhow::Result<()> {
    let _ = opts.collection;
    let app = build_router()?;

    let addr = format!("127.0.0.1:{}", opts.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let url = format!("http://{}", addr);
    println!("strex UI running at {url}");
    println!("Press Ctrl+C to stop.");

    let _ = open::that(&url);

    axum::serve(listener, app).await?;
    Ok(())
}
