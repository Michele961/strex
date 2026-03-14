# Strex Architecture Validation Report

**Date:** 2026-03-14  
**Validation Team:** Oracle (Technical Analysis) + Explorer (Competitive Intelligence) + Librarian (Production Evidence)  
**Status:** ⚠️ **APPROVED WITH CRITICAL MITIGATIONS REQUIRED**

---

## Executive Summary

The proposed Strex architecture (All-Rust + Tokio + rquickjs + YAML) is **fundamentally viable** for a functional testing MVP, with **real production evidence** supporting its feasibility. However, **3 critical risks must be addressed before implementation begins**, or they will become showstoppers.

### Verdict: **PROCEED WITH MANDATORY FIXES**

✅ **Go/No-Go Decision:** **GO** — but only after implementing the 7-point action plan below.

---

## Key Findings Summary

| Dimension | Finding | Risk Level |
|-----------|---------|------------|
| **Technical Viability** | Tokio can handle 1000+ concurrent requests; proven in production | ✅ Low |
| **Stack Validation** | Fusillade (135K RPS) uses identical stack; AWS LLRT uses rquickjs in prod | ✅ Low |
| **Competitive Position** | Bruno/HTTPie gaps validated; killer feature = Git + CI + data-driven | ✅ Strong |
| **rquickjs Risks** | API instability, thread-safety issues, no JIT (20-50x slower than V8) | ⚠️ High |
| **Sequential Execution** | Blocks data-driven CSV/JSON batch job performance | ⚠️ High |
| **YAML Complexity** | "Strict validation" is a trap without subset definition | 🔴 Critical |
| **Scripting Safety** | No DoS controls = CI jobs hang forever on bad scripts | 🔴 Critical |

---

## Critical Risks (MUST FIX BEFORE CODING)

### 🔴 **RISK 1: Scripting Can Stall the Entire Runtime**

**Problem:**
- QuickJS is **CPU-bound and synchronous**
- If scripts run on Tokio's async executor threads, they **block unrelated I/O tasks**
- rquickjs contexts are **NOT `Send`/`Sync`** — cannot be safely shared across threads
- **No timeout/interrupt mechanism by default** — infinite loops = hung CI jobs

**Evidence:**
- Oracle: "QuickJS execution is CPU-bound and synchronous; if scripts run on core async executor threads, they can block unrelated I/O tasks"
- Librarian: "As both Runtime and Context use a lock it is discouraged to use them in an async environment" (rquickjs docs)

**Impact:** **SHOWSTOPPER** — a single bad script crashes the entire test run.

**Mitigation (MANDATORY):**
```rust
// Pseudo-code architecture pattern
use tokio::task;
use std::time::Duration;

async fn execute_script_safely(script: &str, context: ScriptContext) -> Result<(), ScriptError> {
    // 1. Run script on dedicated blocking thread pool
    let handle = task::spawn_blocking(move || {
        // 2. QuickJS context isolated from async runtime
        let mut runtime = rquickjs::Runtime::new()?;
        runtime.set_memory_limit(64 * 1024 * 1024); // 64MB cap
        runtime.set_interrupt_handler(/* timeout handler */);
        
        // 3. Execute with timeout
        let result = runtime.execute(script)?;
        Ok(result)
    });
    
    // 4. Enforce timeout at Tokio level
    tokio::time::timeout(Duration::from_secs(30), handle).await??
}
```

**Action Items:**
1. ✅ Design worker-thread-per-script model (like Fusillade)
2. ✅ Implement memory limit (64MB default, configurable)
3. ✅ Implement timeout (30s default, configurable via `--script-timeout`)
4. ✅ Add interrupt handler for infinite loops
5. ✅ Document script safety model in ADR

---

### 🔴 **RISK 2: YAML "Strict Validation" Is Undefined**

**Problem:**
- YAML 1.2 allows **anchors, aliases, merges, tags, implicit typing**
- Different parsers handle edge cases differently
- "Strict validation" without a **defined subset** = user confusion + security risks

**Evidence:**
- Oracle: "YAML allows anchors/aliases/merges, tags, and implicit typing edge cases; different parsers handle edge cases differently"
- Oracle: "Strict must include policies like duplicate keys, forbidden constructs, maximum sizes, and consistent scalar typing"

**Impact:** **SHOWSTOPPER** — inconsistent behavior, potential DoS via alias expansion, schema drift.

**Mitigation (MANDATORY):**

Define **Strex YAML Subset** in ADR:

```yaml
# ✅ ALLOWED
name: "Test Collection"
variables:
  baseUrl: "https://api.example.com"
  
# ❌ FORBIDDEN
aliases: &default_headers  # No anchors/aliases
  Content-Type: application/json

tags: !!str "forced-type"  # No tags

merge: <<: *default_headers  # No merges
```

**Action Items:**
1. ✅ Document allowed YAML subset in ADR-0002 (create new ADR)
2. ✅ Policy on duplicate keys (reject by default)
3. ✅ Maximum file size (10MB default)
4. ✅ Forbidden constructs: anchors, aliases, merges, custom tags
5. ✅ Use `serde_yaml` in safe mode + custom validator
6. ✅ Add `--loose` flag for permissive mode (forward compatibility)

---

### 🔴 **RISK 3: Sequential Execution Breaks Data-Driven Use Case**

**Problem:**
- MVP spec says "sequential execution" for simplicity
- Target use case includes "data-driven batch jobs (CSV/JSON)"
- **1000 CSV rows × sequential = painfully slow** for CI/CD

**Evidence:**
- Oracle: "For 'data-driven batch jobs (CSV/JSON),' fully sequential can be painfully slow and limit real-world CI throughput"
- User stated use case: "runnare batch job partendo da una collection con logiche o prendendo dati da un csv"

**Impact:** **HIGH** — users will abandon tool if batch processing takes 10x longer than expected.

**Mitigation (MANDATORY FOR MVP):**

Add **bounded concurrency** for data-driven mode:

```bash
# Sequential by default (safe)
strex run collection.yaml --data users.csv

# Bounded parallelism for batch jobs
strex run collection.yaml --data users.csv --concurrency 10
```

**Design:**
- Requests within a single iteration remain **sequential** (preserve flow)
- Iterations (CSV rows) can run **concurrently** with semaphore limit
- Default: `--concurrency 1` (sequential)
- Max: `--concurrency N` where N = number of CSV rows

**Action Items:**
1. ✅ Add `--concurrency N` flag to MVP
2. ✅ Default to sequential (`--concurrency 1`)
3. ✅ Implement semaphore pattern for bounded parallelism
4. ✅ Document variable scope: per-iteration isolation vs shared env
5. ✅ Update ADR-0001 to reflect this change

---

## Major Concerns (SHOULD ADDRESS IN MVP)

### ⚠️ **CONCERN 1: Error Handling Taxonomy Undefined**

**Problem:**
Users need **actionable debugging** — not just "request failed."

**Required Error Taxonomy:**
```rust
enum StrexError {
    // Network layer
    DnsResolutionFailed { domain: String, cause: String },
    TlsHandshakeFailed { domain: String, cause: String },
    ConnectionTimeout { url: String, timeout: Duration },
    
    // Protocol layer
    HttpRequestFailed { status: u16, body: String },
    
    // Scripting layer
    ScriptExecutionFailed { line: usize, error: String },
    ScriptTimeout { timeout: Duration },
    
    // Assertion layer
    AssertionFailed { assertion: String, actual: String, expected: String },
    JsonPathNotFound { path: String },
    
    // Validation layer
    InvalidCollection { field: String, reason: String },
}
```

**Action Items:**
1. ✅ Define error taxonomy in ADR-0002
2. ✅ Map errors to stable JSON output format
3. ✅ Map errors to JUnit XML (failures vs errors)
4. ✅ Colorized console output with suggestions

---

### ⚠️ **CONCERN 2: Variable Scope Semantics Undefined**

**Problem:**
With data-driven testing + future parallelism, variable scope must be **explicit now** to avoid breaking changes.

**Required Semantics:**
```yaml
# Three variable layers
environment:       # Read-only, loaded from env.yaml
  token: "abc123"

variables:         # Mutable, shared across requests in ONE iteration
  userId: null

data:              # Per-iteration, read-only
  email: "user@example.com"
```

**Scoping Rules:**
- **Environment variables**: Immutable, global across all iterations
- **Collection variables**: Mutable, **isolated per iteration** (no cross-iteration leakage)
- **Data variables**: Immutable, per-iteration only

**Action Items:**
1. ✅ Document variable scope in ADR-0002
2. ✅ Implement isolated context per iteration
3. ✅ Add tests for cross-iteration isolation

---

### ⚠️ **CONCERN 3: reqwest Limitations for Performance Testing**

**Problem:**
reqwest is excellent for functional testing, but **hides metrics** needed for performance testing:
- No per-phase timing (DNS, TLS handshake, TTFB, download)
- No connection pool visibility
- No protocol-level control

**Evidence:**
- Oracle: "For API testing and future perf work you'll want: fine-grained timing hooks, connection/pool visibility, protocol control, DNS/TLS observability"
- Librarian: Fusillade uses custom HTTP stack for performance testing

**Mitigation:**
- ✅ Keep reqwest for MVP functional testing
- ⚠️ For v0.2 performance testing, evaluate:
  - Direct hyper usage (lower-level control)
  - Custom metrics middleware
  - Dual-path architecture (reqwest for functional, hyper for perf)

**Action Items:**
1. ✅ Document reqwest limitations in ADR-0001
2. ⚠️ Spike hyper metrics extraction before v0.2

---

## Production Evidence: The Stack Works

### 🎯 **Fusillade: Exact Stack Match**

**Repository:** [Fusillade-io/Fusillade](https://github.com/Fusillade-io/Fusillade)

**Performance (Single Machine):**
- ✅ **135,000+ RPS**
- ✅ **0.07ms P50 latency, 0.25ms P95**
- ✅ **3-5x lower tail latency than k6**
- ✅ Uses **QuickJS + Rust + Tokio**

**Architecture:**
- Worker-per-thread model (not async runtime)
- QuickJS contexts isolated on worker threads
- Supports HTTP/2, WebSocket, gRPC, MQTT

**Lesson:** The stack works at scale **IF** scripting is isolated from I/O runtime.

---

### 🎯 **AWS LLRT: rquickjs in Production**

**Use Case:** AWS serverless JavaScript runtime  
**Stars:** 11,000+  
**Evidence:** rquickjs is production-ready for **bounded execution** (serverless functions have time limits).

**Lesson:** rquickjs is viable **WITH** timeouts and memory limits.

---

### 🎯 **Tokio Performance**

**Hard Numbers:**
- ✅ **1.8M concurrent connections** ([Tokio scheduler study](https://techpreneurr.medium.com/how-to-achieve-1-8m-concurrent-connections-with-rust-inside-tokios-task-scheduler-122628ea1e28))
- ✅ **500K packets/second** ([dwarfhack study](https://dwarfhack.com/posts/tech/tokio_pps/))
- ✅ **3x faster than Go** for API workloads ([Rust vs Go benchmark](https://medium.com/@guvencanguven965/rust-vs-go-for-our-api-one-was-3x-faster-the-other-shipped-3x-faster-175be1d792dc))

**Lesson:** Tokio can absolutely handle 1000+ concurrent HTTP requests.

---

## Competitive Analysis: Market Validation

### **Postman's Weaknesses (Strex Opportunities)**

| Postman Flaw | Strex Advantage |
|--------------|-----------------|
| ❌ Cloud lock-in | ✅ Local-first, no cloud required |
| ❌ Proprietary format | ✅ Open YAML, Git-friendly |
| ❌ Pricing paywall | ✅ Open source (MIT/Apache 2.0) |
| ❌ Privacy concerns | ✅ Zero telemetry, offline by default |
| ❌ Performance testing = $$$$ | ✅ Built-in (v0.2) |

---

### **Bruno's Gap (Strex Opportunity)**

Bruno is Strex's **closest competitor** (file-based, Git-native):

| Bruno Limitation | Strex Differentiator |
|------------------|----------------------|
| ❌ GUI-first | ✅ CLI-first (CI/CD native) |
| ❌ No data-driven testing | ✅ CSV/JSON batch jobs built-in |
| ❌ No performance testing | ✅ Integrated load testing (v0.2) |
| ❌ Limited scripting | ✅ Full JS runtime |

---

### **K6's Strength (Learn From It)**

K6 dominates load testing because:
- ✅ **JavaScript familiarity** (developers already know it)
- ✅ **Code-as-tests** (no GUI needed)
- ✅ **CI/CD integration** (JSON/JUnit output)

**Lesson for Strex:** Match k6's DX (developer experience), but add **functional testing** that k6 lacks.

---

### **HTTPie's Lesson (CLI Purity Has Limits)**

HTTPie is beloved for CLI simplicity, but:
- ❌ No team collaboration features
- ❌ No complex workflows (multi-step auth flows)
- ❌ No GUI for non-technical users

**Lesson for Strex:** CLI-first is correct for MVP, but **plan GUI for v1.0** to avoid HTTPie's ceiling.

---

## Killer Feature: The "Why Switch?" Answer

### **Strex's Unique Value Proposition**

```
Git-native collections
  + CI-grade outputs (JSON/JUnit)
  + Data-driven batch jobs (CSV/JSON)
  + Integrated performance testing (v0.2)
  + Zero lock-in (local-first, offline)
= The only tool that does ALL of this
```

**Narrative:**
> "Postman locked features behind paywall. Bruno gives you Git integration but no performance testing. K6 gives you load testing but no functional testing. HTTPie is too simple for complex workflows. **Strex is the only tool that does functional + performance + data-driven in one reproducible CLI toolchain.**"

---

## Technology Swap Recommendations

### **KEEP These Decisions:**

| Technology | Verdict | Reason |
|------------|---------|--------|
| ✅ **All-Rust** | KEEP | Proven viable; Tokio handles 1.8M connections |
| ✅ **Tokio** | KEEP | Industry standard; excellent performance |
| ✅ **reqwest** | KEEP (for MVP) | Production-ready; sufficient for functional testing |
| ✅ **YAML** | KEEP | Git-friendly; just define strict subset |
| ✅ **rquickjs** | KEEP | AWS/Microsoft use it; just isolate execution |

---

### **MODIFY These Decisions:**

| Decision | Original | Modified | Reason |
|----------|----------|----------|--------|
| ⚠️ **Sequential execution** | "Sequential only in MVP" | **Add `--concurrency N` flag** | Data-driven batch jobs need parallelism |
| ⚠️ **Scripting safety** | "Sandboxed JS" | **Add timeouts + memory limits + worker threads** | Prevent DoS/hanging |
| ⚠️ **YAML validation** | "Strict validation" | **Define Strex YAML Subset** | Avoid parser ambiguity |

---

## 7-Point Action Plan (DO BEFORE CODING)

### **PHASE 0: Design Lockdown (2-3 days)**

1. ✅ **Create ADR-0002: Execution Model & Error Taxonomy**
   - Variable scope semantics (env vs collection vs data)
   - Error taxonomy (network vs script vs assertion)
   - Continue-on-error behavior across layers

2. ✅ **Create ADR-0003: Strex YAML Subset Definition**
   - Allowed constructs
   - Forbidden constructs (anchors, aliases, merges, tags)
   - Duplicate key policy
   - Maximum file size

3. ✅ **Create ADR-0004: Script Safety Model**
   - Worker thread architecture
   - Memory limits (default: 64MB)
   - Timeout policy (default: 30s)
   - Interrupt handler design

4. ✅ **Update ADR-0001: Add `--concurrency` flag**
   - Document bounded parallelism for data-driven mode
   - Default: `--concurrency 1` (sequential)
   - Semaphore-based implementation plan

---

### **PHASE 1: Proof of Concept (1 week)**

5. ✅ **Spike: Tokio + rquickjs Integration**
   - Build minimal example: HTTP request → script execution → timeout enforcement
   - Validate worker thread model
   - Measure JS-Rust marshalling overhead
   - **Success metric:** 1000 concurrent requests without blocking

6. ✅ **Spike: YAML Parser Safety**
   - Implement strict subset validation
   - Test edge cases (duplicate keys, large files, nested structures)
   - Validate `serde_yaml` behavior
   - **Success metric:** Parser rejects all forbidden constructs

7. ✅ **Spike: Data-Driven Concurrency**
   - Implement semaphore-based CSV iteration
   - Test variable isolation across iterations
   - Measure performance: 1000 rows sequential vs `--concurrency 10`
   - **Success metric:** 5x+ speedup with `--concurrency 10`, no variable leakage

---

## MVP Scope Adjustment

### **REMOVE from MVP (too risky):**
- ❌ ~~Postman `pm.*` compatibility~~ → Move to v0.3 (as planned)
- ❌ ~~Performance testing~~ → Move to v0.2 (as planned)

### **ADD to MVP (critical):**
- ✅ **`--concurrency N` flag** (data-driven parallelism)
- ✅ **Script timeout enforcement** (safety)
- ✅ **Memory limit for scripts** (safety)
- ✅ **Error taxonomy** (debuggability)

---

## Final Verdict

### ✅ **GO/NO-GO: GO**

The architecture is **fundamentally sound** and **validated by production evidence** (Fusillade, AWS LLRT, Tokio benchmarks).

**CONDITIONS:**
1. ✅ Implement **7-point action plan** before writing production code
2. ✅ Create **3 new ADRs** (execution model, YAML subset, script safety)
3. ✅ Run **3 validation spikes** (rquickjs integration, YAML safety, concurrency)

**TIMELINE:**
- Phase 0 (Design): 2-3 days
- Phase 1 (Spikes): 5-7 days
- **Total before MVP coding:** 1.5-2 weeks

---

## References

### Technical Evidence:
- [Fusillade GitHub](https://github.com/Fusillade-io/Fusillade) - 135K RPS with QuickJS + Rust
- [AWS LLRT](https://github.com/awslabs/llrt) - rquickjs in production (11K stars)
- [Tokio 1.8M connections study](https://techpreneurr.medium.com/how-to-achieve-1-8m-concurrent-connections-with-rust-inside-tokios-task-scheduler-122628ea1e28)
- [rquickjs documentation](https://docs.rs/rquickjs/)

### Competitive Analysis:
- [Bruno GitHub](https://github.com/usebruno/bruno) - File-based Postman alternative
- [K6 GitHub](https://github.com/grafana/k6) - Load testing with JS
- [HTTPie](https://httpie.io/) - CLI HTTP client

---

## Next Steps

1. **Review this validation report** with team
2. **Approve 7-point action plan** (or modify)
3. **Assign owners** for 3 new ADRs
4. **Schedule spike week** (Phase 1)
5. **Re-evaluate after spikes** — confirm/adjust architecture
6. **Begin MVP implementation** (only after all green lights)

**Estimated Time to MVP Start:** 1.5-2 weeks (with action plan)  
**Estimated MVP Duration:** 2-3 weeks (after spikes)  
**Total Time to MVP:** 4-5 weeks

---

**Report Prepared By:** Oracle + Explorer + Librarian Validation Team  
**Status:** ✅ **APPROVED WITH CONDITIONS**
