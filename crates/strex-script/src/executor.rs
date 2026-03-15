use std::time::{Duration, Instant};

use rquickjs::{Context, Runtime, Value};

use crate::api;
use crate::context::{ScriptContext, ScriptOptions, ScriptResult};
use crate::error::ScriptError;

/// Execute a JavaScript script synchronously inside a QuickJS sandbox.
///
/// Must be called inside `tokio::task::spawn_blocking` — never on an async thread.
pub fn execute_script(
    script: &str,
    context: ScriptContext,
    opts: &ScriptOptions,
) -> Result<ScriptResult, ScriptError> {
    let rt = Runtime::new().map_err(|e| ScriptError::RuntimeInit {
        cause: e.to_string(),
    })?;
    rt.set_memory_limit(opts.memory_limit_bytes);

    // Layer 2 timeout: QuickJS interrupt handler (graceful stop).
    // Layer 1 (hard kill) is tokio::time::timeout in run_script() in the runner.
    let deadline = Instant::now() + Duration::from_millis(opts.timeout_ms);
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

    let ctx = Context::full(&rt).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("Allocation") || msg.contains("out of memory") || msg.contains("memory") {
            ScriptError::MemoryLimit {
                limit_mb: (opts.memory_limit_bytes / (1024 * 1024)) as u64,
            }
        } else {
            ScriptError::RuntimeInit { cause: msg }
        }
    })?;

    let mut result = ScriptResult::default();

    // Use `qctx` (Ctx<'_>) to avoid shadowing the outer `ctx` (Context).
    ctx.with(|qctx| -> Result<(), ScriptError> {
        let handles = api::inject(&qctx, &context).map_err(|e| {
            let msg = e.to_string();
            if msg.contains("Allocation") || msg.contains("out of memory") || msg.contains("memory")
            {
                ScriptError::MemoryLimit {
                    limit_mb: (opts.memory_limit_bytes / (1024 * 1024)) as u64,
                }
            } else {
                ScriptError::RuntimeInit { cause: msg }
            }
        })?;

        let eval_result = qctx.eval::<(), _>(script.as_bytes());

        match eval_result {
            Ok(()) => {
                api::drain(handles, &mut result);
                Ok(())
            }
            Err(rquickjs::Error::Exception) => {
                let exception = qctx.catch();
                Err(classify_exception(exception, opts, deadline))
            }
            Err(e) => Err(ScriptError::RuntimeInit {
                cause: e.to_string(),
            }),
        }
    })?;

    Ok(result)
}

/// Classify a caught JS exception into the appropriate [`ScriptError`] variant.
fn classify_exception<'js>(
    exception: Value<'js>,
    opts: &ScriptOptions,
    deadline: Instant,
) -> ScriptError {
    let (name, message, stack) = if let Some(obj) = exception.as_object() {
        let name: String = obj.get("name").unwrap_or_default();
        let message: String = obj.get("message").unwrap_or_default();
        let stack: Option<String> = obj.get("stack").ok();
        (name, message, stack)
    } else if let Some(s) = exception.as_string() {
        (String::new(), s.to_string().unwrap_or_default(), None)
    } else {
        (String::new(), "Unknown error".to_string(), None)
    };

    if name == "AssertionError" {
        return ScriptError::AssertionFailed { message };
    }
    if name == "InternalError" && (message.contains("interrupted") || Instant::now() >= deadline) {
        return ScriptError::Timeout {
            limit_ms: opts.timeout_ms,
        };
    }
    if message.contains("out of memory") {
        return ScriptError::MemoryLimit {
            limit_mb: (opts.memory_limit_bytes / (1024 * 1024)) as u64,
        };
    }
    if name == "SyntaxError" {
        // Compile-time SyntaxErrors from the top-level eval have a stack that shows
        // a bare `eval_script:` frame WITHOUT an enclosing `<eval>` function wrapper.
        // Runtime SyntaxErrors (e.g. from JSON.parse) have `<eval>` in the stack
        // because they occur inside a called function.
        let is_runtime = stack.as_deref().is_some_and(|s| s.contains("<eval>"));
        if !is_runtime {
            let line: u32 = exception
                .as_object()
                .and_then(|o| o.get::<_, u32>("lineNumber").ok())
                .unwrap_or(0);
            let column: u32 = exception
                .as_object()
                .and_then(|o| o.get::<_, u32>("columnNumber").ok())
                .unwrap_or(0);
            return ScriptError::Compilation {
                line,
                column,
                message,
            };
        }
    }
    ScriptError::Runtime { message, stack }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::context::{LogLevel, ScriptContext, ScriptOptions, ScriptResponse, ScriptTiming};
    use crate::error::ScriptError;

    fn default_response() -> ScriptResponse {
        ScriptResponse {
            status: 200,
            headers: HashMap::new(),
            body: r#"{"id": 1, "name": "Alice"}"#.to_string(),
            timing: ScriptTiming::default(),
        }
    }

    fn ctx_with_response(response: ScriptResponse) -> ScriptContext {
        ScriptContext {
            response: Some(response),
            variables: HashMap::from([
                ("token".to_string(), "abc123".to_string()),
                ("userId".to_string(), "42".to_string()),
            ]),
            environment: HashMap::from([("ENV".to_string(), "test".to_string())]),
            data: HashMap::new(),
        }
    }

    fn ctx_pre() -> ScriptContext {
        ScriptContext {
            response: None,
            variables: HashMap::from([("key".to_string(), "val".to_string())]),
            environment: HashMap::new(),
            data: HashMap::new(),
        }
    }

    fn opts() -> ScriptOptions {
        ScriptOptions {
            memory_limit_bytes: 64 * 1024 * 1024,
            timeout_ms: 5_000,
        }
    }

    // ── variables ───────────────────────────────────────────────────────────

    #[test]
    fn variables_set_string_roundtrip() {
        let ctx = ctx_pre();
        let result = execute_script(r#"variables.set("newKey", "newVal");"#, ctx, &opts()).unwrap();
        assert_eq!(
            result.variable_mutations.get("newKey").map(|s| s.as_str()),
            Some("newVal")
        );
    }

    #[test]
    fn variables_set_number_coerces_to_string() {
        let ctx = ctx_pre();
        let result = execute_script(r#"variables.set("n", 42);"#, ctx, &opts()).unwrap();
        assert_eq!(
            result.variable_mutations.get("n").map(|s| s.as_str()),
            Some("42")
        );
    }

    #[test]
    fn variables_set_bool_coerces_to_string() {
        let ctx = ctx_pre();
        let result = execute_script(r#"variables.set("b", true);"#, ctx, &opts()).unwrap();
        assert_eq!(
            result.variable_mutations.get("b").map(|s| s.as_str()),
            Some("true")
        );
    }

    #[test]
    fn variables_delete_appears_in_deletions() {
        let ctx = ctx_pre();
        let result = execute_script(r#"variables.delete("key");"#, ctx, &opts()).unwrap();
        assert!(result.variable_deletions.contains(&"key".to_string()));
    }

    #[test]
    fn variables_clear_sets_flag() {
        let ctx = ctx_pre();
        let result = execute_script(r#"variables.clear();"#, ctx, &opts()).unwrap();
        assert!(result.variables_cleared);
    }

    #[test]
    fn variables_get_returns_existing_value() {
        let ctx = ctx_pre();
        let result = execute_script(
            r#"variables.set("echo", variables.get("key"));"#,
            ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            result.variable_mutations.get("echo").map(|s| s.as_str()),
            Some("val")
        );
    }

    #[test]
    fn variables_has_returns_true_for_existing() {
        let ctx = ctx_pre();
        let result = execute_script(
            r#"variables.set("exists", variables.has("key") ? "yes" : "no");"#,
            ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            result.variable_mutations.get("exists").map(|s| s.as_str()),
            Some("yes")
        );
    }

    #[test]
    fn variables_keys_returns_all_keys() {
        let ctx = ctx_pre(); // has "key" = "val"
        let result = execute_script(
            r#"
            variables.set("newKey", "newVal");
            const ks = variables.keys();
            variables.set("hasKey", ks.includes("key") ? "yes" : "no");
            variables.set("hasNew", ks.includes("newKey") ? "yes" : "no");
            "#,
            ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            result.variable_mutations.get("hasKey").map(|s| s.as_str()),
            Some("yes")
        );
        assert_eq!(
            result.variable_mutations.get("hasNew").map(|s| s.as_str()),
            Some("yes")
        );
    }

    // ── assertions ──────────────────────────────────────────────────────────

    #[test]
    fn assert_true_succeeds() {
        let ctx = ctx_pre();
        let result = execute_script(r#"assert(true);"#, ctx, &opts());
        assert!(result.is_ok());
    }

    #[test]
    fn assert_false_returns_assertion_failed() {
        let ctx = ctx_pre();
        let err = execute_script(r#"assert(false, "it broke");"#, ctx, &opts()).unwrap_err();
        assert!(
            matches!(err, ScriptError::AssertionFailed { ref message } if message == "it broke"),
            "got: {err:?}"
        );
    }

    #[test]
    fn assert_equal_passes() {
        let ctx = ctx_pre();
        assert!(execute_script(r#"assertEqual(1 + 1, 2);"#, ctx, &opts()).is_ok());
    }

    #[test]
    fn assert_equal_fails() {
        let ctx = ctx_pre();
        let err = execute_script(r#"assertEqual(1, 2, "not equal");"#, ctx, &opts()).unwrap_err();
        assert!(matches!(err, ScriptError::AssertionFailed { .. }));
    }

    #[test]
    fn assert_not_equal_passes() {
        let ctx = ctx_pre();
        assert!(execute_script(r#"assertNotEqual(1, 2);"#, ctx, &opts()).is_ok());
    }

    #[test]
    fn assert_contains_passes() {
        let ctx = ctx_pre();
        assert!(execute_script(r#"assertContains("hello world", "world");"#, ctx, &opts()).is_ok());
    }

    #[test]
    fn assert_match_passes() {
        let ctx = ctx_pre();
        assert!(execute_script(r#"assertMatch("hello123", /\d+/);"#, ctx, &opts()).is_ok());
    }

    // ── response (post-script only) ─────────────────────────────────────────

    #[test]
    fn response_status_accessible() {
        let ctx = ctx_with_response(default_response());
        let result = execute_script(
            r#"variables.set("s", String(response.status));"#,
            ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            result.variable_mutations.get("s").map(|s| s.as_str()),
            Some("200")
        );
    }

    #[test]
    fn response_status_text_accessible() {
        let mut resp = default_response();
        resp.status = 404;
        let ctx = ctx_with_response(resp);
        let result =
            execute_script(r#"variables.set("st", response.statusText);"#, ctx, &opts()).unwrap();
        assert!(!result
            .variable_mutations
            .get("st")
            .unwrap_or(&String::new())
            .is_empty());
    }

    #[test]
    fn response_json_parses_valid_body() {
        let ctx = ctx_with_response(default_response());
        let result = execute_script(
            r#"
            const data = response.json();
            variables.set("name", data.name);
            "#,
            ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            result.variable_mutations.get("name").map(|s| s.as_str()),
            Some("Alice")
        );
    }

    #[test]
    fn response_json_invalid_body_throws_runtime_error() {
        let mut resp = default_response();
        resp.body = "not json".to_string();
        let ctx = ctx_with_response(resp);
        let err = execute_script(r#"response.json();"#, ctx, &opts()).unwrap_err();
        assert!(matches!(err, ScriptError::Runtime { .. }), "got: {err:?}");
    }

    #[test]
    fn response_text_returns_body_string() {
        let ctx = ctx_with_response(default_response());
        let result =
            execute_script(r#"variables.set("body", response.text());"#, ctx, &opts()).unwrap();
        assert!(result
            .variable_mutations
            .get("body")
            .unwrap()
            .contains("Alice"));
    }

    #[test]
    fn response_timing_total_is_number() {
        let mut resp = default_response();
        resp.timing.total_ms = 42;
        let ctx = ctx_with_response(resp);
        let result = execute_script(
            r#"variables.set("t", String(response.timing.total));"#,
            ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            result.variable_mutations.get("t").map(|s| s.as_str()),
            Some("42")
        );
    }

    #[test]
    fn response_headers_accessible() {
        let mut resp = default_response();
        resp.headers
            .insert("content-type".to_string(), "application/json".to_string());
        let ctx = ctx_with_response(resp);
        let result = execute_script(
            r#"variables.set("ct", response.headers["content-type"]);"#,
            ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            result.variable_mutations.get("ct").map(|s| s.as_str()),
            Some("application/json")
        );
    }

    #[test]
    fn env_get_returns_environment_value() {
        let ctx = ctx_with_response(default_response()); // has ENV="test"
        let result =
            execute_script(r#"variables.set("e", env.get("ENV"));"#, ctx, &opts()).unwrap();
        assert_eq!(
            result.variable_mutations.get("e").map(|s| s.as_str()),
            Some("test")
        );
    }

    #[test]
    fn pre_script_accessing_response_throws() {
        let ctx = ctx_pre(); // response: None
        let err = execute_script(r#"response.status;"#, ctx, &opts()).unwrap_err();
        assert!(matches!(err, ScriptError::Runtime { .. }), "got: {err:?}");
    }

    // ── console ─────────────────────────────────────────────────────────────

    #[test]
    fn console_log_captured() {
        let ctx = ctx_pre();
        let result = execute_script(r#"console.log("hello");"#, ctx, &opts()).unwrap();
        assert_eq!(result.console_logs.len(), 1);
        assert_eq!(result.console_logs[0].level, LogLevel::Log);
        assert_eq!(result.console_logs[0].message, "hello");
    }

    #[test]
    fn console_warn_and_error_captured_with_correct_level() {
        let ctx = ctx_pre();
        let result =
            execute_script(r#"console.warn("w"); console.error("e");"#, ctx, &opts()).unwrap();
        assert_eq!(result.console_logs[0].level, LogLevel::Warn);
        assert_eq!(result.console_logs[1].level, LogLevel::Error);
    }

    // ── error cases ─────────────────────────────────────────────────────────

    #[test]
    fn syntax_error_returns_compilation_error() {
        let ctx = ctx_pre();
        let err = execute_script(r#"if ({"#, ctx, &opts()).unwrap_err();
        assert!(
            matches!(err, ScriptError::Compilation { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn runtime_exception_returns_runtime_error() {
        let ctx = ctx_pre();
        let err = execute_script(r#"throw new Error("boom");"#, ctx, &opts()).unwrap_err();
        assert!(
            matches!(err, ScriptError::Runtime { ref message, .. } if message.contains("boom")),
            "got: {err:?}"
        );
    }

    #[test]
    fn memory_limit_exceeded_returns_memory_limit_error() {
        let ctx = ctx_pre();
        let tiny_opts = ScriptOptions {
            memory_limit_bytes: 1024, // 1 KB — tiny limit
            timeout_ms: 5_000,
        };
        let err = execute_script(
            r#"let a = []; while(true) { a.push(new Array(1000)); }"#,
            ctx,
            &tiny_opts,
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                ScriptError::MemoryLimit { .. } | ScriptError::Timeout { .. }
            ),
            "got: {err:?}"
        );
    }
}
