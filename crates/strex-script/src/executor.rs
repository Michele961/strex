use crate::context::{ScriptContext, ScriptOptions, ScriptResult};
use crate::error::ScriptError;

/// Execute a JavaScript script synchronously.
///
/// Must be called inside `tokio::task::spawn_blocking` — never on an async thread.
pub fn execute_script(
    _script: &str,
    _context: ScriptContext,
    _opts: &ScriptOptions,
) -> Result<ScriptResult, ScriptError> {
    todo!("Implemented in Task 4")
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
