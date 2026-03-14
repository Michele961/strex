# ADR-0004: Script Safety Model

**Status:** Accepted  
**Date:** 2026-03-14  
**Supersedes:** None  
**Related:** ADR-0001 (Architecture - rquickjs choice), ADR-0002 (Execution Model)

---

## Context

Strex embeds JavaScript execution (via rquickjs/QuickJS) to enable:
- Dynamic variable extraction from responses
- Custom assertions beyond declarative YAML
- Pre/post-request logic (token refresh, request chaining)

**Security and reliability risks:**
- **Infinite loops** — `while(true) {}` hangs CI jobs
- **Memory bombs** — `let a = []; while(true) a.push(new Array(1000000))` crashes process
- **CPU starvation** — CPU-bound scripts block Tokio async runtime
- **Malicious code** — users may run untrusted collections (import from internet, team sharing)

Without safety controls, a single bad script can:
1. Hang CI/CD pipelines indefinitely
2. Consume all system memory
3. Block concurrent HTTP requests (if scripts run on async runtime threads)

This ADR defines:
1. **Execution isolation** (worker thread model)
2. **Resource limits** (CPU time, memory, stack)
3. **API restrictions** (what scripts can/cannot do)
4. **Error handling** (how failures propagate)

---

## Decision

### 1. Architecture: Worker Thread Isolation

Scripts **MUST NOT** run on Tokio's async runtime threads. Instead, use a **dedicated worker thread pool**.

```
┌─────────────────────────────────────────────────────────┐
│ Main Thread (Tokio Async Runtime)                      │
│ - HTTP requests (reqwest)                              │
│ - Collection orchestration                             │
│ - Result aggregation                                   │
└─────────────────────────────────────────────────────────┘
                         │
                         │ Send script + context
                         ▼
┌─────────────────────────────────────────────────────────┐
│ Worker Thread Pool (blocking threads)                  │
│ - QuickJS runtime (CPU-bound)                          │
│ - Script execution                                     │
│ - Memory/timeout enforcement                           │
└─────────────────────────────────────────────────────────┘
                         │
                         │ Return result or timeout
                         ▼
┌─────────────────────────────────────────────────────────┐
│ Main Thread (continues async work)                     │
└─────────────────────────────────────────────────────────┘
```

**Implementation Pattern (Tokio):**

```rust
use tokio::task;
use rquickjs::{Runtime, Context};
use std::time::Duration;

async fn execute_script_safe(
    script: &str,
    response: &HttpResponse,
    variables: &mut HashMap<String, String>,
) -> Result<(), ScriptError> {
    let script = script.to_string();
    let response = response.clone();
    let variables_clone = variables.clone();
    
    // Execute on blocking thread pool (not async runtime)
    let handle = task::spawn_blocking(move || {
        execute_script_blocking(&script, &response, variables_clone)
    });
    
    // Enforce timeout at Tokio level (30s default)
    match tokio::time::timeout(Duration::from_secs(30), handle).await {
        Ok(Ok(result)) => {
            // Merge variables back
            *variables = result.variables;
            Ok(())
        }
        Ok(Err(e)) => Err(e), // Script error
        Err(_) => Err(ScriptError::Timeout(30)), // Timeout
    }
}

fn execute_script_blocking(
    script: &str,
    response: &HttpResponse,
    mut variables: HashMap<String, String>,
) -> Result<ScriptResult, ScriptError> {
    // Create isolated QuickJS runtime
    let mut runtime = Runtime::new()?;
    
    // Set memory limit (64MB default)
    runtime.set_memory_limit(64 * 1024 * 1024);
    
    // Set interrupt handler (for infinite loops)
    runtime.set_interrupt_handler(Some(Box::new(|| {
        // Called periodically during execution
        // Return true to interrupt (e.g., if timeout exceeded)
        false // Timeout handled at Tokio level
    })));
    
    let context = Context::full(&runtime)?;
    
    context.with(|ctx| {
        // Inject API (response, variables)
        inject_api(ctx, response, &variables)?;
        
        // Execute script
        ctx.eval(script)?;
        
        // Extract modified variables
        let updated_vars = extract_variables(ctx)?;
        
        Ok(ScriptResult {
            variables: updated_vars,
        })
    })
}
```

**Why Worker Threads:**
- ✅ Isolates CPU-bound JS from I/O-bound HTTP requests
- ✅ Tokio remains responsive even if script is slow
- ✅ Can enforce hard timeout via `tokio::time::timeout`
- ✅ No risk of blocking async tasks

---

### 2. Resource Limits (Enforced by Default)

| Resource | Default Limit | Configurable Via | Rationale |
|----------|---------------|------------------|-----------|
| **CPU Time** | 30 seconds | `--script-timeout 60` | Prevent infinite loops |
| **Memory** | 64 MB | `--script-memory 128` | Prevent memory bombs |
| **Stack Depth** | 256 frames | (QuickJS default) | Prevent stack overflow |
| **Network Calls** | ❌ Forbidden | N/A | Scripts cannot do HTTP (use request chaining) |
| **File I/O** | ❌ Forbidden | N/A | Scripts cannot read/write files |
| **Process Spawn** | ❌ Forbidden | N/A | No shell execution |

#### Timeout Enforcement (Dual-Layer):

```
Layer 1: Tokio Timeout (Hard Kill)
  - tokio::time::timeout(30s, spawn_blocking(...))
  - Kills worker thread if not done
  - Always enforced

Layer 2: QuickJS Interrupt Handler (Graceful Stop)
  - runtime.set_interrupt_handler(...)
  - Polls periodically during script execution
  - Returns error instead of hard kill
  - Allows cleanup/stack trace capture
```

**Example Timeout Error:**
```
❌ Script Timeout: Login (Pre-Request)

Script exceeded 30s time limit.

  while (true) {
    // Infinite loop detected
  }

→ Check for infinite loops or increase timeout:
  strex run collection.yaml --script-timeout 60
```

#### Memory Limit Enforcement:

```rust
runtime.set_memory_limit(64 * 1024 * 1024); // 64MB

// QuickJS will throw error when limit exceeded:
// "out of memory"
```

**Example Memory Error:**
```
❌ Script Memory Limit: Parse Response (Post-Request)

Script exceeded 64MB memory limit.

  let data = new Array(10000000); // Too large

→ Reduce memory usage or increase limit:
  strex run collection.yaml --script-memory 128
```

---

### 3. Script API (Restricted Subset)

Scripts have access to a **minimal, safe API** — no filesystem, no network, no system calls.

#### Available APIs:

```javascript
// === Response Object (Read-Only) ===
const response = {
  status: number,              // HTTP status code
  statusText: string,          // "OK", "Not Found", etc.
  headers: { [key: string]: string },
  body: string,                // Raw response body
  
  // Parsed body (throws if not valid)
  json(): object,
  text(): string,
  
  // Timing info (milliseconds)
  timing: {
    total: number,
    dns: number,
    connect: number,
    tls: number,
    send: number,
    wait: number,
    receive: number,
  }
};

// === Variables Object (Mutable) ===
const variables = {
  get(key: string): string | undefined,
  set(key: string, value: string): void,
  has(key: string): boolean,
  delete(key: string): void,
  clear(): void,
  keys(): string[],
};

// === Environment Object (Read-Only) ===
const env = {
  get(key: string): string | undefined,
  has(key: string): boolean,
  keys(): string[],
};

// === Data Object (Read-Only, from CSV/JSON) ===
const data = {
  get(key: string): string | undefined,
  has(key: string): boolean,
  keys(): string[],
};

// === Assertion Functions ===
function assert(condition: boolean, message?: string): void;
function assertEqual(actual: any, expected: any, message?: string): void;
function assertNotEqual(actual: any, expected: any, message?: string): void;
function assertContains(haystack: string, needle: string, message?: string): void;
function assertMatch(text: string, regex: RegExp, message?: string): void;

// === Logging (appears in verbose output) ===
const console = {
  log(...args: any[]): void,
  warn(...args: any[]): void,
  error(...args: any[]): void,
};
```

#### Forbidden APIs (Throw Error):

```javascript
// ❌ Network (use request chaining instead)
fetch("https://example.com");  // Error: fetch is not defined
XMLHttpRequest();               // Error: XMLHttpRequest is not defined

// ❌ File I/O
require("fs");                  // Error: require is not defined
import * as fs from "fs";       // Error: import not supported

// ❌ Process/System
process.exit(1);                // Error: process is not defined
eval("code");                   // Error: eval is disabled (security)
Function("code")();             // Error: Function constructor disabled

// ❌ Timers (could delay execution)
setTimeout(() => {}, 1000);     // Error: setTimeout is not defined
setInterval(() => {}, 1000);    // Error: setInterval is not defined

// ❌ Async (scripts are synchronous)
await fetch(...);               // SyntaxError: await not allowed
async function test() {}        // SyntaxError: async not allowed
```

---

### 4. Sandboxing Strategy (Defense in Depth)

**Layer 1: API Restriction**
- Only inject safe APIs (response, variables, assert)
- Do NOT expose Node.js globals (require, process, Buffer)

**Layer 2: QuickJS Sandbox**
- No file system access (QuickJS has no built-in fs module)
- No network access (QuickJS has no built-in http module)
- No native modules (no FFI)

**Layer 3: Resource Limits**
- Memory limit (64MB)
- CPU time limit (30s)
- Stack depth limit (256 frames)

**Layer 4: Process Isolation (Future)**
- Consider running scripts in separate process for maximum isolation
- Use IPC (stdin/stdout) for communication
- Kill process on timeout (no graceful shutdown needed)

**Current MVP:** Layers 1-3 sufficient (worker threads + resource limits).

---

### 5. Error Handling (Script Failures)

#### Error Types:

```rust
pub enum ScriptError {
    // Compilation errors (syntax)
    CompilationError {
        line: usize,
        column: usize,
        message: String,
    },
    
    // Runtime errors (exceptions)
    RuntimeError {
        message: String,
        stack: Option<String>,
    },
    
    // Resource limits
    Timeout {
        limit_seconds: u64,
    },
    MemoryLimit {
        limit_mb: u64,
    },
    
    // Assertion failures (from assert())
    AssertionFailed {
        message: String,
    },
}
```

#### Error Propagation (from ADR-0002):

| Script Type | On Error | Request Continues? | Collection Continues? |
|-------------|----------|--------------------|-----------------------|
| **Pre-Request** | ❌ Stop request | No | Yes (mark request FAILED) |
| **Post-Request** | ⚠️ Mark FAILED | Yes (assertions still run) | Yes |

#### CLI Flags:

```bash
# Default: stop request on script error
strex run collection.yaml

# Continue request even if script fails (for debugging)
strex run collection.yaml --continue-on-script-error

# Increase timeout for slow scripts
strex run collection.yaml --script-timeout 60

# Increase memory limit
strex run collection.yaml --script-memory 128
```

---

### 6. Script Examples (Safe Patterns)

#### Extract Token from Response:

```javascript
// Post-Request Script
const data = response.json();
if (data.token) {
  variables.set("token", data.token);
  console.log("Token extracted:", data.token);
} else {
  throw new Error("No token in response");
}
```

#### Conditional Assertions:

```javascript
// Post-Request Script
const data = response.json();
if (data.status === "active") {
  assert(data.lastLogin !== null, "Active users must have lastLogin");
} else {
  console.log("User inactive, skipping lastLogin check");
}
```

#### Parse CSV-Like Response:

```javascript
// Post-Request Script
const lines = response.text().split("\n");
const values = lines[1].split(","); // Second line
variables.set("userId", values[0]);
variables.set("userName", values[1]);
```

#### Dynamic Assertion Based on Data:

```javascript
// Post-Request Script (with --data users.csv)
const expectedStatus = parseInt(data.get("expected_status"));
assertEqual(response.status, expectedStatus, 
  `Expected status ${expectedStatus} for user ${data.get("email")}`);
```

---

## Consequences

### Positive

1. **Safety** — scripts cannot hang CI jobs, crash process, or access files/network
2. **Predictability** — resource limits documented and enforced
3. **Debuggability** — clear error messages with line numbers and stack traces
4. **Performance** — scripts don't block async HTTP requests
5. **Security** — sandboxed execution prevents malicious code

### Negative

1. **Limitations** — no async/await, no network calls in scripts (must use request chaining)
2. **Complexity** — worker thread model adds implementation complexity
3. **Memory Overhead** — each script execution spawns blocking task (mitigated by thread pool)

### Risks

1. **QuickJS Vulnerabilities** — QuickJS has had CVEs (use-after-free, buffer overflow); must stay updated
2. **Resource Limit Bypass** — if QuickJS has bugs, limits might be bypassable
3. **Worker Thread Starvation** — if many scripts run concurrently, thread pool may exhaust

---

## Implementation Notes

### Worker Thread Pool Sizing:

```rust
// Configure based on CPU cores
let worker_threads = num_cpus::get(); // Default: # of CPU cores

// Create blocking thread pool
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(worker_threads)
    .max_blocking_threads(worker_threads * 2) // Allow more blocking tasks
    .build()?;
```

### Script Injection Pattern:

```rust
fn inject_api(
    ctx: &Context,
    response: &HttpResponse,
    variables: &HashMap<String, String>,
) -> Result<()> {
    ctx.with(|ctx| {
        let globals = ctx.globals();
        
        // Inject response object
        let response_obj = Object::new(ctx)?;
        response_obj.set("status", response.status)?;
        response_obj.set("body", response.body.clone())?;
        // ... more fields
        globals.set("response", response_obj)?;
        
        // Inject variables object
        let variables_obj = create_variables_proxy(ctx, variables)?;
        globals.set("variables", variables_obj)?;
        
        // Inject assert function
        let assert_fn = Function::new(ctx, |condition: bool, message: Option<String>| {
            if !condition {
                Err(rquickjs::Error::new_from_js("assertion", message.unwrap_or("Assertion failed".into())))
            } else {
                Ok(())
            }
        })?;
        globals.set("assert", assert_fn)?;
        
        Ok(())
    })
}
```

### Timeout Enforcement Example:

```rust
async fn execute_with_timeout(script: &str) -> Result<(), ScriptError> {
    let handle = tokio::task::spawn_blocking(move || {
        // Long-running script
        execute_script_blocking(script)
    });
    
    // Hard timeout: kill thread if exceeds 30s
    match tokio::time::timeout(Duration::from_secs(30), handle).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(e)) => Err(ScriptError::RuntimeError(e)),
        Err(_) => {
            // Timeout: thread is abandoned (will be cleaned up by runtime)
            Err(ScriptError::Timeout(30))
        }
    }
}
```

---

## Security Considerations

### 1. Untrusted Collections

**Threat Model:**
- User imports collection from internet (GitHub, shared link)
- Collection contains malicious script

**Mitigations:**
- ✅ Sandboxed API (no file/network access)
- ✅ Resource limits (can't DOS)
- ⚠️ User should review scripts before running (like code review)

**Future:** Add `--trust-level` flag:
```bash
# Paranoid mode: disable all scripting
strex run collection.yaml --trust-level none

# Default: sandboxed scripts
strex run collection.yaml --trust-level sandboxed

# Dangerous: allow file I/O (for advanced users)
strex run collection.yaml --trust-level full  # NOT MVP
```

### 2. Supply Chain Attacks

**Threat:** QuickJS itself has vulnerabilities.

**Mitigations:**
- ✅ Use QuickJS-NG fork (actively maintained)
- ✅ Monitor CVE databases
- ✅ Update rquickjs regularly
- ⚠️ Consider process isolation (v0.3) for maximum safety

### 3. Data Leakage

**Threat:** Scripts log sensitive data (tokens, passwords).

**Mitigations:**
- ✅ Console output only visible with `--verbose`
- ✅ Sanitize logs (mask tokens in output)
- ⚠️ Document best practices (don't log secrets)

---

## Performance Considerations

### Benchmark Targets (MVP):

| Metric | Target | Rationale |
|--------|--------|-----------|
| Script compilation | < 1ms | Negligible overhead |
| Script execution (simple) | < 10ms | Variable extraction is fast |
| Script execution (complex) | < 100ms | JSON parsing, loops |
| Context creation | < 1ms | Per-request overhead |
| Worker thread spawn | < 5ms | Tokio blocking pool |

### When Scripts Become Bottleneck:

If scripts are slow (> 100ms average), consider:
1. ✅ Optimize script code (avoid unnecessary loops)
2. ✅ Increase `--concurrency` (more parallel iterations)
3. ⚠️ Cache QuickJS contexts (reuse across requests)
4. ⚠️ JIT compilation (future: use V8 instead of QuickJS)

---

## Alternatives Considered

### Alternative 1: V8/Deno Runtime

**Pros:**
- Faster execution (JIT compilation)
- Better async support
- TypeScript native

**Cons:**
- Much larger binary (~50MB vs ~1MB)
- More complex embedding (deno_core API)
- Slower startup time

**Decision:** Start with QuickJS for MVP, consider V8 for v0.3+ if performance critical.

### Alternative 2: Lua Scripting

**Pros:**
- Simpler to embed
- Faster than QuickJS
- Smaller runtime

**Cons:**
- Less familiar to web developers
- Different syntax (breaks Postman compatibility goal)
- Smaller ecosystem

**Decision:** JavaScript is more familiar to target users.

### Alternative 3: No Scripting (Declarative Only)

**Pros:**
- Simplest, safest
- No runtime overhead

**Cons:**
- Cannot extract tokens dynamically
- Cannot handle complex response parsing
- Limits use cases significantly

**Decision:** Scripting is essential for real-world API testing.

### Alternative 4: Process Isolation (Separate Process per Script)

**Pros:**
- Maximum security (kill process = guaranteed cleanup)
- No risk of QuickJS bugs affecting main process

**Cons:**
- Higher overhead (process spawn ~10-50ms)
- IPC complexity (stdin/stdout communication)

**Decision:** Worker threads sufficient for MVP, consider process isolation for v0.3+.

---

## Future Enhancements (Post-MVP)

### 1. Async/Await Support (v0.3)

**Use Case:** Chaining requests inside scripts.

```javascript
// Post-Request Script (future)
const userData = response.json();
const profileResponse = await fetch(`/api/profile/${userData.id}`);
variables.set("profileData", profileResponse.json());
```

**Challenge:** Requires async runtime (QuickJS doesn't support async well).

### 2. TypeScript Support (v0.3)

**Use Case:** Type-safe scripts.

```typescript
interface User {
  id: number;
  email: string;
}

const user: User = response.json();
variables.set("userId", user.id.toString());
```

**Implementation:** Transpile TS → JS before execution (use swc or esbuild).

### 3. Script Debugging (v0.4)

**Use Case:** Step through script execution.

```bash
strex run collection.yaml --debug-script "Login"
# Opens debugger on localhost:9229 (Chrome DevTools Protocol)
```

**Implementation:** Use QuickJS debugger API or switch to V8.

---

## Related Decisions

- **ADR-0001**: Architecture — chose rquickjs as JS runtime
- **ADR-0002**: Execution Model — defines when scripts run in request lifecycle
- **ADR-0003**: YAML Subset — defines how scripts are declared in collections

---

## References

- [rquickjs Documentation](https://docs.rs/rquickjs/)
- [QuickJS-NG Security](https://github.com/quickjs-ng/quickjs)
- [Tokio Blocking Tasks](https://tokio.rs/tokio/tutorial/spawning)
- [AWS LLRT (QuickJS in Production)](https://github.com/awslabs/llrt)
- [Fusillade (QuickJS + Load Testing)](https://github.com/Fusillade-io/Fusillade)
