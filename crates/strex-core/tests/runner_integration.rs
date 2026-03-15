//! End-to-end tests for execute_collection using wiremock as the HTTP server.

use std::collections::HashMap;
use std::time::Duration;

use wiremock::matchers::{body_json, body_string, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use strex_core::{execute_collection, execute_collection_with_opts, RequestOutcome, RunnerOpts};
use strex_core::{Body, BodyType, Collection, ExecutionContext, Request, RequestError};

// ── helpers ──────────────────────────────────────────────────────────────────

fn collection(requests: Vec<Request>) -> Collection {
    Collection {
        name: "test".to_string(),
        version: "1.0".to_string(),
        environment: HashMap::new(),
        variables: HashMap::new(),
        requests,
    }
}

fn collection_with_vars(
    variables: HashMap<String, Option<String>>,
    requests: Vec<Request>,
) -> Collection {
    Collection {
        name: "test".to_string(),
        version: "1.0".to_string(),
        environment: HashMap::new(),
        variables,
        requests,
    }
}

fn get(url: &str, assertions: Vec<HashMap<String, serde_yaml::Value>>) -> Request {
    Request {
        name: "test-request".to_string(),
        method: "GET".to_string(),
        url: url.to_string(),
        headers: HashMap::new(),
        body: None,
        pre_script: None,
        post_script: None,
        assertions,
        timeout: Some(2000),
    }
}

fn status_assertion(code: u64) -> HashMap<String, serde_yaml::Value> {
    HashMap::from([("status".to_string(), serde_yaml::Value::Number(code.into()))])
}

fn jsonpath_equals(path: &str, expected: &str) -> HashMap<String, serde_yaml::Value> {
    HashMap::from([
        (
            "jsonPath".to_string(),
            serde_yaml::Value::String(path.to_string()),
        ),
        (
            "equals".to_string(),
            serde_yaml::Value::String(expected.to_string()),
        ),
    ])
}

fn header_contains(name: &str, substr: &str) -> HashMap<String, serde_yaml::Value> {
    HashMap::from([
        (
            "header".to_string(),
            serde_yaml::Value::String(name.to_string()),
        ),
        (
            "contains".to_string(),
            serde_yaml::Value::String(substr.to_string()),
        ),
    ])
}

// ── tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_request_status_assertion_passes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection(vec![get(
        &format!("{}/users/1", server.uri()),
        vec![status_assertion(200)],
    )]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn failed_status_assertion_captured() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/gone"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let col = collection(vec![get(
        &format!("{}/gone", server.uri()),
        vec![status_assertion(200)],
    )]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(!result.passed());
    assert_eq!(result.failure_count(), 1);
    assert!(matches!(
        result.request_results[0].outcome,
        RequestOutcome::AssertionsFailed(_)
    ));
}

#[tokio::test]
async fn post_request_with_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/items"))
        .and(body_json(serde_json::json!({"name": "widget"})))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let col = collection_with_vars(
        HashMap::from([("item_name".to_string(), Some("widget".to_string()))]),
        vec![Request {
            name: "create-item".to_string(),
            method: "POST".to_string(),
            url: format!("{}/items", server.uri()),
            headers: HashMap::new(),
            body: Some(Body {
                body_type: BodyType::Json,
                content: serde_yaml::from_str(r#"{"name": "{{item_name}}"}"#).unwrap(),
            }),
            pre_script: None,
            post_script: None,
            assertions: vec![status_assertion(201)],
            timeout: Some(2000),
        }],
    );
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn post_request_with_form_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection_with_vars(
        HashMap::from([("user_pass".to_string(), Some("secret".to_string()))]),
        vec![Request {
            name: "login".to_string(),
            method: "POST".to_string(),
            url: format!("{}/login", server.uri()),
            headers: HashMap::new(),
            body: Some(Body {
                body_type: BodyType::Form,
                content: serde_yaml::from_str("username: admin\npassword: \"{{user_pass}}\"")
                    .unwrap(),
            }),
            pre_script: None,
            post_script: None,
            assertions: vec![status_assertion(200)],
            timeout: Some(2000),
        }],
    );
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn post_request_with_text_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/echo"))
        .and(body_string("hello world"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection_with_vars(
        HashMap::from([("greeting".to_string(), Some("hello world".to_string()))]),
        vec![Request {
            name: "echo".to_string(),
            method: "POST".to_string(),
            url: format!("{}/echo", server.uri()),
            headers: HashMap::new(),
            body: Some(Body {
                body_type: BodyType::Text,
                content: serde_yaml::Value::String("{{greeting}}".to_string()),
            }),
            pre_script: None,
            post_script: None,
            assertions: vec![status_assertion(200)],
            timeout: Some(2000),
        }],
    );
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn jsonpath_equals_passes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1, "name": "Alice"})),
        )
        .mount(&server)
        .await;

    let col = collection(vec![get(
        &format!("{}/user", server.uri()),
        vec![
            status_assertion(200),
            jsonpath_equals("$.id", "1"),
            jsonpath_equals("$.name", "Alice"),
        ],
    )]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn jsonpath_not_found_fails() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&server)
        .await;

    let col = collection(vec![get(
        &format!("{}/user", server.uri()),
        vec![HashMap::from([
            (
                "jsonPath".to_string(),
                serde_yaml::Value::String("$.missing".to_string()),
            ),
            ("exists".to_string(), serde_yaml::Value::Bool(true)),
        ])],
    )]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(!result.passed());
    assert!(matches!(
        result.request_results[0].outcome,
        RequestOutcome::AssertionsFailed(_)
    ));
}

#[tokio::test]
async fn header_contains_passes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json; charset=utf-8"),
        )
        .mount(&server)
        .await;

    let col = collection(vec![get(
        &format!("{}/data", server.uri()),
        vec![header_contains("content-type", "application/json")],
    )]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn variable_interpolation_in_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/42"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection_with_vars(
        HashMap::from([
            ("base".to_string(), Some(server.uri())),
            ("user_id".to_string(), Some("42".to_string())),
        ]),
        vec![get(
            "{{base}}/users/{{user_id}}",
            vec![status_assertion(200)],
        )],
    );
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn network_error_captured_and_next_request_continues() {
    let server = MockServer::start().await;
    // Only mount on /second — /first has no listener → connection error on non-existent port
    Mock::given(method("GET"))
        .and(path("/second"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection(vec![
        // First request points at a port that refuses connections
        get("http://127.0.0.1:1/first", vec![]),
        // Second request points at the live mock server
        get(
            &format!("{}/second", server.uri()),
            vec![status_assertion(200)],
        ),
    ]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    // First request errored, second passed
    assert!(matches!(
        result.request_results[0].outcome,
        RequestOutcome::Error(_)
    ));
    assert!(matches!(
        result.request_results[1].outcome,
        RequestOutcome::Passed
    ));
    // Collection-level passed() is false because first request errored
    assert!(!result.passed());
}

#[tokio::test]
async fn multi_request_collection_all_pass() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/b"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let col = collection(vec![
        get(&format!("{}/a", server.uri()), vec![status_assertion(200)]),
        get(&format!("{}/b", server.uri()), vec![status_assertion(201)]),
    ]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed());
    assert_eq!(result.failure_count(), 0);
    assert_eq!(result.request_results.len(), 2);
}

#[tokio::test]
async fn request_timeout_captured() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(10)))
        .mount(&server)
        .await;

    let col = collection(vec![Request {
        name: "slow".to_string(),
        method: "GET".to_string(),
        url: format!("{}/slow", server.uri()),
        headers: HashMap::new(),
        body: None,
        pre_script: None,
        post_script: None,
        assertions: vec![],
        timeout: Some(100), // 100ms — much less than 10s delay
    }]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(matches!(
        result.request_results[0].outcome,
        RequestOutcome::Error(RequestError::Timeout { .. })
    ));
    assert!(result.request_results[0].response.is_none());
}

#[tokio::test]
async fn response_captured_on_assertion_failure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/item"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let col = collection(vec![get(
        &format!("{}/item", server.uri()),
        vec![status_assertion(200)],
    )]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    // Assertion failed, but response is still captured
    assert!(matches!(
        result.request_results[0].outcome,
        RequestOutcome::AssertionsFailed(_)
    ));
    let resp = result.request_results[0].response.as_ref().unwrap();
    assert_eq!(resp.status, 404);
    assert_eq!(resp.body, "not found");
}

// ── script integration tests ─────────────────────────────────────────────────

#[tokio::test]
async fn pre_script_sets_variable_used_in_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/99"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let base = server.uri();
    let col = collection_with_vars(
        HashMap::from([("base".to_string(), Some(base.clone()))]),
        vec![Request {
            name: "get-user".to_string(),
            method: "GET".to_string(),
            url: "{{base}}/users/{{dynamic_id}}".to_string(),
            headers: HashMap::new(),
            body: None,
            pre_script: Some(r#"variables.set("dynamic_id", "99");"#.to_string()),
            post_script: None,
            assertions: vec![status_assertion(200)],
            timeout: Some(2000),
        }],
    );
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result.request_results[0].outcome);
}

#[tokio::test]
async fn post_script_extracts_token_used_in_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": "secret-token"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/profile"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer secret-token",
        ))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection(vec![
        Request {
            name: "login".to_string(),
            method: "POST".to_string(),
            url: format!("{}/login", server.uri()),
            headers: HashMap::new(),
            body: None,
            pre_script: None,
            post_script: Some(
                r#"const d = response.json(); variables.set("token", d.token);"#.to_string(),
            ),
            assertions: vec![status_assertion(200)],
            timeout: Some(2000),
        },
        Request {
            name: "get-profile".to_string(),
            method: "GET".to_string(),
            url: format!("{}/profile", server.uri()),
            headers: HashMap::from([("authorization".to_string(), "Bearer {{token}}".to_string())]),
            body: None,
            pre_script: None,
            post_script: None,
            assertions: vec![status_assertion(200)],
            timeout: Some(2000),
        },
    ]);
    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result);
}

#[tokio::test]
async fn post_script_delete_removes_variable_from_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/first"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/second"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection_with_vars(
        HashMap::from([("key".to_string(), Some("value".to_string()))]),
        vec![
            Request {
                name: "first".to_string(),
                method: "GET".to_string(),
                url: format!("{}/first", server.uri()),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: Some(r#"variables.delete("key");"#.to_string()),
                assertions: vec![],
                timeout: Some(2000),
            },
            Request {
                name: "second".to_string(),
                method: "GET".to_string(),
                url: format!("{}/second", server.uri()),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: None,
                assertions: vec![],
                timeout: Some(2000),
            },
        ],
    );

    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(matches!(
        result.request_results[0].outcome,
        RequestOutcome::Passed
    ));
    assert!(matches!(
        result.request_results[1].outcome,
        RequestOutcome::Passed
    ));
}

#[tokio::test]
async fn pre_script_error_stops_request_next_continues() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/second"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection(vec![
        Request {
            name: "failing-pre-script".to_string(),
            method: "GET".to_string(),
            url: format!("{}/first", server.uri()),
            headers: HashMap::new(),
            body: None,
            pre_script: Some(r#"throw new Error("pre-script boom");"#.to_string()),
            post_script: None,
            assertions: vec![],
            timeout: Some(2000),
        },
        Request {
            name: "ok".to_string(),
            method: "GET".to_string(),
            url: format!("{}/second", server.uri()),
            headers: HashMap::new(),
            body: None,
            pre_script: None,
            post_script: None,
            assertions: vec![status_assertion(200)],
            timeout: Some(2000),
        },
    ]);

    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(
        matches!(result.request_results[0].outcome, RequestOutcome::Error(_)),
        "first should error"
    );
    assert!(result.request_results[0].response.is_none());
    assert!(
        matches!(result.request_results[1].outcome, RequestOutcome::Passed),
        "second should pass"
    );
}

#[tokio::test]
async fn post_script_runtime_error_stops_request_response_captured() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let col = collection(vec![Request {
        name: "failing-post-script".to_string(),
        method: "GET".to_string(),
        url: format!("{}/data", server.uri()),
        headers: HashMap::new(),
        body: None,
        pre_script: None,
        post_script: Some(r#"throw new Error("post-script boom");"#.to_string()),
        assertions: vec![],
        timeout: Some(2000),
    }]);

    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(matches!(
        result.request_results[0].outcome,
        RequestOutcome::Error(_)
    ));
    assert!(result.request_results[0].response.is_some());
    assert_eq!(
        result.request_results[0].response.as_ref().unwrap().status,
        200
    );
}

#[tokio::test]
async fn script_timeout_produces_timeout_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection(vec![Request {
        name: "timeout-script".to_string(),
        method: "GET".to_string(),
        url: format!("{}/ok", server.uri()),
        headers: HashMap::new(),
        body: None,
        pre_script: Some(r#"while(true) {}"#.to_string()),
        post_script: None,
        assertions: vec![],
        timeout: Some(2000),
    }]);

    let ctx = ExecutionContext::new(&col);
    let opts = RunnerOpts {
        script_timeout_ms: 200,
        isolate_script_variables: false,
        continue_on_script_error: false,
    };
    let result = execute_collection_with_opts(&col, ctx, opts).await;

    assert!(
        matches!(
            result.request_results[0].outcome,
            RequestOutcome::Error(strex_core::RequestError::Script(_))
        ),
        "got: {:?}",
        result.request_results[0].outcome
    );
}

#[tokio::test]
async fn continue_on_script_error_collects_assert_failure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"value": 1})))
        .mount(&server)
        .await;

    let col = collection(vec![Request {
        name: "assert-in-script".to_string(),
        method: "GET".to_string(),
        url: format!("{}/data", server.uri()),
        headers: HashMap::new(),
        body: None,
        pre_script: None,
        post_script: Some(
            r#"assert(response.json().value === 999, "value should be 999");"#.to_string(),
        ),
        assertions: vec![status_assertion(200)],
        timeout: Some(2000),
    }]);

    let ctx = ExecutionContext::new(&col);
    let opts = RunnerOpts {
        script_timeout_ms: 5_000,
        isolate_script_variables: false,
        continue_on_script_error: true,
    };
    let result = execute_collection_with_opts(&col, ctx, opts).await;

    assert!(
        matches!(
            result.request_results[0].outcome,
            RequestOutcome::AssertionsFailed(_)
        ),
        "got: {:?}",
        result.request_results[0].outcome
    );
}

#[tokio::test]
async fn isolate_script_variables_prevents_mutation_leaking() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/first"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/check/original"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection_with_vars(
        HashMap::from([("key".to_string(), Some("original".to_string()))]),
        vec![
            Request {
                name: "first".to_string(),
                method: "GET".to_string(),
                url: format!("{}/first", server.uri()),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: Some(r#"variables.set("key", "mutated");"#.to_string()),
                assertions: vec![status_assertion(200)],
                timeout: Some(2000),
            },
            Request {
                name: "second".to_string(),
                method: "GET".to_string(),
                url: format!("{}/check/{{{{key}}}}", server.uri()),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: None,
                assertions: vec![status_assertion(200)],
                timeout: Some(2000),
            },
        ],
    );

    let ctx = ExecutionContext::new(&col);
    let opts = RunnerOpts {
        script_timeout_ms: 5_000,
        isolate_script_variables: true,
        continue_on_script_error: false,
    };
    let result = execute_collection_with_opts(&col, ctx, opts).await;

    assert!(result.passed(), "{:?}", result);
}

#[tokio::test]
async fn post_script_variables_clear_removes_all_for_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/first"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/second"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let col = collection_with_vars(
        HashMap::from([
            ("key1".to_string(), Some("val1".to_string())),
            ("key2".to_string(), Some("val2".to_string())),
        ]),
        vec![
            Request {
                name: "first".to_string(),
                method: "GET".to_string(),
                url: format!("{}/first", server.uri()),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: Some(
                    r#"variables.clear(); variables.set("fresh", "yes");"#.to_string(),
                ),
                assertions: vec![status_assertion(200)],
                timeout: Some(2000),
            },
            Request {
                name: "second".to_string(),
                method: "GET".to_string(),
                url: format!("{}/second", server.uri()),
                headers: HashMap::new(),
                body: None,
                pre_script: None,
                post_script: Some(
                    r#"
                    assert(!variables.has("key1"), "key1 should be cleared");
                    assert(!variables.has("key2"), "key2 should be cleared");
                    assert(variables.has("fresh"), "fresh should survive clear");
                    "#
                    .to_string(),
                ),
                assertions: vec![status_assertion(200)],
                timeout: Some(2000),
            },
        ],
    );

    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result);
}

#[tokio::test]
async fn post_script_can_read_response_timing_total() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/timed"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let col = collection(vec![Request {
        name: "timed".to_string(),
        method: "GET".to_string(),
        url: format!("{}/timed", server.uri()),
        headers: HashMap::new(),
        body: None,
        pre_script: None,
        post_script: Some(
            r#"
            const t = response.timing.total;
            assert(typeof t === "number", "timing.total should be a number");
            "#
            .to_string(),
        ),
        assertions: vec![status_assertion(200)],
        timeout: Some(2000),
    }]);

    let ctx = ExecutionContext::new(&col);
    let result = execute_collection(&col, ctx).await;

    assert!(result.passed(), "{:?}", result);
}
