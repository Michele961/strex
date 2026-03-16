# Request Body Visibility — Design Spec

## Goal

Show the outgoing request body in the UI results panel so users can inspect what was sent alongside what was received.

## Background

`ResolvedRequest` is constructed in `runner.rs` and consumed entirely by `http::send`. The body (text, JSON, or form-encoded) is never stored and never reaches the frontend. `HttpResponse` today captures only the response side of the exchange.

## Approach

Extend `HttpResponse` (Option 1 from brainstorming) so it holds both sides of the HTTP exchange. The request body is serialised to a display string before reqwest consumes it, truncated to 10 KB via the existing `truncate_body` helper in `ws.rs`, and emitted on `WsEvent::RequestCompleted` alongside the existing `response_body`.

The frontend adds a permanent "Request" tab to every expanded request row. When a body was sent it renders the raw text; when there is no body (GET, HEAD, DELETE with no body) it renders a muted "No request body" message. Because the Request tab always exists, every row is always expandable.

## Architecture

### Backend — `strex-core`

**`crates/strex-core/src/http.rs`**

Add one new field to `HttpResponse`:
```rust
/// Serialised outgoing request body, for display in the UI.
/// `None` when the request had no body.
pub request_body: Option<String>,
```

Add a private helper that converts a concrete `ResolvedBody` to a display string:
```rust
fn display_body(body: &ResolvedBody) -> String { ... }
```

The helper covers each variant:
- `ResolvedBody::Text(s)` → `s.clone()`
- `ResolvedBody::Json(v)` → `serde_json::to_string_pretty(v).unwrap_or_else(|_| String::new())`
  — `unwrap_or_else` is intentional: this is a best-effort display helper. `serde_json::to_string_pretty` can fail when a `Value` contains maps with non-string keys, an edge case that cannot occur via strex's own YAML parser but is theoretically possible. A serialisation failure produces an empty string rather than an error, which is acceptable for display purposes. The call site must carry a comment: `// display_body is best-effort; serialisation failure yields an empty string`.
- `ResolvedBody::Form(m)` → URL-encoded string: collect the `HashMap` entries into a `Vec`, sort by key alphabetically, then feed to `url::form_urlencoded::Serializer` for deterministic output.

In `send()`, before the body is moved into reqwest, capture the display string:
```rust
let request_body: Option<String> = request.body.as_ref().map(display_body);
```
Then store `request_body` on the returned `HttpResponse`.

#### Dependency: `url` crate

`url::form_urlencoded` is used for form body encoding. Check `crates/strex-core/Cargo.toml` to see if `url` is already listed. If not, add it as a direct dependency with a justification comment:
```toml
# url: form_urlencoded::Serializer used in display_body to render form request bodies for UI display
url = "2"
```

### Backend — `strex-ui`

**`crates/strex-ui/src/events.rs`**

`WsEvent::RequestCompleted` gains one new field with a doc comment (required by CLAUDE.md for all `pub` items):
```rust
/// Serialised outgoing request body, truncated to 10 240 bytes. `None` when the request had no body.
request_body: Option<String>,
```

**Note — existing tests:** Both existing unit tests in `events.rs` that construct `WsEvent::RequestCompleted { passed, status, failures, error, response_body, response_headers }` by name will not compile. Both must add `request_body: None` to their struct literals.

**`crates/strex-ui/src/ws.rs`**

The `OutcomeFields` type alias is currently a 6-tuple:
```rust
type OutcomeFields = (bool, Option<u16>, Vec<String>, Option<String>, Option<String>, Option<HashMap<String, String>>);
//                   ^^^^  ^^^^^^^^^^^^ ^^^^^^^^^^^^ ^^^^^^^^^^^^^   ^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//                   passed status       failures     error           response_body   response_headers
```

It must be extended to a 7-tuple by appending `request_body` as the last element:
```rust
type OutcomeFields = (bool, Option<u16>, Vec<String>, Option<String>, Option<String>, Option<HashMap<String, String>>, Option<String>);
//                   ^^^^  ^^^^^^^^^^^^ ^^^^^^^^^^^^ ^^^^^^^^^^^^^   ^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^   ^^^^^^^^^^^^^
//                   passed status       failures     error           response_body   response_headers                 request_body
```

`outcome_fields` accesses `request_body` via the existing `response: &Option<HttpResponse>` parameter:
```rust
let request_body = response
    .as_ref()
    .and_then(|r| r.request_body.as_deref().map(truncate_body));
```
It is returned as the 7th element of the tuple.

Both call sites in `ws.rs` where the tuple is destructured must add the new element:
```rust
let (passed, status, failures, error, response_body, response_headers, request_body) = outcome_fields(...);
```

Both run paths (single and data-driven) pass `request_body` through to the emitted `WsEvent::RequestCompleted`.

**Note — existing tests in `ws.rs`:** Two existing tests destructure `OutcomeFields` by position:
- `outcome_fields_passed` — `let (passed, status, failures, error, body, headers) = outcome_fields(...)`
- `outcome_fields_assertions_failed` — `let (passed, _status, failures, error, _body, _headers) = outcome_fields(...)`

Both will not compile after the 7-tuple change. Both must add `_request_body` (or a named binding) as the 7th element.

### Frontend

**`crates/strex-ui/frontend/src/lib/types.ts`**

- `request_completed` union member gains `request_body: string | null`.
- `RequestResult` interface gains `request_body: string | null`.

**`crates/strex-ui/frontend/src/App.svelte`**

The `request_completed` handler maps `e.request_body` onto the result object (same pattern as `response_body`).

**`crates/strex-ui/frontend/src/components/RequestRow.svelte`**

Because the Request tab always exists, every row is always expandable:

- `hasDetails`: remove the `$derived(...)` expression and replace with `const hasDetails = true`.
- `hasTabs`: remove the `$derived(...)` expression and replace with `const hasTabs = true`.
- `activeTab` type is widened: `$state<'request' | 'response' | 'headers'>('response')` — default remains `'response'` so the first thing users see on expand is the received response.
- **Truncation indicator:** Request bodies are capped at 10 KB by `truncate_body` (which appends `" [truncated]"`). No additional UI truncation note is shown for the Request tab — the `" [truncated]"` suffix in the raw body text is sufficient. The existing `isTruncated` derived value applies only to the response body and is left unchanged.

Tab bar order: **Request → Response → Headers** (sent side first, then received).

Request tab content:
- When `result.request_body` is non-null: `<pre class="body-pre">` with the body text (same style as the Response tab).
- When null: `<p class="no-body">No request body</p>` in muted style.

## Data Flow

```
http.rs / send()
  request.body: Option<ResolvedBody>
    → request.body.as_ref().map(display_body) → Option<String>  [step 1: serialise to display string]
    → stored as HttpResponse { request_body: Option<String>, body, status, headers, timing }

runner.rs
  RequestResult { outcome: Passed/Failed(HttpResponse) }

ws.rs / outcome_fields(outcome, response: &Option<HttpResponse>)
  response.as_ref().and_then(|r| r.request_body.as_deref().map(truncate_body))
    → Option<String>                                             [step 2: cap at 10 KB]
    → returned as 7th element of OutcomeFields 7-tuple
  → WsEvent::RequestCompleted { request_body: Option<String>, … }

WebSocket → frontend
  App.svelte maps request_body → RequestResult
  RequestRow.svelte renders Request tab (always visible)
```

## Body Serialisation Rules

| Body type | Display format |
|-----------|----------------|
| None      | `None` (tab shows "No request body") |
| Text      | Raw string as-is |
| JSON      | `serde_json::to_string_pretty` (empty string on serialisation failure) |
| Form      | `key=value&key2=value2` — entries collected into Vec, sorted by key alphabetically, percent-encoded via `url::form_urlencoded::Serializer` |

## Testing

**`strex-core` (unit, in `http.rs`)**
- `display_body_text` — text body returns raw string unchanged
- `display_body_json` — JSON body returns pretty-printed JSON string
- `display_body_form` — form body with multiple keys returns alphabetically sorted URL-encoded string; verify ordering is deterministic

**`strex-ui` (unit, in `ws.rs`)**
- `outcome_fields_includes_request_body` — passed result with request body flows through `outcome_fields` as the 7th element
- `outcome_fields_truncates_long_request_body` — body > 10 KB is truncated to `BODY_LIMIT + " [truncated]".len()`
- **Update existing** `outcome_fields_passed` — add `_request_body` as 7th binding in destructure
- **Update existing** `outcome_fields_assertions_failed` — add `_request_body` as 7th binding in destructure

**`strex-ui` (existing tests to update, in `events.rs`)**
- Both existing `WsEvent::RequestCompleted` tests must add `request_body: None` to the struct literal.

**Frontend (visual)**
- Request tab renders body text for POST/PUT/PATCH
- "No request body" renders for GET
- Default active tab is Response, not Request
- Row is expandable even for a passing GET request with no body, no failures, no error

## Out of Scope

- Sent request headers in the UI (separate feature if needed later)
- Binary/multipart bodies (not supported by strex-core today)
