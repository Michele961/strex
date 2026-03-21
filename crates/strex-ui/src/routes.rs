//! HTTP route handlers for static files and the collections API.

use axum::{
    extract::{Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rust_embed::Embed;
use serde::Deserialize;

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

/// Query parameters for `/api/data-preview`.
#[derive(Deserialize)]
pub struct DataPreviewParams {
    /// Path to the data file (.csv or .json) to preview.
    pub file: String,
}

/// `GET /api/data-preview?file=<path>` — parse a data file and return up to 20 rows.
///
/// Returns `400` for unsupported file types or parse errors, `500` for I/O errors.
pub async fn data_preview(Query(params): Query<DataPreviewParams>) -> impl IntoResponse {
    let path = &params.file;

    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not read file '{path}': {e}"),
            )
                .into_response()
        }
    };

    let rows = if path.ends_with(".csv") {
        strex_core::parse_csv(&content).map_err(|e| format!("CSV parse error: {e}"))
    } else if path.ends_with(".json") {
        strex_core::parse_json(&content).map_err(|e| format!("JSON parse error: {e}"))
    } else {
        Err(format!(
            "Unsupported file extension (expected .csv or .json): {path}"
        ))
    };

    match rows {
        Ok(rows) => {
            let preview: Vec<_> = rows.into_iter().take(20).collect();
            Json(preview).into_response()
        }
        Err(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
    }
}

/// `POST /api/history` — persist a completed run and return its id.
///
/// Expects a JSON body with fields: `collection`, `passed`, `failed`, `skipped`, `run`.
pub async fn save_history(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let collection = body["collection"].as_str().unwrap_or("unknown");
    let passed = body["passed"].as_u64().unwrap_or(0) as usize;
    let failed = body["failed"].as_u64().unwrap_or(0) as usize;
    let skipped = body["skipped"].as_u64().unwrap_or(0) as usize;
    let run = &body["run"];

    match crate::history::save_run(collection, passed, failed, skipped, run) {
        Ok(id) => Json(serde_json::json!({ "id": id })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// `GET /api/history` — list all saved runs ordered newest-first.
pub async fn list_history() -> impl IntoResponse {
    Json(crate::history::list_runs())
}

/// `GET /api/history/:id` — return the full payload for a single run.
pub async fn get_history(Path(id): Path<String>) -> impl IntoResponse {
    match crate::history::load_run(&id) {
        Ok(v) => Json(v).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Run not found").into_response(),
    }
}

/// `POST /api/perf-history` — persist a completed performance run and return its id.
///
/// Typed body extraction via `Json<SavePerfRunRequest>` returns `400` on missing fields.
pub async fn save_perf_history(
    Json(body): Json<crate::perf_history::SavePerfRunRequest>,
) -> impl IntoResponse {
    match crate::perf_history::save_perf_run(&body) {
        Ok(id) => Json(serde_json::json!({ "id": id })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// `GET /api/perf-history` — list all saved performance runs ordered newest-first.
pub async fn list_perf_history() -> impl IntoResponse {
    Json(crate::perf_history::list_perf_runs())
}

/// `GET /api/perf-history/:id` — return the full payload for a single performance run.
pub async fn get_perf_history(Path(id): Path<String>) -> impl IntoResponse {
    match crate::perf_history::load_perf_run(&id) {
        Ok(v) => Json(v).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Perf run not found").into_response(),
    }
}
