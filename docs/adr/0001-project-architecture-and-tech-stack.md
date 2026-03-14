# ADR-0001: Project Architecture and Tech Stack

**Status:** Accepted  
**Date:** 2026-03-14  
**Decision Makers:** Core Team  

---

## Context

Postman has moved Collection Runner and Performance Testing features to paid tiers, creating a gap in the market for an open-source, CLI-first, file-based API testing tool. We need to build **Strex** (stress + execution) as a viable alternative that:

- Prioritizes CLI workflow over GUI
- Uses Git-friendly file formats (no proprietary lock-in)
- Works offline by default
- Provides integrated performance testing without paywalls
- Targets backend developers, DevOps engineers, and performance engineers
- Supports data-driven testing (CSV/JSON batch jobs)

---

## Decision

### 1. **Core Architecture: All-Rust Stack**

**Decision:** Build entire core engine in Rust with embedded JavaScript runtime.

**Structure:**
```
strex/
├── crates/
│   ├── strex-core/          # HTTP client, parser, runner orchestration
│   ├── strex-script/        # JS/TS runtime (rquickjs)
│   └── strex-cli/           # CLI interface + TUI
├── docs/
│   └── adr/                 # Architecture Decision Records
├── formats/
│   └── collection.schema.json
└── examples/
    └── github-api/
```

**Rationale:**
- **Single binary deployment** — no IPC overhead, simpler packaging
- **Rust performance + safety** — async I/O (Tokio), memory safety, zero-cost abstractions
- **Tokio handles 1000+ concurrent requests** — sufficient for single-machine load testing
- **Simplified toolchain** — one language, one build system (Cargo)

**Rejected Alternative:** Rust + Go hybrid
- Go performance engine via gRPC would add deployment complexity
- IPC latency overhead
- Two separate toolchains to maintain
- Tokio async proven sufficient for HTTP load generation

---

### 2. **JavaScript Runtime: rquickjs**

**Decision:** Embed QuickJS via `rquickjs` for scripting engine.

**Rationale:**
- ✅ **Lightweight** (~1MB runtime vs ~50MB for Deno)
- ✅ **Rust-native bindings** — stable API, easy embedding
- ✅ **ES6+ support** — modern JavaScript without TypeScript complexity
- ✅ **Sandboxed** — no file I/O or network by default (security)
- ✅ **Fast enough** — script execution not bottleneck (HTTP I/O is)

**Rejected Alternatives:**

| Option | Why Rejected |
|--------|--------------|
| `deno_core` | Unstable API, heavy runtime, unnecessary complexity for MVP |
| `boa` | Pure Rust but slower performance, less mature |
| No scripting | Limits flexibility — assertions and variable extraction need logic |

---

### 3. **Configuration Format: YAML (Git-Friendly)**

**Decision:** YAML as primary collection format with strict validation by default.

**Example:**
```yaml
name: "GitHub API Tests"
version: "1.0"

variables:
  baseUrl: "https://api.github.com"
  
requests:
  - name: "Get User"
    method: GET
    url: "{{baseUrl}}/users/{{username}}"
    script: |
      const data = response.json();
      variables.set("userId", data.id);
    assertions:
      - status: 200
      - jsonPath: "$.login"
        exists: true
```

**Validation Mode:**
- **Default: Strict** — reject unknown fields, catch typos early
- **Optional: Permissive** (`--loose` flag) — forward compatibility

**Rationale:**
- Human-readable and editable
- Clean diffs in Git (vs JSON single-line blobs)
- Native support via `serde_yaml`
- JSON Schema validation for IDE autocompletion

**Rejected Alternative:** JSON
- Less human-friendly for manual editing
- Comments not supported in standard JSON
- Postman uses JSON but exports are machine-generated

---

### 4. **Scripting API Design: Minimal but Extensible**

**Decision:** Start with minimal custom API, add Postman compatibility layer later.

**MVP API:**
```javascript
// Available in scripts
const response = {
  status: number,
  headers: object,
  body: string,
  json(): object,
  text(): string,
};

const variables = {
  get(key: string): string | undefined,
  set(key: string, value: string): void,
};

const env = {
  get(key: string): string | undefined,
};

// Assertions
function assert(condition: boolean, message?: string): void;
function assertEqual(actual, expected, message?): void;
```

**Rationale:**
- **Simplicity first** — Postman `pm.*` API has 40+ methods (scope creep risk)
- **Custom API is cleaner** — no legacy baggage
- **Conversion tool later** — `strex convert` can transform `pm.*` calls
- **Extensibility** — can add `pm` compatibility layer in v0.3 without breaking changes

**Future Compatibility Layer (v0.3):**
```javascript
// Optional compatibility mode
const pm = {
  response,
  variables,
  environment: env,
  test(name, fn) { /* wrapper around assert */ },
};
```

---

### 5. **Assertions: Declarative + Scripted (Dual Mode)**

**Decision:** Support both declarative YAML assertions and script-based assertions.

**Declarative (80% of use cases):**
```yaml
assertions:
  - status: 200
  - jsonPath: "$.data.token"
    exists: true
  - jsonPath: "$.data.email"
    equals: "{{expected_email}}"
  - header: "Content-Type"
    contains: "application/json"
```

**Scripted (complex logic):**
```javascript
const data = response.json();
assert(data.items.length > 0, "Must have items");
assert(data.items.every(i => i.price > 0), "Prices must be positive");
```

**Rationale:**
- Declarative covers common cases without scripting knowledge
- Scripts handle edge cases (array validation, computed checks)
- Both use same underlying validation engine
- Clear separation improves readability

---

### 6. **Data-Driven Testing: CSV/JSON with Continue-on-Error**

**Decision:** Support CSV/JSON iteration with configurable error handling.

**Usage:**
```bash
strex run collection.yaml --data users.csv
strex run collection.yaml --data users.csv --fail-fast
```

**Behavior:**
- **Default: Continue** — run all rows, report failures at end
- **`--fail-fast`** — stop on first failure

**Data Access:**
```yaml
body:
  content:
    email: "{{data.email}}"    # Current CSV row
    name: "{{data.name}}"
```

**Rationale:**
- Continue-on-error better for CI/CD (get full failure report)
- Fail-fast useful for development (quick feedback)
- CSV/JSON are universal, no custom format needed

**Deferred to v0.2:**
- Filters (`status === 'active'`) — requires JS eval, security concerns
- Shuffle mode — non-critical for MVP

---

### 7. **HTTP Stack: reqwest + Tokio (HTTP/1.1 Only for MVP)**

**Decision:** Use `reqwest` async client with Tokio runtime, HTTP/1.1 only.

**Tech Stack:**

| Component | Library | Version |
|-----------|---------|---------|
| HTTP Client | `reqwest` | 0.11+ |
| Async Runtime | `tokio` | 1.x |
| TLS | `rustls` | via reqwest |
| JSON | `serde_json` | 1.x |

**Rationale:**
- `reqwest` is production-ready, widely used
- Tokio is ecosystem standard
- HTTP/1.1 covers 95% of APIs
- Can add HTTP/2, HTTP/3, WebSocket in v0.3

**Deferred Features:**
- HTTP/2, HTTP/3 — not critical, adds complexity
- WebSocket, gRPC — different protocol layer
- Custom TLS certs — enterprise feature (v0.3)

---

### 8. **CLI Output: Multi-Format with Strict Defaults**

**Decision:** Support console, JSON, and JUnit XML output formats.

**Formats:**
```bash
# Pretty console (default)
strex run collection.yaml

# JSON for parsing
strex run collection.yaml --output report.json

# JUnit XML for CI/CD
strex run collection.yaml --output report.xml --format junit

# Verbose mode
strex run collection.yaml --verbose
```

**Exit Codes:**
- `0` — all tests passed
- `1` — test failures
- `2` — runtime error (invalid collection, network failure)

**Rationale:**
- Console for human readability
- JSON for scripting/automation
- JUnit XML for Jenkins, GitLab CI, GitHub Actions
- Clear exit codes for CI/CD integration

---

### 9. **Performance Testing: Deferred to v0.2**

**Decision:** Exclude load testing from MVP, implement in v0.2.

**Rationale:**
- MVP focuses on functional testing (core value)
- Performance testing adds significant complexity:
  - Metrics collection (latency histograms, percentiles)
  - Real-time TUI
  - Threshold evaluation
  - HTML report generation
- Tokio + semaphore pattern proven sufficient (no need for Go)

**v0.2 Design (preview):**
```yaml
performance:
  scenarios:
    - name: "Load Test API"
      requests: ["Get User", "Update Profile"]
      vus: 100              # Virtual users
      duration: 60s
      rampUp: 10s
      thresholds:
        - metric: http_req_duration
          percentile: 95
          max: 500ms
```

**Implementation:**
```rust
// Pseudo-code
let semaphore = Arc::new(Semaphore::new(vus));
for _ in 0..total_iterations {
    let permit = semaphore.clone().acquire_owned().await;
    tokio::spawn(async move {
        let _permit = permit;
        execute_request(...).await;
    });
}
```

---

### 10. **Parallelism Model: Bounded Concurrency for Data-Driven**

**Decision:** Requests within a collection execute sequentially, but data-driven iterations (CSV rows) can run with bounded parallelism.

**Default Behavior:**
- Requests within one iteration: **Sequential** (preserves flow: login → get token → use token)
- Iterations (CSV rows): **Sequential by default** (`--concurrency 1`)

**Bounded Concurrency (MVP):**
```bash
# Default: sequential (one row at a time)
strex run collection.yaml --data users.csv

# Run 10 CSV rows concurrently
strex run collection.yaml --data users.csv --concurrency 10

# Run all rows concurrently (use with caution)
strex run collection.yaml --data users.csv --concurrency 0  # 0 = unlimited
```

**Rationale:**
- **Data-driven batch jobs** are a stated use case — 1000 CSV rows sequentially is too slow for CI/CD
- **Sequential default** is safe for beginners (no race conditions)
- **Bounded concurrency** balances throughput and resource usage
- **Requests remain sequential** within each iteration (preserves dependencies)

**Variable Isolation (Critical):**
- Each iteration gets **isolated variable context** (no cross-iteration leakage)
- Environment variables are **read-only** (safe to share)
- See ADR-0002 for detailed scoping rules

**Implementation:**
```rust
// Pseudo-code (from ADR-0002)
let semaphore = Arc::new(Semaphore::new(concurrency));
for row in csv_data {
    let permit = semaphore.clone().acquire_owned().await;
    tokio::spawn(async move {
        let _permit = permit; // Released on drop
        let mut context = ExecutionContext::new_isolated(row);
        execute_collection(&collection, &mut context).await
    });
}
```

**Future (v0.3):**
- Request-level parallelism: `parallel: true` in YAML for independent requests
- Intelligent dependency detection (automatic parallel execution when safe)

---

## Consequences

### Positive

1. **Single Binary** — easy distribution via Homebrew, Cargo, GitHub Releases
2. **Fast Execution** — Rust + Tokio async provides excellent performance
3. **Safe & Reliable** — Rust's type system prevents entire classes of bugs
4. **Git-Friendly** — YAML collections track cleanly in version control
5. **Incremental Complexity** — MVP is simple, advanced features come later

### Negative

1. **Postman Incompatibility** — users need to convert scripts (mitigated by v0.3 tool)
2. **No TypeScript in Scripts** — only JavaScript (acceptable for MVP)
3. **Learning Curve** — new syntax vs Postman (mitigated by similar concepts)

### Risks

1. **rquickjs Stability** — less mature than Deno (mitigated: can swap runtime if needed)
2. **Ecosystem Adoption** — need marketing + documentation to compete with Postman
3. **Feature Parity Expectations** — users may expect all Postman features (manage via roadmap communication)

---

## MVP Scope Summary

**Included:**
- ✅ YAML collection parser + validation (strict subset - see ADR-0003)
- ✅ HTTP client (GET, POST, PUT, DELETE, PATCH)
- ✅ Variable interpolation (`{{var}}`)
- ✅ Environment files
- ✅ Scripting (minimal API: `response`, `variables`, `assert` - see ADR-0004)
- ✅ Script safety (timeouts, memory limits, worker threads - see ADR-0004)
- ✅ Declarative assertions (status, jsonPath, headers)
- ✅ Data-driven testing (CSV/JSON with `--concurrency N`)
- ✅ CLI: `run`, `validate`
- ✅ Output: console, JSON, JUnit XML
- ✅ Structured error taxonomy (see ADR-0002)

**Excluded (future versions):**
- ❌ Performance testing → v0.2
- ❌ Postman compatibility → v0.3
- ❌ HTTP/2, WebSocket, gRPC → v0.3
- ❌ GUI (Tauri) → v1.0
- ❌ Cloud sync → v1.0+

---

## References

- [reqwest documentation](https://docs.rs/reqwest/)
- [rquickjs documentation](https://docs.rs/rquickjs/)
- [Tokio async runtime](https://tokio.rs/)
- [YAML 1.2 Spec](https://yaml.org/spec/1.2/spec.html)
- [JUnit XML Format](https://llg.cubic.org/docs/junit/)

---

## Related Decisions

- ADR-0002: Execution Model and Error Taxonomy
- ADR-0003: Strex YAML Subset Definition
- ADR-0004: Script Safety Model
