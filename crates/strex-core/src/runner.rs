use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant};

use serde_yaml::Value as YamlValue;

use crate::assertions;
use crate::collection::{Body, BodyType, Collection, OnFailure};
use crate::context::ExecutionContext;
use crate::error::{AssertionFailure, AssertionType, RequestError};
use crate::http::{self, ResolvedBody, ResolvedRequest};
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
            .all(|r| matches!(r.outcome, RequestOutcome::Passed | RequestOutcome::Skipped))
    }

    /// Count of requests whose outcome is `AssertionsFailed` or `Error`.
    pub fn failure_count(&self) -> usize {
        self.request_results
            .iter()
            .filter(|r| {
                matches!(
                    r.outcome,
                    RequestOutcome::AssertionsFailed(_) | RequestOutcome::Error(_)
                )
            })
            .count()
    }

    /// Count of requests whose outcome is [`RequestOutcome::Skipped`].
    pub fn skipped_count(&self) -> usize {
        self.request_results
            .iter()
            .filter(|r| matches!(r.outcome, RequestOutcome::Skipped))
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
    pub response: Option<crate::http::HttpResponse>,
    /// Console output emitted by pre- and post-request scripts, in emission order.
    pub logs: Vec<strex_script::ConsoleEntry>,
    /// Human-readable descriptions of declarative assertions that passed (e.g. "status 200").
    /// Empty when the request errored, was skipped, or had no assertions.
    pub passed_assertions: Vec<String>,
}

/// Outcome of a single request execution.
#[derive(Debug)]
pub enum RequestOutcome {
    /// All assertions passed (or no assertions defined).
    Passed,
    /// One or more declarative assertions failed; all collected (execution continues).
    AssertionsFailed(Vec<AssertionFailure>),
    /// A stopping error occurred in phase 1, 2, or 3; subsequent phases were skipped.
    Error(RequestError),
    /// This request was skipped because a prior request's `on_failure` action targeted it
    /// or all remaining requests were aborted via `on_failure: stop`.
    Skipped,
}

/// Runtime configuration for collection execution.
///
/// In SP3, these defaults are hardcoded. SP5 (CLI) will wire them to CLI flags.
#[derive(Debug, Clone)]
pub struct RunnerOpts {
    /// Per-script CPU time limit in milliseconds. Default: 30_000.
    pub script_timeout_ms: u64,
    /// When `true`, variable mutations from scripts do not persist to subsequent requests.
    /// Default: `false` (mutations persist — enables token chaining).
    pub isolate_script_variables: bool,
    /// When `true`, `assert()` failures in Phase 5 scripts are collected as non-stopping
    /// assertion failures (alongside declarative assertions).
    /// Default: `false` (`assert()` failures stop the request).
    pub continue_on_script_error: bool,
    /// Shared HTTP client used for all requests in this run.
    ///
    /// A single `reqwest::Client` maintains a connection pool, enabling TCP and TLS
    /// connection reuse across requests and concurrent iterations. Callers should
    /// create one client per collection run and pass it here. `RunnerOpts::default()`
    /// builds a fresh client automatically.
    pub http_client: std::sync::Arc<reqwest::Client>,
    /// Milliseconds to sleep before each request after the first one in a collection run.
    /// Default: `0` (no delay).
    pub delay_between_requests_ms: u64,
}

impl Default for RunnerOpts {
    fn default() -> Self {
        Self {
            script_timeout_ms: 30_000,
            isolate_script_variables: false,
            continue_on_script_error: false,
            http_client: std::sync::Arc::new(
                reqwest::Client::builder()
                    .build()
                    .expect("default reqwest::Client build should never fail"),
            ),
            delay_between_requests_ms: 0,
        }
    }
}

/// Run all requests in `collection` sequentially with default options.
///
/// All per-request failures are captured in [`RequestOutcome`] — this function never fails.
pub async fn execute_collection(
    collection: &Collection,
    context: ExecutionContext,
) -> CollectionResult {
    execute_collection_with_opts(collection, context, RunnerOpts::default()).await
}

/// Run all requests with explicit runtime options (used for testing and SP5 CLI).
///
/// All per-request failures are captured in [`RequestOutcome`] — this function never fails.
pub async fn execute_collection_with_opts(
    collection: &Collection,
    mut context: ExecutionContext,
    opts: RunnerOpts,
) -> CollectionResult {
    let name_to_index: HashMap<&str, usize> = collection
        .requests
        .iter()
        .enumerate()
        .map(|(i, r)| (r.name.as_str(), i))
        .collect();

    let mut results = Vec::with_capacity(collection.requests.len());
    let mut skip_until: Option<usize> = None;
    let mut i = 0;

    while i < collection.requests.len() {
        let request = &collection.requests[i];

        if let Some(resume_at) = skip_until {
            if i < resume_at {
                results.push(RequestResult {
                    name: request.name.clone(),
                    outcome: RequestOutcome::Skipped,
                    duration_ms: 0,
                    response: None,
                    logs: vec![],
                    passed_assertions: vec![],
                });
                i += 1;
                continue;
            }
            skip_until = None;
        }

        let result = execute_request(request, &mut context, &opts).await;

        if is_failure(&result.outcome) {
            if let Some(action) = &request.on_failure {
                match action {
                    OnFailure::Stop => {
                        results.push(result);
                        i += 1;
                        while i < collection.requests.len() {
                            results.push(RequestResult {
                                name: collection.requests[i].name.clone(),
                                outcome: RequestOutcome::Skipped,
                                duration_ms: 0,
                                response: None,
                                logs: vec![],
                                passed_assertions: vec![],
                            });
                            i += 1;
                        }
                        return CollectionResult { request_results: results };
                    }
                    OnFailure::SkipTo(target) => {
                        if let Some(&target_idx) = name_to_index.get(target.as_str()) {
                            skip_until = Some(target_idx);
                        }
                    }
                }
            }
        }

        results.push(result);
        i += 1;
    }

    CollectionResult { request_results: results }
}

/// Run all requests sequentially, invoking `on_result` after each one completes.
///
/// This allows callers (e.g. the WebSocket handler) to stream results incrementally
/// rather than waiting for the entire collection to finish. The callback receives a
/// reference to the completed [`RequestResult`] and may perform async work (e.g.
/// sending a WebSocket message).
///
/// All per-request failures are captured in [`RequestOutcome`] — this function never fails.
pub async fn execute_collection_streaming<F, Fut>(
    collection: &Collection,
    context: ExecutionContext,
    opts: RunnerOpts,
    mut on_result: F,
) -> CollectionResult
where
    F: FnMut(&RequestResult) -> Fut,
    Fut: Future<Output = ()>,
{
    let name_to_index: HashMap<&str, usize> = collection
        .requests
        .iter()
        .enumerate()
        .map(|(i, r)| (r.name.as_str(), i))
        .collect();

    let mut results = Vec::with_capacity(collection.requests.len());
    let mut context = context;
    let mut skip_until: Option<usize> = None;
    let mut i = 0;

    while i < collection.requests.len() {
        let request = &collection.requests[i];

        if let Some(resume_at) = skip_until {
            if i < resume_at {
                let result = RequestResult {
                    name: request.name.clone(),
                    outcome: RequestOutcome::Skipped,
                    duration_ms: 0,
                    response: None,
                    logs: vec![],
                    passed_assertions: vec![],
                };
                on_result(&result).await;
                results.push(result);
                i += 1;
                continue;
            }
            skip_until = None;
        }

        if i > 0 && opts.delay_between_requests_ms > 0 {
            tokio::time::sleep(Duration::from_millis(opts.delay_between_requests_ms)).await;
        }

        let result = execute_request(request, &mut context, &opts).await;
        on_result(&result).await;

        if is_failure(&result.outcome) {
            if let Some(action) = &request.on_failure {
                match action {
                    OnFailure::Stop => {
                        results.push(result);
                        i += 1;
                        while i < collection.requests.len() {
                            let skipped = RequestResult {
                                name: collection.requests[i].name.clone(),
                                outcome: RequestOutcome::Skipped,
                                duration_ms: 0,
                                response: None,
                                logs: vec![],
                                passed_assertions: vec![],
                            };
                            on_result(&skipped).await;
                            results.push(skipped);
                            i += 1;
                        }
                        return CollectionResult { request_results: results };
                    }
                    OnFailure::SkipTo(target) => {
                        if let Some(&target_idx) = name_to_index.get(target.as_str()) {
                            skip_until = Some(target_idx);
                        }
                    }
                }
            }
        }

        results.push(result);
        i += 1;
    }

    CollectionResult { request_results: results }
}

fn is_failure(outcome: &RequestOutcome) -> bool {
    matches!(
        outcome,
        RequestOutcome::AssertionsFailed(_) | RequestOutcome::Error(_)
    )
}

async fn execute_request(
    request: &crate::collection::Request,
    context: &mut ExecutionContext,
    opts: &RunnerOpts,
) -> RequestResult {
    let start = Instant::now();
    let name = request.name.clone();
    let mut logs: Vec<strex_script::ConsoleEntry> = Vec::new();

    // Phase 1 — Snapshot current variables for use by pre-script.
    let mut vars = context.resolve_all();

    // Phase 2 — Pre-Request Script
    // Runs before template resolution so mutations affect URL/headers.
    if let Some(script) = &request.pre_script {
        let script_ctx = strex_script::ScriptContext {
            response: None,
            variables: vars.clone(),
            environment: context.environment.clone(),
            data: context.data.clone(),
        };
        match run_script(script, script_ctx, opts).await {
            Ok(result) => {
                logs.extend(result.console_logs.iter().cloned());
                apply_mutations(context, result, opts.isolate_script_variables);
                vars = context.resolve_all(); // re-merge after mutations
            }
            Err(e) => {
                return RequestResult {
                    name,
                    outcome: RequestOutcome::Error(RequestError::Script(e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    response: None,
                    logs,
                    passed_assertions: vec![],
                };
            }
        }
    }

    // Phase 1 (continued) — Template Interpolation using post-Phase-2 vars
    let resolved = match resolve_request(request, &vars) {
        Ok(r) => r,
        Err(e) => {
            return RequestResult {
                name,
                outcome: RequestOutcome::Error(e),
                duration_ms: start.elapsed().as_millis() as u64,
                response: None,
                logs,
                passed_assertions: vec![],
            };
        }
    };

    // Phase 3 — HTTP Execution / Phase 4 — Response Capture
    let response = match http::send(&opts.http_client, &resolved).await {
        Ok(r) => r,
        Err(e) => {
            return RequestResult {
                name,
                outcome: RequestOutcome::Error(e),
                duration_ms: start.elapsed().as_millis() as u64,
                response: None,
                logs,
                passed_assertions: vec![],
            };
        }
    };

    // Phase 5 — Post-Request Script
    let mut script_assertion_failures: Vec<AssertionFailure> = Vec::new();
    if let Some(script) = &request.post_script {
        let script_response = strex_script::ScriptResponse {
            status: response.status,
            headers: response.headers.clone(),
            body: response.body.clone(),
            timing: strex_script::ScriptTiming {
                dns_ms: response.timing.dns_ms,
                connect_ms: response.timing.connect_ms,
                tls_ms: response.timing.tls_ms,
                send_ms: response.timing.send_ms,
                wait_ms: response.timing.wait_ms,
                receive_ms: response.timing.receive_ms,
                total_ms: response.timing.total_ms,
            },
        };
        let script_ctx = strex_script::ScriptContext {
            response: Some(script_response),
            variables: vars.clone(),
            environment: context.environment.clone(),
            data: context.data.clone(),
        };
        match run_script(script, script_ctx, opts).await {
            Ok(result) => {
                logs.extend(result.console_logs.iter().cloned());
                apply_mutations(context, result, opts.isolate_script_variables);
            }
            Err(strex_script::ScriptError::AssertionFailed { message })
                if opts.continue_on_script_error =>
            {
                script_assertion_failures.push(AssertionFailure {
                    assertion_type: AssertionType::Script,
                    expected: message,
                    actual: String::new(),
                    path: None,
                });
            }
            Err(e) => {
                // MVP behaviour: a non-assertion post-script error stops the request and
                // skips Phase 6 declarative assertions. ADR-0002 says to "continue to
                // assertions", but implementing that requires merging two failure sources.
                // Tracked for SP5 refinement.
                return RequestResult {
                    name,
                    outcome: RequestOutcome::Error(RequestError::Script(e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    response: Some(response),
                    logs,
                    passed_assertions: vec![],
                };
            }
        }
    }

    // Phase 6 — Declarative Assertions
    let (passed_assertions, mut failures) =
        match assertions::evaluate(&request.assertions, &response, &vars) {
            Ok((p, f)) => (p, f),
            Err(e) => {
                return RequestResult {
                    name,
                    outcome: RequestOutcome::Error(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    response: Some(response),
                    logs,
                    passed_assertions: vec![],
                };
            }
        };

    failures.extend(script_assertion_failures);

    // Phase 7 — Result Recording
    let outcome = if failures.is_empty() {
        RequestOutcome::Passed
    } else {
        RequestOutcome::AssertionsFailed(failures)
    };

    // Capture elapsed once — used for both RequestResult.duration_ms and
    // response.timing.total_ms. Both represent the full lifecycle duration
    // (phase 1 start → phase 7 end), not the HTTP-only round-trip time.
    let duration_ms = start.elapsed().as_millis() as u64;
    let mut response = response;
    response.timing.total_ms = duration_ms;

    RequestResult {
        name,
        outcome,
        duration_ms,
        response: Some(response),
        logs,
        passed_assertions,
    }
}

/// Run a script on a blocking worker thread with a Tokio timeout (dual-layer).
///
/// The QuickJS interrupt handler provides a graceful layer-2 timeout; this function
/// adds a hard layer-1 timeout via `tokio::time::timeout`.
async fn run_script(
    script: &str,
    ctx: strex_script::ScriptContext,
    opts: &RunnerOpts,
) -> Result<strex_script::ScriptResult, strex_script::ScriptError> {
    let script = script.to_string();
    let script_opts = strex_script::ScriptOptions {
        memory_limit_bytes: 64 * 1024 * 1024, // 64 MB
        timeout_ms: opts.script_timeout_ms,
    };
    let handle = tokio::task::spawn_blocking(move || {
        strex_script::execute_script(&script, ctx, &script_opts)
    });
    tokio::time::timeout(Duration::from_millis(opts.script_timeout_ms), handle)
        .await
        .map_err(|_| strex_script::ScriptError::Timeout {
            limit_ms: opts.script_timeout_ms,
        })?
        .map_err(|join_err| strex_script::ScriptError::ThreadPanic {
            cause: join_err.to_string(),
        })?
}

/// Write script variable mutations back to `ExecutionContext.variables`.
///
/// If `isolate` is true, mutations are discarded (do not affect subsequent requests).
fn apply_mutations(
    context: &mut ExecutionContext,
    result: strex_script::ScriptResult,
    isolate: bool,
) {
    if isolate {
        return;
    }
    if result.variables_cleared {
        context.variables.clear();
    }
    for key in result.variable_deletions {
        context.variables.remove(&key);
    }
    context.variables.extend(result.variable_mutations);
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
        other => Ok(other.clone()),
    }
}
