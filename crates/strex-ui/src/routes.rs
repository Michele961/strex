//! HTTP route handlers for static files and the collections API.

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rust_embed::Embed;

use crate::collections::scan_yaml_files;

/// Embedded static assets from the Svelte build output.
#[derive(Embed)]
#[folder = "frontend/dist/"]
struct Assets;

/// Serve the root `index.html`.
pub async fn serve_index() -> impl IntoResponse {
    serve_asset("index.html".to_string()).await
}

/// Serve any static asset by path (JS, CSS, fonts, etc.).
pub async fn serve_asset(path: String) -> impl IntoResponse {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            match Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(axum::body::Body::from(content.data))
            {
                Ok(r) => r,
                Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response(),
            }
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

/// `GET /api/collections` — list `.yaml` files in the current working directory.
pub async fn list_collections() -> impl IntoResponse {
    let cwd = std::env::current_dir().unwrap_or_default();
    let files = scan_yaml_files(&cwd);
    Json(files)
}
