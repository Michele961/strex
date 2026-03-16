# strex UI Results Detail Design

**Goal:** Enrich the `strex ui` results view with response body/header inspection, run summary stats, filter tabs, and a pre-run request sequence list — closing the gap with Postman's Collection Runner.

**Date:** 2026-03-16

---

## Problem

The current UI shows method, name, status, duration, and pass/fail per request. Three things Postman provides that are missing:

1. **No response inspection** — users cannot see what the server actually returned.
2. **No run summary stats** — total duration and average response time are not shown.
3. **No request sequence preview** — users cannot see which requests will run before clicking Run.

---

## Out of Scope

- Selective request execution (checkboxes that skip requests) — requires core runner changes.
- Request body / sent headers inspection — only response is in scope.
- Performance tab — deferred.
- Pagination or search within the results list.

---

## Architecture

Three layers change together:

1. **Backend** — extend `WsEvent`, add `request_list.rs` endpoint.
2. **Frontend data layer** — update TypeScript types, add one API function.
3. **Frontend UI** — update three existing components.

---

## Backend

### `crates/strex-ui/src/events.rs`

**`WsEvent::RequestCompleted`** gains two new optional fields:

```rust
/// Response body, truncated to BODY_LIMIT bytes. None on network error.
response_body: Option<String>,
/// Response headers (lowercase names). None on network error.
response_headers: Option<HashMap<String, String>>,
```

`serde` must derive `Serialize` for `HashMap` — add `use std::collections::HashMap` if not already imported.

**`WsEvent::RunFinished`** gains two new fields:

```rust
/// Sum of all request `duration_ms` values.
total_duration_ms: u64,
/// Mean of all request `duration_ms` values (0 if no requests ran).
avg_response_ms: u64,
```

### `crates/strex-ui/src/ws.rs`

**Body truncation constant:**
```rust
const BODY_LIMIT: usize = 10_240;
```

**Body extraction helper:**
```rust
fn truncate_body(body: &str) -> String {
    // Find last valid UTF-8 character boundary at or before BODY_LIMIT
    if body.len() <= BODY_LIMIT {
        body.to_string()
    } else {
        let boundary = (0..=BODY_LIMIT)
            .rev()
            .find(|&i| body.is_char_boundary(i))
            .unwrap_or(0);
        format!("{} [truncated]", &body[..boundary])
    }
}
```

**Populating new fields — both code paths:**

Both the **single-run** path (`execute_collection`) and the **data-driven** path (`run_collection_with_data`) build `WsEvent::RequestCompleted` from each `RequestResult`. In both paths, extract body and headers from `req_result.response: Option<HttpResponse>`:

```rust
let response_body = req_result.response.as_ref().map(|r| truncate_body(&r.body));
let response_headers = req_result.response.as_ref().map(|r| r.headers.clone());
```

**Duration tracking for `RunFinished`:**

- **Single-run path:** accumulate a `Vec<u64>` of `duration_ms` values while emitting `RequestCompleted` events. After the loop, compute `total` and `avg`.
- **Data-driven path:** after `run_collection_with_data` returns, iterate `result.iterations` (the actual field name in `DataRunResult`) to sum each iteration's `collection_result.request_results[*].duration_ms`. Compute `total` and `avg` before emitting `RunFinished`.

```rust
let total_duration_ms: u64 = durations.iter().sum();
let avg_response_ms: u64 = if durations.is_empty() {
    0
} else {
    total_duration_ms / durations.len() as u64
};
```

### `crates/strex-ui/src/request_list.rs` (new file)

Single async handler: `list_collection_requests`.

**Query param:** `file` — filename relative to CWD (e.g. `jsonplaceholder.yaml`).

**Security:** Reject the request with HTTP 400 if `file` is an absolute path or contains `..` components:
```rust
let p = Path::new(&params.file);
if p.is_absolute() || p.components().any(|c| c == std::path::Component::ParentDir) {
    return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid file path"}))).into_response();
}
```

**Execution:** Call `parse_collection` inside `tokio::task::spawn_blocking` (per ADR-0004 — no blocking I/O on async threads):
```rust
let file_path = std::env::current_dir()?.join(&params.file);
let collection = tokio::task::spawn_blocking(move || parse_collection(&file_path)).await??;
```

On parse failure: return HTTP 400 with `{"error": "<message>"}`.

On success: return HTTP 200 with `Vec<CollectionRequestItem>` as JSON:
```rust
#[derive(Serialize)]
pub struct CollectionRequestItem {
    pub name: String,
    pub method: String,
}
```

Registered in `server.rs` as `GET /api/collection-requests`.

**Tests:**
- Path traversal attempt (`../etc/passwd`) returns error response.
- Absolute path returns error response.
- Valid filename returns correct `[{name, method}]` list.

---

## Frontend Data Layer

### `src/lib/types.ts`

Extend `request_completed` union member:

```typescript
response_body: string | null
response_headers: Record<string, string> | null
```

Extend `run_finished` union member:

```typescript
total_duration_ms: number
avg_response_ms: number
```

Extend `RequestResult` interface (used by `App.svelte` when building results and by `RequestRow.svelte` when rendering):

```typescript
export interface RequestResult {
  name: string
  method: string
  passed: boolean
  status: number | null
  duration_ms: number
  failures: string[]
  error: string | null
  response_body: string | null      // new
  response_headers: Record<string, string> | null  // new
}
```

Add new type:

```typescript
export interface RequestSequenceItem {
  name: string
  method: string
}
```

### `src/lib/api.ts`

Add:

```typescript
export async function fetchCollectionRequests(file: string): Promise<RequestSequenceItem[]>
```

Calls `GET /api/collection-requests?file=<file>`. Throws on non-OK response.

---

## Frontend UI

### `App.svelte`

The existing `summary` state is `{ passed: number; failed: number } | null`. Extend it to include the timing fields (avoiding a second prop):

```typescript
let summary = $state<{
  passed: number
  failed: number
  total_duration_ms: number
  avg_response_ms: number
} | null>(null)
```

When handling the `run_finished` event:

```typescript
summary = {
  passed: event.passed,
  failed: event.failed,
  total_duration_ms: event.total_duration_ms,
  avg_response_ms: event.avg_response_ms,
}
```

When mapping `request_completed` events into `RequestResult` objects, include the new fields:

```typescript
results = [
  ...results,
  {
    // existing fields...
    response_body: event.response_body,
    response_headers: event.response_headers,
  },
]
```

### `RequestRow.svelte`

**Expand trigger:** `hasDetails = failures.length > 0 || !!error || !!response_body || !!response_headers`

**Expanded area — two tabs: Response | Headers**

- Tab bar rendered only when `response_body !== null || response_headers !== null`.
- Failures and errors render above the tab bar (unchanged).
- Active tab: local `$state`, defaults to `'response'`.
- **Response tab:** `<pre>` with `overflow-y: auto; max-height: 300px; font-family: monospace; white-space: pre-wrap`. If body ends with `" [truncated]"`, append a note in muted italic: _"Response truncated at 10 KB"_.
- **Headers tab:** `<table>` with two columns — header name (monospace, bold) / value.

### `ResultsPanel.svelte`

The `summary` prop type updates to match the extended shape from `App.svelte`.

**Stats bar** — shown as soon as `running` becomes true (use `results.length > 0 || running` as visibility condition), stays after completion. Positioned below the "RESULTS" header, above the filter tabs:

```
5 requests  ·  5 passed  ·  0 failed  ·  481ms total  ·  avg 96ms
```

While running, `total_duration_ms` and `avg_response_ms` are `null` (summary not yet received) — show only the live request count and pass/fail counts derived from the `results` array. Timing fields appear once `summary` is non-null.

**Filter tabs** — `All | Passed | Failed | Errors` — rendered below the stats bar:

- `All` — show all results (default).
- `Passed` — `result.passed === true && !result.error`.
- `Failed` — `result.passed === false && !result.error`.
- `Errors` — `!!result.error`.
- Active tab stored as local `$state`.
- `$derived` computed list: `filteredResults = results.filter(...)`.
- `{#each filteredResults as result, i (i)}`.

Replace the existing footer summary with the stats bar (no separate footer needed).

### `ConfigPanel.svelte`

When `selectedCollection` changes (via `$effect`):

1. Call `fetchCollectionRequests(selectedCollection)`.
2. While loading: show `"Loading requests…"` in muted text below the collection picker.
3. On success: render a numbered read-only list:
   ```
   1.  GET   Get user
   2.  GET   Get posts by user
   3.  POST  Create post
   ```
   Method badge uses the same `methodColors` map as `RequestRow`. Font size 0.8rem, muted color.
4. On error or empty: hide the list silently (`.catch` logs to console).
5. Clear the list when `selectedCollection` is empty.

---

## Data Flow Summary

```
Browser selects collection
  → GET /api/collection-requests?file=foo.yaml
  → ConfigPanel renders read-only sequence list

User clicks Run
  → WS connect, send RunConfig JSON
  → Server: RunStarted { total }
  → Server: RequestCompleted { ..., response_body, response_headers } × N
  → Server: RunFinished { passed, failed, total_duration_ms, avg_response_ms }

Browser:
  - Stats bar updates live during run (timing shown after RunFinished)
  - Filter tabs filter completed results
  - Click any row → Response / Headers tabs appear
```

---

## File Changes

| File | Change |
|------|--------|
| `crates/strex-ui/src/events.rs` | Add `response_body`, `response_headers` to `RequestCompleted`; add `total_duration_ms`, `avg_response_ms` to `RunFinished` |
| `crates/strex-ui/src/ws.rs` | `truncate_body` helper; populate new fields in both run paths; track durations for summary |
| `crates/strex-ui/src/request_list.rs` | New — `list_collection_requests` handler with path validation and `spawn_blocking` |
| `crates/strex-ui/src/lib.rs` | Add `mod request_list` |
| `crates/strex-ui/src/server.rs` | Register `GET /api/collection-requests` |
| `frontend/src/lib/types.ts` | Extend event union types, extend `RequestResult`, add `RequestSequenceItem` |
| `frontend/src/lib/api.ts` | Add `fetchCollectionRequests` |
| `frontend/src/App.svelte` | Extend `summary` state type; map `response_body`/`response_headers` into results |
| `frontend/src/components/RequestRow.svelte` | Response/Headers tabs in expanded area |
| `frontend/src/components/ResultsPanel.svelte` | Stats bar, filter tabs, updated `summary` prop type |
| `frontend/src/components/ConfigPanel.svelte` | Request sequence list |
