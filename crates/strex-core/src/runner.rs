use std::collections::HashMap;
use std::time::Instant;

use serde_yaml::Value as YamlValue;

use crate::assertions;
use crate::collection::{Body, BodyType, Collection};
use crate::context::ExecutionContext;
use crate::error::{AssertionFailure, RequestError};
use crate::http::{self, HttpResponse, ResolvedBody, ResolvedRequest};
use crate::interpolation::interpolate;

/// Aggregated result of running all requests in a collection.
#[derive(Debug)]
pub struct CollectionResult {
    /// Results for each request in the collection, in declaration order.
    pub request_results: Vec<RequestResult>,
}

impl CollectionResult {
    /// Returns `true` iff every request outcome is [`RequestOutcome::Passed`].
    pub fn passed(&self) -> bool {
        self.request_results
            .iter()
            .all(|r| matches!(r.outcome, RequestOutcome::Passed))
    }

    /// Count of requests whose outcome is not `Passed`.
    pub fn failure_count(&self) -> usize {
        self.request_results
            .iter()
            .filter(|r| !matches!(r.outcome, RequestOutcome::Passed))
            .count()
    }
}

/// Result of executing a single request through all applicable lifecycle phases.
#[derive(Debug)]
pub struct RequestResult {
    /// The request name from the collection YAML.
    pub name: String,
    /// Final outcome for this request.
    pub outcome: RequestOutcome,
    /// Full lifecycle duration (phase 1 start → phase 7 end), in milliseconds.
    pub duration_ms: u64,
    /// HTTP response captured in phase 4. `None` if a stopping error occurred before phase 3.
    pub response: Option<HttpResponse>,
}

/// Outcome of a single request execution.
#[derive(Debug)]
pub enum RequestOutcome {
    /// All assertions passed (or no assertions defined).
    Passed,
    /// One or more declarative assertions failed; all collected (execution continues).
    AssertionsFailed(Vec<AssertionFailure>),
    /// A stopping error occurred in phase 1 or 3; subsequent phases were skipped.
    Error(RequestError),
}

/// Run all requests in `collection` sequentially.
///
/// All per-request failures are captured in [`RequestOutcome`] — this function never fails.
pub async fn execute_collection(
    collection: &Collection,
    context: ExecutionContext,
) -> CollectionResult {
    let mut results = Vec::with_capacity(collection.requests.len());
    for request in &collection.requests {
        results.push(execute_request(request, &context).await);
    }
    CollectionResult {
        request_results: results,
    }
}

async fn execute_request(
    request: &crate::collection::Request,
    context: &ExecutionContext,
) -> RequestResult {
    let start = Instant::now();
    let name = request.name.clone();

    // Phase 1 — Template Interpolation
    let vars = context.resolve_all();
    let resolved = match resolve_request(request, &vars) {
        Ok(r) => r,
        Err(e) => {
            return RequestResult {
                name,
                outcome: RequestOutcome::Error(e),
                duration_ms: start.elapsed().as_millis() as u64,
                response: None,
            };
        }
    };

    // Phase 3 — HTTP Execution / Phase 4 — Response Capture
    let response = match http::send(&resolved).await {
        Ok(r) => r,
        Err(e) => {
            return RequestResult {
                name,
                outcome: RequestOutcome::Error(e),
                duration_ms: start.elapsed().as_millis() as u64,
                response: None,
            };
        }
    };

    // Phase 6 — Declarative Assertions
    let failures = match assertions::evaluate(&request.assertions, &response, &vars) {
        Ok(f) => f,
        Err(e) => {
            return RequestResult {
                name,
                outcome: RequestOutcome::Error(e),
                duration_ms: start.elapsed().as_millis() as u64,
                response: Some(response),
            };
        }
    };

    // Phase 7 — Result Recording
    let outcome = if failures.is_empty() {
        RequestOutcome::Passed
    } else {
        RequestOutcome::AssertionsFailed(failures)
    };

    RequestResult {
        name,
        outcome,
        duration_ms: start.elapsed().as_millis() as u64,
        response: Some(response),
    }
}

/// Interpolate all template fields in a request and produce a `ResolvedRequest`.
fn resolve_request(
    request: &crate::collection::Request,
    vars: &HashMap<String, String>,
) -> Result<ResolvedRequest, RequestError> {
    let url = interpolate(&request.url, vars).map_err(RequestError::Interpolation)?;

    let mut headers = HashMap::new();
    for (key, val) in &request.headers {
        let resolved = interpolate(val, vars).map_err(RequestError::Interpolation)?;
        headers.insert(key.clone(), resolved);
    }

    let body = request
        .body
        .as_ref()
        .map(|b| resolve_body(b, vars))
        .transpose()?;

    Ok(ResolvedRequest {
        method: request.method.clone(),
        url,
        headers,
        body,
        timeout_ms: request.timeout.unwrap_or(60_000),
    })
}

/// Resolve a request body — interpolate templates and convert to the wire format.
fn resolve_body(body: &Body, vars: &HashMap<String, String>) -> Result<ResolvedBody, RequestError> {
    match body.body_type {
        BodyType::Text => {
            let s = body
                .content
                .as_str()
                .ok_or_else(|| RequestError::InvalidBody {
                    cause: "Text body content must be a scalar string".to_string(),
                })?;
            let resolved = interpolate(s, vars).map_err(RequestError::Interpolation)?;
            Ok(ResolvedBody::Text(resolved))
        }

        BodyType::Form => {
            let mapping = body
                .content
                .as_mapping()
                .ok_or_else(|| RequestError::InvalidBody {
                    cause: "Form body content must be a YAML mapping".to_string(),
                })?;
            let mut form = HashMap::new();
            for (k, v) in mapping {
                let key = k.as_str().ok_or_else(|| RequestError::InvalidBody {
                    cause: "Form body keys must be strings".to_string(),
                })?;
                let raw = match v {
                    YamlValue::String(s) => s.clone(),
                    YamlValue::Number(n) => n.to_string(),
                    YamlValue::Bool(b) => b.to_string(),
                    YamlValue::Null => String::new(),
                    _ => {
                        return Err(RequestError::InvalidBody {
                            cause: format!("Form body values must be scalars (key: '{key}')"),
                        })
                    }
                };
                let resolved = interpolate(&raw, vars).map_err(RequestError::Interpolation)?;
                form.insert(key.to_string(), resolved);
            }
            Ok(ResolvedBody::Form(form))
        }

        BodyType::Json => {
            // Walk the YAML value tree, interpolate every String leaf, then convert to JSON.
            let resolved_yaml = interpolate_yaml_value(&body.content, vars)?;
            let json_value =
                serde_json::to_value(&resolved_yaml).map_err(|e| RequestError::InvalidBody {
                    cause: format!("Cannot convert body to JSON: {e}"),
                })?;
            Ok(ResolvedBody::Json(json_value))
        }
    }
}

/// Recursively walk a YAML value and interpolate every String leaf.
fn interpolate_yaml_value(
    value: &YamlValue,
    vars: &HashMap<String, String>,
) -> Result<YamlValue, RequestError> {
    match value {
        YamlValue::String(s) => {
            let resolved = interpolate(s, vars).map_err(RequestError::Interpolation)?;
            Ok(YamlValue::String(resolved))
        }
        YamlValue::Mapping(m) => {
            let mut result = serde_yaml::Mapping::new();
            for (k, v) in m {
                result.insert(k.clone(), interpolate_yaml_value(v, vars)?);
            }
            Ok(YamlValue::Mapping(result))
        }
        YamlValue::Sequence(s) => {
            let resolved: Result<Vec<YamlValue>, _> =
                s.iter().map(|v| interpolate_yaml_value(v, vars)).collect();
            Ok(YamlValue::Sequence(resolved?))
        }
        // Numbers, booleans, null — return as-is
        other => Ok(other.clone()),
    }
}
