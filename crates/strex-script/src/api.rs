use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rquickjs::{function::Rest, Ctx, Function, Object, Value};

use crate::context::{ConsoleEntry, LogLevel, ScriptContext, ScriptResult};

/// Shared state mutated by the injected JavaScript `variables` global.
pub(crate) struct DrainHandles {
    /// Key/value pairs written by `variables.set()`.
    pub mutations: Arc<Mutex<HashMap<String, String>>>,
    /// Keys passed to `variables.delete()`.
    pub deletions: Arc<Mutex<Vec<String>>>,
    /// Set to `true` when `variables.clear()` is called.
    pub cleared: Arc<Mutex<bool>>,
    /// Entries appended by `console.log/warn/error()`.
    pub logs: Arc<Mutex<Vec<ConsoleEntry>>>,
}

/// Drain all accumulated JS-side state into a [`ScriptResult`].
pub(crate) fn drain(handles: DrainHandles, result: &mut ScriptResult) {
    result.variable_mutations = handles.mutations.lock().unwrap().drain().collect();
    result.variable_deletions = handles.deletions.lock().unwrap().drain(..).collect();
    result.variables_cleared = *handles.cleared.lock().unwrap();
    result.console_logs = handles.logs.lock().unwrap().drain(..).collect();
}

/// Inject all Strex globals into the QuickJS context.
///
/// Returns [`DrainHandles`] so the caller can read back accumulated state
/// after the script finishes.
pub(crate) fn inject<'js>(
    ctx: &Ctx<'js>,
    context: &ScriptContext,
) -> rquickjs::Result<DrainHandles> {
    let mutations: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let deletions: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let cleared: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let logs: Arc<Mutex<Vec<ConsoleEntry>>> = Arc::new(Mutex::new(Vec::new()));

    let globals = ctx.globals();

    // ── variables ─────────────────────────────────────────────────────────────
    {
        let initial_vars: HashMap<String, String> = context.variables.clone();
        let vars_obj = Object::new(ctx.clone())?;

        // variables.set(key, value)
        {
            let mutations_c = Arc::clone(&mutations);
            let f = Function::new(ctx.clone(), move |key: String, val: Value<'_>| {
                let s = value_to_string(&val);
                mutations_c.lock().unwrap().insert(key, s);
                rquickjs::Result::Ok(())
            })?;
            vars_obj.set("set", f)?;
        }

        // variables.get(key)
        {
            let mutations_c = Arc::clone(&mutations);
            let initial_c = initial_vars.clone();
            let f = Function::new(ctx.clone(), move |key: String| {
                let guard = mutations_c.lock().unwrap();
                let val = guard
                    .get(&key)
                    .or_else(|| initial_c.get(&key))
                    .cloned()
                    .unwrap_or_default();
                rquickjs::Result::Ok(val)
            })?;
            vars_obj.set("get", f)?;
        }

        // variables.has(key)
        {
            let mutations_c = Arc::clone(&mutations);
            let initial_c = initial_vars.clone();
            let f = Function::new(ctx.clone(), move |key: String| {
                let guard = mutations_c.lock().unwrap();
                let found = guard.contains_key(&key) || initial_c.contains_key(&key);
                rquickjs::Result::Ok(found)
            })?;
            vars_obj.set("has", f)?;
        }

        // variables.delete(key)
        {
            let deletions_c = Arc::clone(&deletions);
            let f = Function::new(ctx.clone(), move |key: String| {
                deletions_c.lock().unwrap().push(key);
                rquickjs::Result::Ok(())
            })?;
            vars_obj.set("delete", f)?;
        }

        // variables.clear()
        {
            let cleared_c = Arc::clone(&cleared);
            let f = Function::new(ctx.clone(), move || {
                *cleared_c.lock().unwrap() = true;
                rquickjs::Result::Ok(())
            })?;
            vars_obj.set("clear", f)?;
        }

        // variables.keys() — union of initial keys and mutation keys
        {
            let mutations_c = Arc::clone(&mutations);
            let initial_c = initial_vars.clone();
            let f = Function::new(ctx.clone(), move || {
                let guard = mutations_c.lock().unwrap();
                let mut keys: Vec<String> = initial_c.keys().cloned().collect();
                for k in guard.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
                rquickjs::Result::Ok(keys)
            })?;
            vars_obj.set("keys", f)?;
        }

        globals.set("variables", vars_obj)?;
    }

    // ── env ───────────────────────────────────────────────────────────────────
    {
        let env_map: HashMap<String, String> = context.environment.clone();
        inject_readonly_map(ctx, &globals, "env", env_map)?;
    }

    // ── data ──────────────────────────────────────────────────────────────────
    {
        let data_map: HashMap<String, String> = context.data.clone();
        inject_readonly_map(ctx, &globals, "data", data_map)?;
    }

    // ── response ──────────────────────────────────────────────────────────────
    if let Some(resp) = &context.response {
        let resp_obj = Object::new(ctx.clone())?;

        resp_obj.set("status", resp.status as i32)?;
        resp_obj.set("statusText", status_text(resp.status))?;
        resp_obj.set("body", resp.body.clone())?;

        // headers — plain JS object
        let headers_obj = Object::new(ctx.clone())?;
        for (k, v) in &resp.headers {
            headers_obj.set(k.as_str(), v.as_str())?;
        }
        resp_obj.set("headers", headers_obj)?;

        // response.text()
        {
            let body_c = resp.body.clone();
            let f = Function::new(ctx.clone(), move || rquickjs::Result::Ok(body_c.clone()))?;
            resp_obj.set("text", f)?;
        }

        // response.timing
        let timing_obj = Object::new(ctx.clone())?;
        timing_obj.set("dns", resp.timing.dns_ms as i64)?;
        timing_obj.set("connect", resp.timing.connect_ms as i64)?;
        timing_obj.set("tls", resp.timing.tls_ms as i64)?;
        timing_obj.set("send", resp.timing.send_ms as i64)?;
        timing_obj.set("wait", resp.timing.wait_ms as i64)?;
        timing_obj.set("receive", resp.timing.receive_ms as i64)?;
        timing_obj.set("total", resp.timing.total_ms as i64)?;
        resp_obj.set("timing", timing_obj)?;

        globals.set("response", resp_obj)?;

        // Define response.json() via JS eval so JSON.parse has access to the body
        // without any Rust/JS Value<'js> lifetime issues in Rust closures.
        ctx.eval::<(), _>(br"response.json = function() { return JSON.parse(response.body); };")?;
    }

    // ── assert functions ──────────────────────────────────────────────────────
    // Injected entirely as JS code to avoid Value<'js> lifetime issues in closures
    // that accept multiple JS-typed parameters.
    ctx.eval::<(), _>(
        br#"
(function() {
    function _makeAssertionError(msg) {
        var e = { name: "AssertionError", message: msg };
        return e;
    }
    function assert(cond, msg) {
        if (!cond) { throw _makeAssertionError(msg || "Assertion failed"); }
    }
    function assertEqual(a, b, msg) {
        if (a !== b) { throw _makeAssertionError(msg || ("Expected " + String(a) + " to equal " + String(b))); }
    }
    function assertNotEqual(a, b, msg) {
        if (a === b) { throw _makeAssertionError(msg || ("Expected " + String(a) + " to not equal " + String(b))); }
    }
    function assertContains(haystack, needle, msg) {
        if (!haystack.includes(needle)) {
            throw _makeAssertionError(msg || ("Expected " + JSON.stringify(haystack) + " to contain " + JSON.stringify(needle)));
        }
    }
    function assertMatch(text, regex, msg) {
        if (!regex.test(text)) {
            throw _makeAssertionError(msg || ("Expected " + JSON.stringify(text) + " to match regex"));
        }
    }
    globalThis.assert = assert;
    globalThis.assertEqual = assertEqual;
    globalThis.assertNotEqual = assertNotEqual;
    globalThis.assertContains = assertContains;
    globalThis.assertMatch = assertMatch;
})();
"#,
    )?;

    // ── console ───────────────────────────────────────────────────────────────
    {
        let console_obj = Object::new(ctx.clone())?;

        inject_console_fn(ctx, &console_obj, &logs, "log", LogLevel::Log)?;
        inject_console_fn(ctx, &console_obj, &logs, "warn", LogLevel::Warn)?;
        inject_console_fn(ctx, &console_obj, &logs, "error", LogLevel::Error)?;

        globals.set("console", console_obj)?;
    }

    Ok(DrainHandles {
        mutations,
        deletions,
        cleared,
        logs,
    })
}

/// Inject a read-only map (env / data) as a JS object with `.get()`, `.has()`, `.keys()`.
fn inject_readonly_map<'js>(
    ctx: &Ctx<'js>,
    globals: &Object<'js>,
    name: &'static str,
    map: HashMap<String, String>,
) -> rquickjs::Result<()> {
    let obj = Object::new(ctx.clone())?;

    {
        let map_c = map.clone();
        let f = Function::new(ctx.clone(), move |key: String| {
            rquickjs::Result::Ok(map_c.get(&key).cloned().unwrap_or_default())
        })?;
        obj.set("get", f)?;
    }

    {
        let map_c = map.clone();
        let f = Function::new(ctx.clone(), move |key: String| {
            rquickjs::Result::Ok(map_c.contains_key(&key))
        })?;
        obj.set("has", f)?;
    }

    {
        let map_c = map.clone();
        let f = Function::new(ctx.clone(), move || {
            rquickjs::Result::Ok(map_c.keys().cloned().collect::<Vec<_>>())
        })?;
        obj.set("keys", f)?;
    }

    globals.set(name, obj)?;
    Ok(())
}

/// Inject a single `console.log/warn/error` function.
fn inject_console_fn<'js>(
    ctx: &Ctx<'js>,
    console_obj: &Object<'js>,
    logs: &Arc<Mutex<Vec<ConsoleEntry>>>,
    method: &'static str,
    level: LogLevel,
) -> rquickjs::Result<()> {
    let logs_c = Arc::clone(logs);
    let f = Function::new(ctx.clone(), move |args: Rest<Value<'_>>| {
        let message = args
            .0
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join(" ");
        logs_c.lock().unwrap().push(ConsoleEntry { level, message });
        rquickjs::Result::Ok(())
    })?;
    console_obj.set(method, f)?;
    Ok(())
}

/// Convert a JS [`Value`] to a Rust [`String`] using JavaScript's `String()` coercion rules.
pub(crate) fn value_to_string(val: &Value<'_>) -> String {
    if let Some(s) = val.as_string() {
        return s.to_string().unwrap_or_default();
    }
    if let Some(n) = val.as_int() {
        return n.to_string();
    }
    if let Some(n) = val.as_float() {
        return n.to_string();
    }
    if let Some(b) = val.as_bool() {
        return b.to_string();
    }
    if val.is_null() || val.is_undefined() {
        return String::new();
    }
    "[object Object]".to_string()
}

/// Return the standard HTTP reason phrase for a status code.
pub(crate) fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}
