//! Handler for `GET /api/collection-requests` — returns request names and methods from a collection.

use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

/// Query parameters for `GET /api/collection-requests`.
#[derive(Deserialize)]
pub(crate) struct RequestListParams {
    /// Collection filename, relative to the current working directory.
    pub file: String,
}

/// A single entry in the collection request sequence.
#[derive(Debug, Serialize, PartialEq)]
pub struct CollectionRequestItem {
    /// Request name from the collection YAML.
    pub name: String,
    /// HTTP method (GET, POST, etc.).
    pub method: String,
}

/// `GET /api/collection-requests?file=<name>` — parse a collection YAML and return its request sequence.
///
/// Returns HTTP 400 if the path is absolute, contains `..`, or cannot be parsed.
/// Returns HTTP 200 with a JSON array of `{ name, method }` objects on success.
pub(crate) async fn list_collection_requests(Query(params): Query<RequestListParams>) -> Response {
    // Security: reject absolute paths and path traversal attempts.
    let p = Path::new(&params.file);
    if p.is_absolute() || p.components().any(|c| c == Component::ParentDir) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid file path" })),
        )
            .into_response();
    }

    let file_path = match std::env::current_dir() {
        Ok(cwd) => cwd.join(&params.file),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // parse_collection does blocking file I/O — run on a worker thread (ADR-0004).
    let result =
        tokio::task::spawn_blocking(move || strex_core::parse_collection(&file_path)).await;

    match result {
        Ok(Ok(collection)) => {
            let items: Vec<CollectionRequestItem> = collection
                .requests
                .into_iter()
                .map(|r| CollectionRequestItem {
                    name: r.name,
                    method: r.method,
                })
                .collect();
            (StatusCode::OK, Json(items)).into_response()
        }
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absolute_path() {
        let p = Path::new("/etc/passwd");
        assert!(p.is_absolute());
    }

    #[test]
    fn rejects_parent_dir_component() {
        let p = Path::new("../secrets.yaml");
        assert!(p.components().any(|c| c == Component::ParentDir));
    }

    #[test]
    fn accepts_simple_filename() {
        let p = Path::new("collection.yaml");
        assert!(!p.is_absolute());
        assert!(!p.components().any(|c| c == Component::ParentDir));
    }

    #[test]
    fn collection_request_item_serializes() {
        let item = CollectionRequestItem {
            name: "Get user".into(),
            method: "GET".into(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains(r#""name":"Get user""#));
        assert!(json.contains(r#""method":"GET""#));
    }
}
