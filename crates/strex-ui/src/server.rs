//! Axum server setup — router, bind, browser open.

use std::path::PathBuf;

use axum::{
    routing::{any, get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::{request_list, routes, ws};

/// Options for starting the strex UI server.
pub struct ServerOpts {
    /// TCP port to bind on. Default: 7878.
    pub port: u16,
    /// Optional collection path to pre-select in the UI.
    pub collection: Option<PathBuf>,
}

/// Start the Axum server, print the URL, open the browser, and block until shutdown.
pub async fn start_server(opts: ServerOpts) -> anyhow::Result<()> {
    // TODO: pass opts.collection to the frontend as a query param on the initial
    // page load so `strex ui --collection api.yaml` pre-selects that file.
    let _ = opts.collection;

    let app = Router::new()
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
        .route("/ws", any(ws::ws_handler))
        .layer(CorsLayer::permissive());

    let addr = format!("127.0.0.1:{}", opts.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let url = format!("http://{}", addr);
    println!("strex UI running at {url}");
    println!("Press Ctrl+C to stop.");

    // Open browser — ignore error (browser may not be available in CI)
    let _ = open::that(&url);

    axum::serve(listener, app).await?;
    Ok(())
}
