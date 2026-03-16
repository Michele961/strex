//! Axum server setup — router, bind, browser open.

use std::path::PathBuf;

use axum::{
    routing::{any, get},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::{routes, ws};

/// Options for starting the strex UI server.
pub struct ServerOpts {
    /// TCP port to bind on. Default: 7878.
    pub port: u16,
    /// Optional collection path to pre-select in the UI.
    pub collection: Option<PathBuf>,
}

/// Start the Axum server, print the URL, open the browser, and block until shutdown.
pub async fn start_server(opts: ServerOpts) -> anyhow::Result<()> {
    let _ = opts.collection;

    let app = Router::new()
        .route("/", get(routes::serve_index))
        .route(
            "/assets/*path",
            get(|axum::extract::Path(p): axum::extract::Path<String>| routes::serve_asset(p)),
        )
        .route("/api/collections", get(routes::list_collections))
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
