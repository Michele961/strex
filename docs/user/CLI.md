# strex CLI documentation

strex is a command-line API testing tool. Collections of HTTP requests are defined in YAML and executed with `strex run`. This document covers both how to use it (tutorial) and what every option does (reference).

**Commands:**
- [`strex run`](#strex-run) — execute a collection
- [`strex validate`](#strex-validate) — check a collection without making HTTP requests

---

## Part 1 — Tutorial

### Your first collection

Create a file called `api.yaml`:

```yaml
name: Reqres demo
version: "1.0"

environment:
  baseUrl: https://reqres.in/api

requests:
  - name: List users
    method: GET
    url: "{{baseUrl}}/users?page=1"
    assertions:
      - status: 200
      - jsonPath: "$.data"
        exists: true
```

Run it:

```bash
strex run api.yaml
```

```
GET  List users   ✓

1 requests · 1 passed · 0 failed
```

Exit code `0` means all assertions passed. Exit code `1` means at least one failed.

---

### Adding assertions

Assertions are evaluated against the HTTP response. There are three types.

**Status assertion** — checks the HTTP status code:

```yaml
assertions:
  - status: 201
```

**JSON path assertion** — evaluates a [JSONPath](https://goessner.net/articles/JsonPath/) expression against the response body:

```yaml
assertions:
  - jsonPath: "$.user.id"
    exists: true           # field must be present (any value)

  - jsonPath: "$.user.role"
    equals: admin          # field must equal this value

  - jsonPath: "$.message"
    contains: success      # field must contain this substring
```

**Header assertion** — checks a response header (names are case-insensitive):

```yaml
assertions:
  - header: content-type
    equals: application/json

  - header: x-request-id
    exists: true
```

Multiple assertions per request are all evaluated; all failures are reported even if one fails early.

---

### Variables and scripts

Variables allow values to flow between requests in a collection.

**Declare variables** in the collection header. `null` means required with no default:

```yaml
variables:
  token: null
  userId: null
```

**Use variables** in any string field with `{{varName}}`:

```yaml
url: "{{baseUrl}}/users/{{userId}}"
headers:
  Authorization: "Bearer {{token}}"
```

**Set variables from a post-request script** — scripts run after the response is received and can read and write collection variables:

```yaml
requests:
  - name: Login
    method: POST
    url: "{{baseUrl}}/auth/login"
    body:
      type: json
      content:
        email: admin@example.com
        password: secret
    assertions:
      - status: 200
    post_script: |
      const body = response.json();
      variables.set("token", body.token);
      variables.set("userId", body.user.id.toString());

  - name: Get profile
    method: GET
    url: "{{baseUrl}}/users/{{userId}}"
    headers:
      Authorization: "Bearer {{token}}"
    assertions:
      - status: 200
      - jsonPath: "$.email"
        equals: admin@example.com
```

Scripts can also run _before_ the request using `pre_script` — useful for generating dynamic values before URL interpolation.

---

### Data-driven testing

Run a collection against multiple rows of input using `--data`. Each row becomes one iteration.

**CSV file** (`users.csv`):

```csv
email,password,expected_role
alice@example.com,pass1,admin
bob@example.com,pass2,viewer
carol@example.com,pass3,editor
```

**Collection** — data row fields are available as variables alongside declared collection variables:

```yaml
name: Role verification
version: "1.0"

environment:
  baseUrl: https://api.example.com

requests:
  - name: Login as {{email}}
    method: POST
    url: "{{baseUrl}}/auth/login"
    body:
      type: json
      content:
        email: "{{email}}"
        password: "{{password}}"
    assertions:
      - status: 200
    post_script: |
      variables.set("token", response.json().token);

  - name: Check role
    method: GET
    url: "{{baseUrl}}/me"
    headers:
      Authorization: "Bearer {{token}}"
    assertions:
      - status: 200
      - jsonPath: "$.role"
        equals: "{{expected_role}}"
```

**Run with data:**

```bash
strex run roles.yaml --data users.csv
```

```
── Row 1 (email=alice@example.com, password=pass1, expected_role=admin) ───
  POST  Login as alice@example.com   ✓
  GET   Check role                   ✓
── Row 2 (email=bob@example.com, password=pass2, expected_role=viewer) ────
  POST  Login as bob@example.com     ✓
  GET   Check role                   ✓
── Row 3 (email=carol@example.com, password=pass3, expected_role=editor) ──
  POST  Login as carol@example.com   ✓
  GET   Check role                   ✓

3 iterations · 3 passed · 0 failed
```

**Run iterations concurrently:**

```bash
strex run roles.yaml --data users.csv --concurrency 4
```

Each iteration gets its own isolated variable context — no shared state between concurrent rows.

**Stop after first failure:**

```bash
strex run roles.yaml --data users.csv --fail-fast
```

---

## Part 2 — Reference

### Collection format

A collection file is a YAML document with the following top-level fields:

```yaml
name: string           # required — human-readable collection name
version: string        # required — schema version, currently "1.0"
environment: {}        # optional — key/value pairs, immutable, lowest priority
variables: {}          # optional — key/value pairs, mutable per-iteration
requests: []           # required — ordered list of request definitions
```

#### `environment`

Key/value pairs that are global and immutable. Use for base URLs and other constants:

```yaml
environment:
  baseUrl: https://api.example.com
  timeout: 30
```

#### `variables`

Key/value pairs that can be modified by scripts during execution. `null` declares a required variable with no default value:

```yaml
variables:
  token: null           # required — no default
  retries: "3"          # optional with default
```

#### `requests`

An ordered list of request definitions. Requests execute sequentially. Each request:

```yaml
requests:
  - name: string          # required — unique name for this request
    method: string        # required — HTTP method: GET, POST, PUT, PATCH, DELETE, etc.
    url: string           # required — full URL; may contain {{variables}}
    headers: {}           # optional — key/value header map; values may contain {{variables}}
    body:                 # optional
      type: json|text|form
      content: ...        # varies by type (see below)
    pre_script: |         # optional — JS script runs before the request
      ...
    post_script: |        # optional — JS script runs after the response is captured
      ...
    assertions: []        # optional — list of assertion maps (see below)
    timeout: 30000        # optional — per-request timeout in milliseconds (default: 60000)
```

#### Request body types

**`json`** — serialized as `application/json`. `content` is a YAML mapping or sequence:

```yaml
body:
  type: json
  content:
    email: "{{email}}"
    role: admin
    tags: [a, b, c]
```

**`text`** — sent as `text/plain`. `content` is a string:

```yaml
body:
  type: text
  content: "raw body content"
```

**`form`** — URL-encoded as `application/x-www-form-urlencoded`. `content` is a YAML mapping:

```yaml
body:
  type: form
  content:
    grant_type: password
    username: "{{email}}"
    password: "{{password}}"
```

#### Assertions

Each assertion is a YAML mapping. Multiple assertions in the list are all evaluated:

```yaml
assertions:
  # Status code
  - status: 200

  # JSON path — checks a JSONPath expression against the response body
  - jsonPath: "$.id"
    exists: true        # true = must exist, false = must not exist

  - jsonPath: "$.name"
    equals: "Alice"     # must equal this value (string comparison after JSONPath extraction)

  - jsonPath: "$.bio"
    contains: "engineer"  # must contain this substring

  # Header — case-insensitive header name
  - header: content-type
    equals: application/json

  - header: x-rate-limit-remaining
    exists: true
```

#### Forbidden YAML constructs

strex uses a strict YAML subset. The following constructs are rejected at parse time:

- YAML anchors (`&anchor`), aliases (`*alias`), and merge keys (`<<`)
- Duplicate keys at any level
- YAML tags (`!!str`, `!!int`, etc.)

---

### Variable resolution

All three variable layers are merged into a flat namespace before template interpolation. When the same key exists in multiple layers, the higher-priority layer wins:

| Priority | Layer | Mutable | Scope |
|----------|-------|---------|-------|
| Highest | `data` (from `--data` file) | No | Per-iteration |
| Mid | `variables` (collection variables) | Yes (via scripts) | Per-iteration |
| Lowest | `environment` | No | Global |

All variables are accessed with `{{varName}}` — no prefix needed regardless of which layer they come from. If the same key appears in the data row and in `variables`, the data row value wins.

---

### Script API

Scripts are JavaScript (ES6+) running in a sandboxed QuickJS environment. They have no access to the filesystem, network, or system APIs.

#### Available globals

**`variables`** — read and write collection variables:

```javascript
variables.get("token")           // returns string or "" if not set
variables.set("token", "abc123") // set a value (always coerced to string)
variables.has("token")           // returns boolean
variables.delete("token")        // remove a variable
variables.clear()                // remove all variables
variables.keys()                 // returns array of current variable names
```

**`response`** — available in `post_script` only:

```javascript
response.status        // number — HTTP status code, e.g. 200
response.statusText    // string — e.g. "OK"
response.body          // string — raw response body
response.text()        // function — same as body
response.json()        // function — parses body as JSON, returns object
response.headers       // object — all headers, lowercased names
response.headers["content-type"]  // e.g. "application/json"
response.timing.wait   // number — ms from request sent to first byte
response.timing.total  // number — ms for full request lifecycle
```

**`env`** — read environment variables (read-only):

```javascript
env.baseUrl            // reads the environment block
env["api-key"]         // bracket syntax for keys with hyphens
```

**`data`** — read the current data row (read-only):

```javascript
data.email             // reads a field from the current CSV/JSON row
data["user-id"]        // bracket syntax
```

**`console`** — logging (output visible with `--verbose`, future flag):

```javascript
console.log("value:", someVar)
console.warn("something looks off")
console.error("something went wrong")
```

#### Assertion functions

Available in both `pre_script` and `post_script`. Failures are collected as assertion failures (same as declarative assertions):

```javascript
assert(condition, "message if false")
assertEqual(actual, expected, "optional message")
assertNotEqual(actual, expected, "optional message")
assertContains(string, substring, "optional message")
```

#### Example: extract and chain a token

```yaml
post_script: |
  const body = response.json();
  if (!body.token) {
    assert(false, "Login response missing token field");
  }
  variables.set("token", body.token);
  variables.set("userId", String(body.user.id));
```

---

### `strex run`

Execute a collection, optionally with a data file.

```
strex run <collection> [OPTIONS]
```

| Argument / Flag | Type | Default | Description |
|-----------------|------|---------|-------------|
| `<collection>` | path | — | Path to the YAML collection file |
| `--data <path>` | path | — | CSV or JSON data file; enables data-driven mode |
| `--concurrency <n>` | integer | `1` | Maximum concurrent iterations (data-driven only) |
| `--fail-fast` | flag | off | Stop after the first iteration failure (data-driven only) |
| `--format <fmt>` | enum | `console` | Output format: `console`, `json`, or `junit` |
| `--output <path>` | path | — | Write output to a file instead of stdout |

**Examples:**

```bash
# Basic run
strex run api.yaml

# Data-driven with concurrency
strex run load.yaml --data rows.csv --concurrency 10

# JUnit output to file for CI
strex run suite.yaml --format junit --output results/junit.xml

# Fail fast on first bad row
strex run smoke.yaml --data inputs.csv --fail-fast

# JSON output piped to jq
strex run api.yaml --format json | jq '.requests[] | select(.passed == false)'
```

---

### `strex validate`

Parse a collection file and check that all `{{variable}}` references are declared in `variables`. No HTTP requests are made.

```
strex validate <collection>
```

| Argument | Type | Description |
|----------|------|-------------|
| `<collection>` | path | Path to the YAML collection file |

**Success output (exit 0):**

```
valid: api.yaml (4 requests, 0 unresolved variables)
```

**Unresolved variable (exit 2):**

```
error: unresolved variable `baseUrl` in requests[0].url
  declared variables: token, userId
```

**Parse error (exit 2):**

```
error: unknown field `metod` in request, did you mean `method`?
```

---

### Output formats

#### Console (default)

Human-readable, coloured output suitable for local development.

```
GET  Create user      ✓
POST Authenticate     ✓
GET  Protected page   ✗
       assertion failed: status expected 200, got 401

3 requests · 2 passed · 1 failed
```

Network errors appear as:

```
GET  Flaky endpoint   ✗
       error: connection refused (http://localhost:9999)
```

Data-driven runs show one block per iteration:

```
── Row 1 (email=alice@example.com) ──────────────────────
  POST  Login   ✓
  GET   Profile ✓
── Row 2 (email=bob@example.com) ────────────────────────
  POST  Login   ✗
    assertion failed: status expected 200, got 401

2 iterations · 1 passed · 1 failed
```

#### JSON

A JSON object suitable for programmatic processing:

```json
{
  "passed": 2,
  "failed": 1,
  "requests": [
    {
      "name": "Create user",
      "passed": true,
      "status": 201,
      "assertions": []
    },
    {
      "name": "Protected page",
      "passed": false,
      "status": 401,
      "assertions": [
        { "passed": false, "message": "status expected 200, got 401" }
      ]
    },
    {
      "name": "Flaky endpoint",
      "passed": false,
      "status": null,
      "error": "connection refused (http://localhost:9999)",
      "assertions": []
    }
  ]
}
```

For data-driven runs, a top-level `"iterations"` array wraps the per-row results:

```json
{
  "passed": 1,
  "failed": 1,
  "iterations": [
    {
      "row_index": 0,
      "row": { "email": "alice@example.com" },
      "passed": true,
      "requests": [...]
    }
  ]
}
```

#### JUnit XML

Standard JUnit format compatible with Jenkins, GitHub Actions test reporters, and GitLab CI:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="My API Suite" tests="3" failures="1" errors="0">
    <testcase name="Create user"/>
    <testcase name="Authenticate"/>
    <testcase name="Protected page">
      <failure message="status expected 200, got 401"/>
    </testcase>
  </testsuite>
</testsuites>
```

Network errors produce `<error>` elements (distinct from `<failure>`):

```xml
<testcase name="Flaky endpoint">
  <error message="connection refused (http://localhost:9999)"/>
</testcase>
```

Data-driven runs produce one `<testsuite>` per iteration, named `"<collection> row <index>"`.

---

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | All requests passed — all assertions satisfied |
| `1` | One or more test failures — assertion failed or network error during a request |
| `2` | Pre-run error — malformed collection file, unreadable data file, unsupported file extension, invalid `--concurrency` value, file write error |

The distinction between `1` and `2` is intentional: a `1` means the test ran but something failed; a `2` means the test could not start at all. This makes it straightforward to distinguish flaky infrastructure from genuine test failures in CI scripts.

```bash
strex run api.yaml
case $? in
  0) echo "All good" ;;
  1) echo "Tests failed — check output" ;;
  2) echo "Could not run — check collection file" ;;
esac
```
