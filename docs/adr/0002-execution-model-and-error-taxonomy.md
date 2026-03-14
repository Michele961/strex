# ADR-0002: Execution Model and Error Taxonomy

**Status:** Accepted  
**Date:** 2026-03-14  
**Supersedes:** None  
**Related:** ADR-0001 (Project Architecture)

---

## Context

API testing tools must handle multiple execution contexts (requests, scripts, assertions) and report errors clearly for debugging. Without a well-defined execution model and error taxonomy, users face:

- **Ambiguous failures** — "request failed" doesn't indicate if it was DNS, TLS, timeout, or assertion failure
- **Variable leakage** — scripts modifying shared state cause flaky tests
- **Inconsistent behavior** — data-driven testing with unclear scope rules
- **Poor CI/CD integration** — exit codes and output formats that don't map to standard tooling

This ADR defines:
1. **Variable scope semantics** (environment vs collection vs data)
2. **Execution flow** (request lifecycle, script boundaries)
3. **Error taxonomy** (network, protocol, script, assertion, validation)
4. **Continue-on-error behavior** (when to stop vs keep going)

---

## Decision

### 1. Variable Scope Model (Three-Layer Hierarchy)

Strex uses a **three-layer variable system** with clear isolation rules:

```yaml
# Layer 1: Environment (immutable, global)
environment:
  baseUrl: "https://api.example.com"
  apiKey: "{{env.API_KEY}}"  # Can reference system env vars

# Layer 2: Collection Variables (mutable, per-iteration scope)
variables:
  userId: null
  token: null

# Layer 3: Data Variables (immutable, per-iteration)
# Loaded from CSV/JSON via --data flag
data:
  email: "user@example.com"
  password: "pass123"
```

#### Scope Rules:

| Variable Type | Mutability | Scope | Access Pattern |
|--------------|------------|-------|----------------|
| **Environment** | ❌ Immutable | Global (all iterations) | `{{env.baseUrl}}` |
| **Collection** | ✅ Mutable | Per-iteration (isolated) | `{{userId}}`, `variables.set("userId", ...)` |
| **Data** | ❌ Immutable | Per-iteration | `{{data.email}}` |
| **System Env** | ❌ Immutable | Global | `{{env.API_KEY}}` (reads from OS env) |

#### Isolation Guarantees:

**Sequential execution (`--concurrency 1`):**
- Collection variables reset to initial state at start of each iteration
- Scripts cannot affect variables in the next iteration

**Concurrent execution (`--concurrency N`):**
- Each iteration gets its own isolated variable context
- No shared mutable state between concurrent iterations
- Environment variables are safely read-only across all threads

**Example:**
```yaml
# collection.yaml
variables:
  counter: 0  # Reset to 0 for each CSV row

requests:
  - name: "Increment Counter"
    script: |
      const current = parseInt(variables.get("counter") || "0");
      variables.set("counter", (current + 1).toString());
      console.log(`Iteration counter: ${current + 1}`);
```

With `--data users.csv --concurrency 5`:
- 5 iterations run concurrently
- Each sees `counter = 0` initially
- No race conditions — each has isolated context

---

### 2. Execution Flow (Request Lifecycle)

Each request follows a **deterministic 7-phase lifecycle**:

```
┌─────────────────────────────────────────────────────────────┐
│ Phase 1: Template Interpolation                             │
│ - Resolve {{variables}} from environment/collection/data    │
│ - Evaluate expressions (if any, future feature)             │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Pre-Request Script (optional)                      │
│ - Execute script with current context                       │
│ - Can modify collection variables                           │
│ - Can access data variables (read-only)                     │
│ - Timeout enforced (30s default)                            │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 3: HTTP Request Execution                             │
│ - Send HTTP request via reqwest                             │
│ - Capture timing metrics                                    │
│ - Handle redirects (configurable)                           │
│ - Timeout enforced (60s default)                            │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 4: Response Capture                                   │
│ - Store status, headers, body                               │
│ - Parse JSON/XML if applicable                              │
│ - Make available to scripts/assertions                      │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 5: Post-Request Script (optional)                     │
│ - Execute script with response object                       │
│ - Can extract data: variables.set("token", response.json()) │
│ - Can perform custom assertions                             │
│ - Timeout enforced (30s default)                            │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 6: Declarative Assertions                             │
│ - Evaluate YAML assertions (status, jsonPath, headers)      │
│ - Report failures but continue to Phase 7                   │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 7: Result Recording                                   │
│ - Aggregate errors from all phases                          │
│ - Record timing, status, failures                           │
│ - Determine if request "passed" or "failed"                 │
└─────────────────────────────────────────────────────────────┘
```

#### Error Propagation Rules:

| Phase | On Error | Behavior |
|-------|----------|----------|
| 1. Template Interpolation | ❌ Stop request | Variable not found → SKIP request, mark FAILED |
| 2. Pre-Request Script | ⚠️ Configurable | Default: STOP request, mark FAILED. Flag: `--continue-on-script-error` |
| 3. HTTP Execution | ⚠️ Depends on error | Network errors → STOP. HTTP 4xx/5xx → CONTINUE to assertions |
| 4. Response Capture | ❌ Stop request | Body parse failure → CONTINUE (assertions can check raw body) |
| 5. Post-Request Script | ⚠️ Configurable | Default: MARK request FAILED, but continue to assertions |
| 6. Assertions | ✅ Always continue | Record failures, never stop execution |
| 7. Result Recording | ✅ Always happens | Aggregate all errors |

---

### 3. Error Taxonomy (Structured Error Types)

Strex uses a **hierarchical error taxonomy** for precise debugging:

```rust
// Conceptual Rust enum (not implementation)
pub enum StrexError {
    // === VALIDATION ERRORS (pre-execution) ===
    CollectionValidation {
        file: String,
        line: Option<usize>,
        field: String,
        reason: String,
        suggestion: Option<String>,
    },
    
    // === NETWORK ERRORS (Layer 4-7) ===
    DnsResolution {
        domain: String,
        error: String,
    },
    TlsHandshake {
        domain: String,
        error: String,
    },
    ConnectionRefused {
        url: String,
    },
    ConnectionTimeout {
        url: String,
        timeout_ms: u64,
    },
    
    // === HTTP PROTOCOL ERRORS ===
    HttpTimeout {
        url: String,
        timeout_ms: u64,
        phase: String, // "connect" | "headers" | "body"
    },
    TooManyRedirects {
        url: String,
        max_redirects: u32,
    },
    InvalidHttpResponse {
        url: String,
        error: String,
    },
    
    // === SCRIPTING ERRORS ===
    ScriptCompilation {
        request_name: String,
        line: usize,
        column: usize,
        error: String,
    },
    ScriptExecution {
        request_name: String,
        error: String,
        stack_trace: Option<String>,
    },
    ScriptTimeout {
        request_name: String,
        timeout_ms: u64,
    },
    ScriptMemoryLimit {
        request_name: String,
        limit_mb: u64,
    },
    
    // === ASSERTION ERRORS ===
    AssertionFailed {
        request_name: String,
        assertion_type: String, // "status" | "jsonPath" | "header" | "script"
        expected: String,
        actual: String,
        message: Option<String>,
    },
    JsonPathNotFound {
        request_name: String,
        path: String,
    },
    
    // === DATA-DRIVEN ERRORS ===
    DataFileNotFound {
        path: String,
    },
    DataParseError {
        path: String,
        line: Option<usize>,
        error: String,
    },
    
    // === TEMPLATE ERRORS ===
    VariableNotFound {
        request_name: String,
        variable: String,
        available: Vec<String>, // Suggestions
    },
}
```

#### Error Output Formats:

**Console (Human-Readable):**
```
❌ Request Failed: Get User Profile

  Network Error: DNS resolution failed
  Domain: api.example.com
  Cause: NXDOMAIN
  
  → Check if the domain exists or if you're offline

Duration: 234ms
```

**JSON (Machine-Readable):**
```json
{
  "request": "Get User Profile",
  "status": "failed",
  "error": {
    "type": "DnsResolution",
    "domain": "api.example.com",
    "message": "NXDOMAIN"
  },
  "duration_ms": 234
}
```

**JUnit XML (CI/CD):**
```xml
<testcase name="Get User Profile" time="0.234">
  <error type="DnsResolution" message="NXDOMAIN">
    Domain: api.example.com
  </error>
</testcase>
```

---

### 4. Continue-on-Error Behavior

Strex supports **fine-grained control** over failure handling:

#### Default Behavior (No Flags):

| Failure Type | Request-Level | Collection-Level | Data-Driven (CSV) |
|--------------|---------------|------------------|-------------------|
| Assertion failure | ✅ Continue to next request | ✅ Continue | ✅ Continue to next row |
| Script error | ❌ Stop request, ✅ continue collection | ✅ Continue | ✅ Continue to next row |
| Network error | ❌ Stop request, ✅ continue collection | ✅ Continue | ✅ Continue to next row |
| Template error | ❌ Skip request, ✅ continue collection | ✅ Continue | ✅ Continue to next row |

#### CLI Flags for Control:

```bash
# Stop on first failure (any type)
strex run collection.yaml --fail-fast

# Continue even on script errors (for debugging)
strex run collection.yaml --continue-on-script-error

# Stop if any assertion fails (strict mode)
strex run collection.yaml --fail-on-assertion

# Combination: stop on first assertion failure
strex run collection.yaml --fail-fast --fail-on-assertion
```

#### Exit Codes:

| Exit Code | Meaning |
|-----------|---------|
| `0` | All requests passed (no assertion failures, no errors) |
| `1` | One or more assertion failures (but execution completed) |
| `2` | Runtime error (network failure, script crash, invalid collection) |
| `3` | Invalid CLI arguments or missing files |

**CI/CD Usage:**
```bash
# Fail pipeline on any assertion failure
strex run tests.yaml --output report.xml --format junit
if [ $? -ne 0 ]; then
  echo "Tests failed"
  exit 1
fi
```

---

### 5. Data-Driven Execution Model

With `--data users.csv`:

```csv
email,password,expected_status
user1@example.com,pass123,200
user2@example.com,invalid,401
admin@example.com,admin123,200
```

**Execution semantics:**

```
For each row in CSV:
  1. Create isolated ExecutionContext
  2. Load row data into context.data (immutable)
  3. Initialize context.variables from collection (fresh copy)
  4. Initialize context.environment (shared, read-only)
  5. Execute all requests in collection sequentially
  6. Record iteration result (pass/fail)
  7. Discard context (no leakage to next iteration)

Concurrency (--concurrency N):
  - Semaphore limits N iterations running simultaneously
  - Each iteration fully isolated (no shared mutable state)
  - Results aggregated after all complete
```

**Variable access in templates:**
```yaml
requests:
  - name: "Login"
    body:
      content:
        email: "{{data.email}}"        # From current CSV row
        password: "{{data.password}}"  # From current CSV row
    assertions:
      - status: "{{data.expected_status}}"  # Dynamic assertion
```

---

## Consequences

### Positive

1. **Predictable Execution** — clear lifecycle, no hidden state mutations
2. **Debuggable Failures** — error taxonomy provides actionable information
3. **Isolated Iterations** — data-driven testing with no flaky cross-iteration bugs
4. **CI/CD Integration** — stable exit codes and output formats
5. **Flexible Error Handling** — continue-on-error flags for different workflows

### Negative

1. **Complexity** — three-layer variable system may confuse new users
2. **Performance Overhead** — context isolation per iteration adds memory/CPU cost
3. **Migration Path** — if Postman users expect different semantics, need conversion guide

### Risks

1. **Script Isolation** — enforcing memory/timeout limits requires robust runtime integration (see ADR-0004)
2. **Error Message Quality** — taxonomy only helps if messages are well-written (needs ongoing investment)
3. **Concurrency Edge Cases** — shared resources (file handles, external services) may cause issues despite variable isolation

---

## Implementation Notes

### Variable Resolution Order:

```rust
fn resolve_variable(name: &str, context: &ExecutionContext) -> Result<String> {
    // 1. Check data variables (current iteration)
    if let Some(value) = context.data.get(name) {
        return Ok(value.clone());
    }
    
    // 2. Check collection variables (mutable)
    if let Some(value) = context.variables.get(name) {
        return Ok(value.clone());
    }
    
    // 3. Check environment variables (immutable)
    if let Some(value) = context.environment.get(name) {
        return Ok(value.clone());
    }
    
    // 4. Check system environment (OS env vars)
    if let Ok(value) = std::env::var(name) {
        return Ok(value);
    }
    
    // 5. Not found
    Err(StrexError::VariableNotFound {
        variable: name.to_string(),
        available: context.list_all_variables(),
    })
}
```

### Context Isolation (Data-Driven):

```rust
async fn execute_data_driven(
    collection: &Collection,
    data: Vec<HashMap<String, String>>,
    concurrency: usize,
) -> Vec<IterationResult> {
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut handles = vec![];
    
    for (index, row) in data.into_iter().enumerate() {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let collection = collection.clone();
        
        let handle = tokio::spawn(async move {
            let _permit = permit; // Released on drop
            
            // Fresh context per iteration
            let mut context = ExecutionContext {
                environment: collection.environment.clone(), // Shared
                variables: collection.variables.clone(),    // Fresh copy
                data: row,                                   // Unique to this iteration
            };
            
            let result = execute_collection(&collection, &mut context).await;
            
            IterationResult {
                index,
                result,
            }
        });
        
        handles.push(handle);
    }
    
    // Await all iterations
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    
    results
}
```

---

## Alternatives Considered

### Alternative 1: Global Mutable Variables (Postman-like)

**Rejected because:**
- Causes flaky tests with `--concurrency N`
- Race conditions hard to debug
- Not thread-safe without complex locking

### Alternative 2: No Variable Mutation (Pure Functional)

**Rejected because:**
- Common workflow: login → extract token → use token
- Would require storing state externally (files, global store)
- Less intuitive for API testing use case

### Alternative 3: Single-Layer Variable System

**Rejected because:**
- Cannot distinguish between "configuration" (env) and "runtime state" (variables)
- Data-driven testing needs isolated per-iteration scope
- Less clear when parallelizing

---

## Related Decisions

- **ADR-0001**: Architecture — defines scripting runtime (rquickjs)
- **ADR-0003**: YAML Subset — defines how variables are declared in collections
- **ADR-0004**: Script Safety — defines timeout/memory enforcement for script execution

---

## References

- [Postman Variables Documentation](https://learning.postman.com/docs/sending-requests/variables/)
- [K6 Execution Context](https://k6.io/docs/using-k6/execution-context-variables/)
- [JUnit XML Format](https://llg.cubic.org/docs/junit/)
