use std::collections::HashMap;

use crate::error::RequestError;

/// Captured HTTP response — available to assertions (Phase 6) and scripts (SP3).
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers with all names lowercased.
    ///
    /// Duplicate header names are joined with `, ` (reqwest default behaviour).
    pub headers: HashMap<String, String>,
    /// Response body as a UTF-8 string.
    pub body: String,
}

/// Internal resolved request constructed by the runner for consumption by `send`.
// Allow dead_code: consumed by runner.rs (Task 6) which is a stub at this stage.
#[allow(dead_code)]
pub(crate) struct ResolvedRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<ResolvedBody>,
    /// Per-request timeout in milliseconds. Default: 60 000.
    pub timeout_ms: u64,
}

/// Resolved body content ready to hand to reqwest.
// Allow dead_code: consumed by runner.rs (Task 6) which is a stub at this stage.
#[allow(dead_code)]
pub(crate) enum ResolvedBody {
    Json(serde_json::Value),
    Text(String),
    Form(HashMap<String, String>),
}

/// Send an HTTP request and return the captured response.
///
/// HTTP 4xx/5xx status codes are **not** errors — they are returned as `Ok(HttpResponse)`.
/// Only transport-level failures (DNS, TLS, timeout, etc.) return `Err`.
///
/// A new `reqwest::Client` is built per call (connection pooling deferred to SP5).
// Allow dead_code: called by runner.rs (Task 6) which is a stub at this stage.
#[allow(dead_code)]
pub(crate) async fn send(request: &ResolvedRequest) -> Result<HttpResponse, RequestError> {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    let url = &request.url;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(request.timeout_ms))
        .build()
        .map_err(|e| RequestError::Network {
            cause: e.to_string(),
        })?;

    let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|e| {
        RequestError::InvalidBody {
            cause: format!("invalid HTTP method '{}': {}", request.method, e),
        }
    })?;

    let mut req = client.request(method, url);

    // Apply user-provided headers
    let mut header_map = HeaderMap::new();
    for (name, value) in &request.headers {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            header_map.insert(n, v);
        }
    }
    req = req.headers(header_map);

    // Apply body (reqwest sets Content-Type automatically for json and form)
    req = match &request.body {
        None => req,
        Some(ResolvedBody::Json(v)) => req.json(v),
        Some(ResolvedBody::Text(s)) => req.body(s.clone()).header("content-type", "text/plain"),
        Some(ResolvedBody::Form(m)) => req.form(m),
    };

    let resp = req
        .send()
        .await
        .map_err(|e| map_reqwest_error(e, url, request.timeout_ms))?;

    let status = resp.status().as_u16();

    // Collect headers — lowercase names, join duplicate values with ", "
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in resp.headers() {
        let key = name.as_str().to_lowercase();
        let val = String::from_utf8_lossy(value.as_bytes()).into_owned();
        headers
            .entry(key)
            .and_modify(|existing| {
                existing.push_str(", ");
                existing.push_str(&val);
            })
            .or_insert(val);
    }

    let body = resp
        .text()
        .await
        .map_err(|e| RequestError::InvalidResponse {
            cause: e.to_string(),
        })?;

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

/// Extract the hostname from a URL for use in error messages.
// Allow dead_code: called only by map_reqwest_error, both removed when runner is wired up.
#[allow(dead_code)]
fn extract_domain(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.split(':').next())
        .unwrap_or(url)
        .to_string()
}

/// Map a reqwest error to the appropriate `RequestError` variant.
///
/// reqwest surfaces DNS, TLS, and connection-refused errors all as `is_connect()`.
/// We use string inspection of the error message as a best-effort heuristic.
// Allow dead_code: called by send(), which is unused until runner.rs (Task 6) is wired up.
#[allow(dead_code)]
fn map_reqwest_error(e: reqwest::Error, url: &str, timeout_ms: u64) -> RequestError {
    if e.is_timeout() {
        return RequestError::Timeout {
            url: url.to_string(),
            timeout_ms,
        };
    }
    if e.is_redirect() {
        return RequestError::TooManyRedirects {
            url: url.to_string(),
            max_redirects: 10,
        };
    }
    if e.is_connect() {
        let msg = e.to_string().to_lowercase();
        let domain = extract_domain(url);
        if msg.contains("dns") {
            return RequestError::DnsResolution {
                domain,
                cause: e.to_string(),
            };
        }
        if msg.contains("tls") || msg.contains("certificate") {
            return RequestError::TlsHandshake {
                domain,
                cause: e.to_string(),
            };
        }
        if msg.contains("connection refused") {
            return RequestError::ConnectionRefused {
                url: url.to_string(),
            };
        }
        return RequestError::Network {
            cause: e.to_string(),
        };
    }
    RequestError::Network {
        cause: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn make_get(url: &str) -> ResolvedRequest {
        ResolvedRequest {
            method: "GET".to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_ms: 200,
        }
    }

    #[tokio::test]
    async fn successful_get_returns_status_and_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .mount(&server)
            .await;

        let req = make_get(&format!("{}/ok", server.uri()));
        let resp = send(&req).await.expect("send should succeed");

        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "hello");
    }

    #[tokio::test]
    async fn response_headers_are_lowercased() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/headers"))
            .respond_with(ResponseTemplate::new(200).insert_header("X-Custom-Header", "value123"))
            .mount(&server)
            .await;

        let req = make_get(&format!("{}/headers", server.uri()));
        let resp = send(&req).await.unwrap();

        assert!(
            resp.headers.contains_key("x-custom-header"),
            "{:?}",
            resp.headers
        );
        assert!(!resp.headers.contains_key("X-Custom-Header"));
    }

    #[tokio::test]
    async fn http_404_is_not_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/notfound"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let req = make_get(&format!("{}/notfound", server.uri()));
        let resp = send(&req).await.expect("404 should be Ok, not Err");

        assert_eq!(resp.status, 404);
    }

    #[tokio::test]
    async fn timeout_produces_timeout_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(10)))
            .mount(&server)
            .await;

        let req = make_get(&format!("{}/slow", server.uri()));
        let err = send(&req).await.expect_err("should timeout");

        assert!(
            matches!(err, RequestError::Timeout { .. }),
            "expected Timeout, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn connection_refused_produces_error() {
        // Port 1 is never listening — guaranteed connection refused on all OSes.
        let req = make_get("http://127.0.0.1:1/test");
        let err = send(&req).await.expect_err("should fail");

        assert!(
            matches!(
                err,
                RequestError::ConnectionRefused { .. } | RequestError::Network { .. }
            ),
            "expected ConnectionRefused or Network, got: {err:?}"
        );
    }
}
