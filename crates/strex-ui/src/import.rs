//! Route handlers for `POST /api/import/generate` and `POST /api/import/save`.

use std::fs::OpenOptions;
use std::io::Write;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::server::AppState;

// ── Request / response types ───────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportSource {
    Curl,
    Openapi,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportMode {
    Scaffold,
    WithTests,
}

impl From<ImportMode> for strex_import::ImportMode {
    fn from(m: ImportMode) -> Self {
        match m {
            ImportMode::Scaffold => strex_import::ImportMode::Scaffold,
            ImportMode::WithTests => strex_import::ImportMode::WithTests,
        }
    }
}

/// Request body for `POST /api/import/generate`.
#[derive(Deserialize)]
pub struct GenerateRequest {
    source: ImportSource,
    input: String,
    mode: ImportMode,
}

/// Request body for `POST /api/import/save`.
#[derive(Deserialize)]
pub struct SaveRequest {
    yaml: String,
    filename: String,
}

// ── Handlers ───────────────────────────────────────────────────────────────────

/// `POST /api/import/generate` — convert a curl command or OpenAPI spec to a Strex YAML string.
pub async fn generate(
    State(state): State<AppState>,
    Json(body): Json<GenerateRequest>,
) -> impl IntoResponse {
    let mode: strex_import::ImportMode = body.mode.into();

    let result = match body.source {
        ImportSource::Curl => strex_import::from_curl(&body.input, mode),
        ImportSource::Openapi => {
            // Detect URL vs file path
            let spec = if body.input.starts_with("http://") || body.input.starts_with("https://") {
                fetch_url(&state.http_client, &body.input).await
            } else {
                std::fs::read_to_string(&body.input)
                    .map_err(|e| strex_import::ImportError::OpenApiParse(e.to_string()))
            };
            spec.and_then(|s| strex_import::from_openapi(&s, mode))
        }
    };

    match result {
        Ok(yaml) => (StatusCode::OK, Json(serde_json::json!({ "yaml": yaml }))).into_response(),
        // FetchTimeout is a struct variant: { url: String }
        Err(strex_import::ImportError::FetchTimeout { .. }) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Request timed out fetching the spec URL" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn fetch_url(
    client: &reqwest::Client,
    url: &str,
) -> Result<String, strex_import::ImportError> {
    client
        .get(url)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                strex_import::ImportError::FetchTimeout {
                    url: url.to_string(),
                }
            } else {
                strex_import::ImportError::OpenApiParse(e.to_string())
            }
        })?
        .text()
        .await
        .map_err(|e| strex_import::ImportError::OpenApiParse(e.to_string()))
}

/// `POST /api/import/save` — write generated YAML to a file in the current working directory.
pub async fn save(Json(body): Json<SaveRequest>) -> impl IntoResponse {
    // Validate filename
    if !body.filename.ends_with(".yaml") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Filename must end in .yaml" })),
        )
            .into_response();
    }
    if body.filename.contains('/') || body.filename.contains('\\') || body.filename.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Filename must not contain path separators or .." })),
        )
            .into_response();
    }

    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Could not determine working directory: {e}") })),
            ).into_response()
        }
    };

    let path = cwd.join(&body.filename);

    // Atomically create — fails if file already exists (no TOCTOU race)
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": format!("File already exists: {}", body.filename) })),
        )
            .into_response(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Could not write file: {e}") })),
            )
                .into_response()
        }
    };

    if let Err(e) = file.write_all(body.yaml.as_bytes()) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Write error: {e}") })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "filename": body.filename })),
    )
        .into_response()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use crate::server::build_router;

    // Serialise tests that mutate the process-wide working directory.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    async fn post_json(
        router: axum::Router,
        path: &str,
        body: serde_json::Value,
    ) -> axum::response::Response {
        router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn generate_curl_returns_yaml() {
        let app = build_router().unwrap();
        let res = post_json(
            app,
            "/api/import/generate",
            serde_json::json!({
                "source": "curl",
                "input": "curl https://api.example.com/users",
                "mode": "scaffold"
            }),
        )
        .await;
        assert_eq!(res.status(), 200);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["yaml"].as_str().unwrap().contains("GET"));
    }

    #[tokio::test]
    async fn generate_invalid_curl_returns_400() {
        let app = build_router().unwrap();
        let res = post_json(
            app,
            "/api/import/generate",
            serde_json::json!({
                "source": "curl",
                "input": "wget https://example.com",
                "mode": "scaffold"
            }),
        )
        .await;
        assert_eq!(res.status(), 400);
    }

    #[tokio::test]
    async fn save_rejects_traversal() {
        let app = build_router().unwrap();
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "../evil.yaml" }),
        )
        .await;
        assert_eq!(res.status(), 400);
    }

    #[tokio::test]
    async fn save_rejects_non_yaml_extension() {
        let app = build_router().unwrap();
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "collection.json" }),
        )
        .await;
        assert_eq!(res.status(), 400);
    }

    #[tokio::test]
    async fn save_writes_file_and_returns_filename() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = CWD_LOCK.lock().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let app = build_router().unwrap();
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "my-import.yaml" }),
        )
        .await;

        std::env::set_current_dir(original).unwrap();

        assert_eq!(res.status(), 200);
        assert!(dir.path().join("my-import.yaml").exists());
    }

    #[tokio::test]
    async fn save_returns_409_if_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = CWD_LOCK.lock().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        std::fs::write(dir.path().join("dupe.yaml"), "existing").unwrap();

        let app = build_router().unwrap();
        let res = post_json(
            app,
            "/api/import/save",
            serde_json::json!({ "yaml": "name: test", "filename": "dupe.yaml" }),
        )
        .await;

        std::env::set_current_dir(original).unwrap();
        assert_eq!(res.status(), 409);
    }
}
