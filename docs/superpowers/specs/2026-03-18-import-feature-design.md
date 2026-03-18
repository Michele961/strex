# Import Feature Design

**Date:** 2026-03-18
**Status:** Approved
**Scope:** UI-only (CLI import deferred to a future iteration)

---

## Overview

Users can generate a Strex YAML collection from an existing source — a curl command or an OpenAPI/Swagger spec — directly from the web UI. The generated YAML is previewed before being saved to disk and auto-selected in the collection dropdown.

---

## Sources Supported

| Source | Priority | Input method |
|--------|----------|--------------|
| curl command | MVP | Paste raw curl string into modal |
| OpenAPI / Swagger 2.x & 3.x | MVP | File path or URL (auto-detected) |
| Postman collection | Deferred | — |

---

## Output Modes

Chosen by the user at import time inside the modal:

- **Quick scaffold** — generates requests with method, URL, headers, and body only. No assertions.
- **Generate tests** — generates requests plus basic assertions (`status: 200`) and, for OpenAPI, existence checks on required response properties derived from the response schema.

---

## UI Design

### Entry point

A secondary `+ Import` button is added to the bottom of `ConfigPanel.svelte`, below the `Run` button.

### Interaction

Clicking `+ Import` opens a **centered modal overlay** that dims the rest of the app. The modal contains a 3-step internal state machine:

```
source-select → input-form → preview
                    ↑              |
                  (back)    (save → close + refresh dropdown)
```

#### Step 1 — Source selection

Three tiles: `curl command` (active), `OpenAPI / Swagger` (active), `Postman collection` (disabled, "coming soon").

#### Step 2 — Input form

Fields shown depend on source type:

**curl:**
- Multiline paste area for the raw curl command
- Output mode toggle: `Quick scaffold` | `Generate tests`
- `Generate →` button

**OpenAPI:**
- Single text input accepting a file path (`./spec.yaml`) or a URL (`https://...`). Auto-detected on Generate.
- Output mode toggle: `Quick scaffold` | `Generate tests`
- `Generate →` button

#### Step 3 — Preview

- Read-only `<pre>` block showing the generated YAML (monospace, dark theme — no syntax highlighting library required for MVP)
- Editable filename input (default: `imported-collection.yaml`)
- `Save` button → writes file to CWD, closes modal, refreshes collection list, auto-selects the new file
- `← Back` link: discards `generatedYaml`, returns to step 2 with `input`, `source`, and `mode` still populated so the user can tweak and re-generate without re-typing

**Default filename:** `imported-collection.yaml`. Post-MVP improvement: derive from OpenAPI `info.title` (slugified).

On save success: refresh the collection dropdown via `fetchCollections()` and auto-select the new file, then close the modal.

### Frontend state

```ts
type Step   = 'source-select' | 'input-form' | 'preview'
type Source = 'curl' | 'openapi'
type Mode   = 'scaffold' | 'with_tests'

let step:          Step
let source:        Source
let input:         string        // pasted curl or path/URL
let mode:          Mode   = 'scaffold'
let generatedYaml: string
let filename:      string        // default: "imported-collection.yaml"
let generating:    boolean
let saving:        boolean
let error:         string | null
```

---

## Architecture

### New crate: `crates/strex-import/`

Pure conversion logic with no I/O. Dependencies: `serde_yaml`, `thiserror`.

```
crates/strex-import/
├── src/
│   ├── lib.rs        — public API
│   ├── curl.rs       — curl parser
│   ├── openapi.rs    — OpenAPI 2.x / 3.x converter
│   └── error.rs      — ImportError (thiserror)
└── Cargo.toml
```

**Public API:**

```rust
/// Whether to generate only request scaffolding or include assertions.
pub enum ImportMode {
    /// Generate method, URL, headers, and body only — no assertions.
    Scaffold,
    /// Generate requests plus basic assertions derived from the source.
    WithTests,
}

/// Parse a curl command and return a Strex YAML collection string.
///
/// Sensitive header and body values are replaced with `{{variable}}` placeholders.
pub fn from_curl(input: &str, mode: ImportMode) -> Result<String, ImportError>;

/// Convert an OpenAPI/Swagger spec (as a YAML or JSON string) and return a Strex YAML collection string.
///
/// Accepts both YAML and JSON input — `serde_yaml::from_str` handles both formats
/// since JSON is a valid subset of YAML; no separate JSON branch is required.
pub fn from_openapi(spec: &str, mode: ImportMode) -> Result<String, ImportError>;
```

Both functions return the generated YAML as a `String`. This avoids coupling to strex-core's internal types and keeps serialization concerns local to the crate.

**Doc comments:** All `pub` items in `strex-import` (types, variants, functions, error variants) must have `///` doc comments. `cargo clippy -- -D warnings` enforces `missing_docs` and will fail without them.

### Backend: `strex-ui`

Two new endpoints added to `routes.rs`:

#### `POST /api/import/generate`

```json
// Request
{
  "source": "curl" | "openapi",
  "input": "...",
  "mode": "scaffold" | "with_tests"
}

// 200 Response
{ "yaml": "name: \"Imported Collection\"\n..." }

// 400 Response
{ "error": "Failed to parse curl command: unexpected token at position 12" }

// 500 Response (I/O or fetch error)
{ "error": "Could not read file: No such file or directory" }
```

For `openapi`: the route handler detects whether `input` starts with `http://` or `https://` (URL → fetch via `reqwest`) or treats it as a file path (`fs::read_to_string`). The raw spec string is passed to `strex_import::from_openapi(...)`.

**OpenAPI URL fetch:** A `reqwest` client with a **10-second timeout** is constructed once and stored in Axum state (not created per-request). If the fetch times out, return `400` with `ImportError::FetchTimeout`. File-path input for OpenAPI is unrestricted (no path sanitisation) — this is an intentional local-tool tradeoff; users run `strex ui` on their own machine and are trusted to provide valid paths.

#### `POST /api/import/save`

```json
// Request
{ "yaml": "...", "filename": "my-collection.yaml" }

// 200 Response
{ "filename": "my-collection.yaml" }

// 400  filename must end in .yaml
// 400  filename must not contain path separators (/ or \) or ..
// 409  file already exists
// 500  CWD unavailable or I/O error
```

Writes to CWD. Rejects filenames containing `/`, `\`, or `..` to prevent directory traversal. Use `OpenOptions::create_new(true)` to atomically fail if the file already exists — do **not** use a separate `Path::exists()` check (TOCTOU race).

---

## Conversion Logic

### curl parser (`curl.rs`)

Supported flags: `-X`, `-H` / `--header`, `-d` / `--data` / `--data-raw`, `-u` / `--user`, `--url`, and bare URL as positional argument.

**Method inference:** defaults to `GET`; inferred as `POST` when `-d` is present and no `-X` is given.

**Request name:** `"METHOD /path"` derived from the URL (e.g. `"POST /users"`).

**Sensitive value scrubbing** — values replaced with `{{variable}}` placeholders:

| Trigger | Placeholder |
|---------|-------------|
| `Authorization` header | `{{authorization}}` |
| `X-Api-Key` header | `{{api_key}}` |
| `X-Auth-Token` header | `{{auth_token}}` |
| `Cookie` header | `{{cookie}}` |
| `-u user:password` | `Authorization: Basic {{credentials}}` |
| JSON body fields: `password`, `secret`, `token`, `api_key` | `{{field_name}}` |

This list is **exhaustive for MVP**. No additional headers or body fields are scrubbed without an explicit spec update. Tests must verify that unlisted headers are passed through unmodified.

### OpenAPI converter (`openapi.rs`)

**Version detection:** presence of `openapi:` key → OpenAPI 3.x; `swagger:` key → Swagger 2.x.

**Base URL:**
- OpenAPI 3.x: `servers[0].url` → `environment.baseUrl`. If `servers` is absent or empty, emit `baseUrl: "/"` and include a YAML comment `# TODO: replace baseUrl with your API base URL` so the user is warned.
- Swagger 2.x: `scheme://host + basePath` → `environment.baseUrl`. If `host` is absent, use `"/"` with the same comment.

**Request generation:** one request per operation, in spec order.

| Field | Source |
|-------|--------|
| `name` | `operationId` if present, else `"METHOD /path"` |
| `method` | HTTP verb |
| `url` | `"{{baseUrl}}/path"` with path params as `{{param_name}}` |
| `headers` | `Content-Type` derived from `requestBody` media type |
| `body` | Shape derived from `requestBody` schema (scaffold mode: structure only) |

**Scaffold mode:** method, url, headers, body shape.

**WithTests mode:** adds per-request:
```yaml
assertions:
  - status: 201               # derived from the first 2xx response code in the operation's `responses`
                              # object; falls back to 200 if no 2xx code is declared
  - jsonPath: "$.fieldName"   # for each required property in response schema
    exists: true
```

---

## Error Handling

`strex-import` uses `thiserror`. `strex-ui` maps `ImportError` variants to appropriate HTTP status codes (all `400` for parse/conversion failures, `500` for I/O errors). The frontend displays the error message inline in the modal below the Generate button, keeping the user in context to correct their input.

---

## Testing

- `strex-import`: unit tests per converter covering happy paths, common edge cases (no method flag, multiline body, path params, missing `operationId`), and sensitive value scrubbing
- `strex-ui`: integration tests for both new endpoints using `wiremock` (already a dev dependency) and temp files

---

## Out of Scope (this iteration)

- CLI subcommand (`strex import`)
- Postman collection import
- Syntax highlighting in the YAML preview
- Editing the YAML before saving
- Importing multiple operations as separate files
