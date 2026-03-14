# ADR-0003: Strex YAML Subset Definition

**Status:** Accepted  
**Date:** 2026-03-14  
**Supersedes:** None  
**Related:** ADR-0001 (Architecture), ADR-0002 (Execution Model)

---

## Context

YAML 1.2 is a complex specification with many features that, while powerful, introduce:

- **Ambiguity** — implicit typing, multiple ways to express the same thing
- **Security risks** — billion laughs attack via alias expansion, arbitrary code execution via tags
- **Parser inconsistencies** — different YAML parsers handle edge cases differently
- **Debugging difficulty** — anchors and merges make collections harder to understand

Strex collections must be:
- **Git-friendly** — clean diffs, human-readable, mergeable
- **Predictable** — same file always means the same thing
- **Secure** — no parser vulnerabilities
- **Tool-friendly** — IDE autocomplete, schema validation work reliably

This ADR defines the **Strex YAML Subset**: what is allowed, what is forbidden, and how violations are handled.

---

## Decision

### 1. Strex YAML Subset (Allowed Features)

Strex supports a **restricted subset of YAML 1.2**:

#### ✅ Allowed Constructs

| Feature | Example | Notes |
|---------|---------|-------|
| **Maps (Objects)** | `key: value` | Nested allowed |
| **Sequences (Arrays)** | `- item1`<br>`- item2` | Nested allowed |
| **Strings** | `"quoted"`, `unquoted`, `\|` multiline | Explicit quotes preferred |
| **Numbers** | `123`, `3.14`, `-1` | Integer or float |
| **Booleans** | `true`, `false` | Lowercase only |
| **Null** | `null` or empty value | Explicit `null` preferred |
| **Comments** | `# This is a comment` | Anywhere except mid-line |
| **Multiline Strings (Literal)** | `\|`<br>`  line1`<br>`  line2` | Preserves newlines |
| **Multiline Strings (Folded)** | `>`<br>`  text` | Folds into single line |

#### Example Valid Collection:

```yaml
name: "GitHub API Tests"
version: "1.0"

# Global environment variables
environment:
  baseUrl: "https://api.github.com"
  timeout: 30

# Mutable collection variables
variables:
  token: null
  userId: null

# Request definitions
requests:
  - name: "Login"
    method: POST
    url: "{{environment.baseUrl}}/auth/login"
    headers:
      Content-Type: "application/json"
    body:
      type: json
      content:
        username: "{{data.username}}"
        password: "{{data.password}}"
    script: |
      const response_data = response.json();
      variables.set("token", response_data.token);
    assertions:
      - status: 200
      - jsonPath: "$.token"
        exists: true

  - name: "Get User Profile"
    method: GET
    url: "{{environment.baseUrl}}/user/profile"
    headers:
      Authorization: "Bearer {{token}}"
    assertions:
      - status: 200
      - jsonPath: "$.username"
        equals: "{{data.username}}"
```

---

### 2. Forbidden Constructs (Strict Mode)

#### ❌ Explicitly Forbidden

| Feature | Example | Why Forbidden | Error Message |
|---------|---------|---------------|---------------|
| **Anchors & Aliases** | `&anchor`, `*alias` | Makes diffs unreadable, causes expansion attacks | "Anchors/aliases not allowed. Define values explicitly." |
| **Merge Keys** | `<<: *default` | Implicit behavior, confusing diffs | "Merge keys not allowed. Inline values explicitly." |
| **Custom Tags** | `!!python/object` | Security risk (arbitrary code execution) | "Custom tags not allowed for security." |
| **Explicit Tags** | `!!str "123"` | Unnecessary complexity | "Explicit tags not needed. Remove `!!str`." |
| **Duplicate Keys** | `name: "a"`<br>`name: "b"` | Ambiguous, parser-dependent | "Duplicate key 'name' found. Use unique keys." |
| **Document Markers** | `---` (between docs) | Collections must be single-document | "Multiple YAML documents not supported. Use one file per collection." |
| **Directives** | `%YAML 1.2` | Unnecessary, version enforced by parser | "YAML directives not allowed." |
| **Binary Data** | `!!binary` | Use files instead | "Binary data not allowed. Reference files via path." |

#### Example Invalid Collection:

```yaml
# ❌ INVALID: Uses anchors, aliases, and merge keys
name: "Invalid Collection"

default_headers: &headers
  Content-Type: application/json
  Accept: application/json

requests:
  - name: "Request 1"
    headers:
      <<: *headers  # ❌ Merge key
      Authorization: "Bearer {{token}}"
  
  - name: "Request 2"
    headers: *headers  # ❌ Alias
```

**Parser Error:**
```
❌ Collection Validation Failed: collection.yaml

Line 3: Anchors not allowed (&headers)
Line 9: Merge keys not allowed (<<: *headers)
Line 13: Aliases not allowed (*headers)

→ Define headers explicitly in each request or use environment variables.
```

---

### 3. Strict Validation Rules

#### Rule 1: Duplicate Keys Are Rejected

**Invalid:**
```yaml
requests:
  - name: "Get User"
    method: GET
    name: "Create User"  # ❌ Duplicate 'name'
```

**Error:**
```
❌ Duplicate key 'name' at line 4
   First occurrence: line 2
   
   → Each key in a map must be unique.
```

#### Rule 2: Unknown Fields Are Rejected

Collections must conform to the schema. Unknown fields are errors (not warnings).

**Invalid:**
```yaml
requests:
  - name: "Get User"
    metod: GET  # ❌ Typo: should be 'method'
    url: "https://api.example.com/users/1"
```

**Error:**
```
❌ Unknown field 'metod' at line 3
   Did you mean: method?
   
   Valid fields: name, method, url, headers, body, script, assertions
```

**Rationale:** Catch typos early rather than silently ignoring them.

#### Rule 3: Maximum File Size (10MB)

Collections larger than 10MB are rejected.

**Error:**
```
❌ Collection file too large: 12.3 MB (limit: 10 MB)
   
   → Split into multiple collection files or reduce request count.
```

**Rationale:** Prevents parser DoS, encourages modular collections.

#### Rule 4: Maximum Nesting Depth (20 levels)

Prevents stack overflow and readability issues.

**Invalid:**
```yaml
a:
  b:
    c:
      # ... 20 levels deep ...
        z: value  # ❌ Too deep
```

**Error:**
```
❌ Maximum nesting depth exceeded (20 levels)
   
   → Flatten your collection structure.
```

#### Rule 5: String Interpolation Only in Values

Template variables (`{{...}}`) are only allowed in **string values**, not keys.

**Invalid:**
```yaml
requests:
  - "{{requestName}}": value  # ❌ Variable in key
```

**Valid:**
```yaml
requests:
  - name: "{{requestName}}"  # ✅ Variable in value
```

---

### 4. Permissive Mode (`--loose` flag)

For forward compatibility or importing external collections, Strex supports a **permissive mode**:

```bash
strex run collection.yaml --loose
```

**Behavior in Permissive Mode:**

| Validation | Strict Mode | Permissive Mode |
|------------|-------------|-----------------|
| Unknown fields | ❌ Error | ⚠️ Warning (ignored) |
| Duplicate keys | ❌ Error | ⚠️ Warning (last value wins) |
| Anchors/aliases | ❌ Error | ✅ Allowed (expanded) |
| Custom tags | ❌ Error | ❌ Error (security) |
| File size > 10MB | ❌ Error | ⚠️ Warning |

**Use Cases:**
- Importing Postman collections (may have unknown fields)
- Testing backward compatibility
- Gradual migration from other tools

**Warning:**
```
⚠️ Permissive mode enabled (--loose)
   3 unknown fields ignored
   1 duplicate key resolved (last value wins)
   
   → Run without --loose to see strict validation errors.
```

---

### 5. Schema Validation (JSON Schema)

Strex collections are validated against a **JSON Schema** for:
- IDE autocomplete (VS Code, IntelliJ)
- Pre-commit hooks (validate before push)
- Documentation generation

**Schema Location:** `formats/collection.schema.json`

**VS Code Integration:**
```yaml
# collection.yaml (first line)
# yaml-language-server: $schema=https://strex.dev/schema/v1/collection.json

name: "My Collection"
# ... rest of collection
```

**Benefits:**
- Real-time validation in IDE
- Autocomplete for field names
- Inline documentation (hover tooltips)

---

### 6. Type Coercion Rules

Strex uses **strict typing** to avoid YAML's implicit type conversion surprises:

| Input | YAML Default | Strex Interpretation |
|-------|--------------|----------------------|
| `no` | Boolean `false` | ❌ Error: "Use explicit `false`" |
| `yes` | Boolean `true` | ❌ Error: "Use explicit `true`" |
| `on` | Boolean `true` | ❌ Error: "Use explicit `true`" |
| `off` | Boolean `false` | ❌ Error: "Use explicit `false`" |
| `123` | Integer | ✅ Integer |
| `"123"` | String | ✅ String |
| `3.14` | Float | ✅ Float |
| `null` | Null | ✅ Null |
| Empty value | Null | ✅ Null |
| `!!str 123` | String | ❌ Error: "Remove explicit tag" |

**Rationale:** Prevent surprises like `country: NO` (Norway) being parsed as `false`.

---

## Consequences

### Positive

1. **Predictable Parsing** — no hidden behavior from YAML features
2. **Security** — no alias expansion attacks, no tag-based code execution
3. **Git-Friendly** — no anchors/aliases to obscure diffs
4. **IDE Support** — JSON Schema enables autocomplete and validation
5. **Catch Errors Early** — typos and unknown fields rejected immediately

### Negative

1. **Learning Curve** — users familiar with full YAML may be surprised by restrictions
2. **Verbosity** — no anchors/aliases means some repetition (mitigated by environment variables)
3. **Migration Friction** — existing YAML collections may need rewriting

### Risks

1. **Schema Drift** — must keep JSON Schema in sync with parser implementation
2. **Permissive Mode Abuse** — users may rely on `--loose` and miss real errors
3. **Type Coercion Edge Cases** — strict typing may reject valid use cases (monitor feedback)

---

## Implementation Notes

### Parser Configuration (serde_yaml)

```rust
use serde_yaml::{Value, Mapping};

fn parse_collection(yaml_str: &str, strict: bool) -> Result<Collection> {
    // 1. Parse YAML with serde_yaml
    let value: Value = serde_yaml::from_str(yaml_str)
        .map_err(|e| ValidationError::YamlParseError(e.to_string()))?;
    
    // 2. Validate subset constraints
    validate_no_anchors(&value)?;
    validate_no_custom_tags(&value)?;
    validate_max_depth(&value, 0)?;
    validate_file_size(yaml_str.len())?;
    
    // 3. Check for duplicate keys (serde_yaml may silently overwrite)
    check_duplicate_keys(yaml_str)?;
    
    // 4. Deserialize into Collection struct
    let collection: Collection = serde_yaml::from_value(value)
        .map_err(|e| ValidationError::SchemaValidation(e.to_string()))?;
    
    // 5. Validate against JSON Schema (if strict)
    if strict {
        validate_against_schema(&collection)?;
    }
    
    Ok(collection)
}

fn validate_no_anchors(value: &Value) -> Result<()> {
    // Recursively check for anchors (stored in tagged values)
    // serde_yaml doesn't expose anchors directly, so we need to:
    // 1. Use a custom Deserializer, OR
    // 2. Pre-parse with yaml-rust and check for anchors
    
    // Pseudo-code:
    if yaml_str.contains('&') || yaml_str.contains('*') {
        return Err(ValidationError::AnchorsNotAllowed);
    }
    Ok(())
}
```

### Duplicate Key Detection

```rust
fn check_duplicate_keys(yaml_str: &str) -> Result<()> {
    // serde_yaml silently overwrites duplicate keys
    // We need to parse manually to detect duplicates
    
    use yaml_rust::{YamlLoader, Yaml};
    
    let docs = YamlLoader::load_from_str(yaml_str)?;
    for doc in docs {
        check_duplicates_recursive(&doc, vec![])?;
    }
    Ok(())
}

fn check_duplicates_recursive(node: &Yaml, path: Vec<String>) -> Result<()> {
    if let Yaml::Hash(map) = node {
        let mut seen = HashSet::new();
        for (key, value) in map {
            let key_str = key.as_str().unwrap_or("<non-string-key>");
            if !seen.insert(key_str) {
                return Err(ValidationError::DuplicateKey {
                    key: key_str.to_string(),
                    path: path.join("."),
                });
            }
            check_duplicates_recursive(value, path.clone())?;
        }
    }
    Ok(())
}
```

---

## Validation Error Examples

### Example 1: Anchor Usage

**Input:**
```yaml
default_headers: &headers
  Content-Type: application/json

requests:
  - name: "Request"
    headers: *headers
```

**Output:**
```
❌ Collection Validation Failed: collection.yaml

Line 1: Anchors not allowed (&headers)
Line 6: Aliases not allowed (*headers)

Strex uses a restricted YAML subset for predictability and security.

Fix: Define headers explicitly or use environment variables:

  environment:
    contentType: "application/json"
  
  requests:
    - name: "Request"
      headers:
        Content-Type: "{{contentType}}"
```

### Example 2: Duplicate Keys

**Input:**
```yaml
requests:
  - name: "Get User"
    method: GET
    method: POST  # Duplicate
```

**Output:**
```
❌ Duplicate key 'method' in requests[0]
   First: line 3 (value: GET)
   Duplicate: line 4 (value: POST)
   
   → Remove one of the duplicate keys.
```

### Example 3: Unknown Field

**Input:**
```yaml
requests:
  - name: "Test"
    metod: GET  # Typo
```

**Output:**
```
❌ Unknown field 'metod' in requests[0]
   Did you mean: method?
   
   Valid fields:
   - name, method, url, headers, body, script, assertions, timeout
   
   Run with --loose to ignore unknown fields (not recommended).
```

---

## Migration Guide (From Full YAML)

### Converting Anchors to Environment Variables

**Before (Full YAML):**
```yaml
default_headers: &headers
  Content-Type: application/json
  Accept: application/json

requests:
  - name: "Request 1"
    headers: *headers
  - name: "Request 2"
    headers: *headers
```

**After (Strex YAML):**
```yaml
environment:
  contentType: "application/json"
  accept: "application/json"

requests:
  - name: "Request 1"
    headers:
      Content-Type: "{{contentType}}"
      Accept: "{{accept}}"
  - name: "Request 2"
    headers:
      Content-Type: "{{contentType}}"
      Accept: "{{accept}}"
```

### Converting Merge Keys

**Before:**
```yaml
base: &base
  timeout: 30
  headers:
    Content-Type: application/json

requests:
  - <<: *base
    name: "Request 1"
    url: "/api/users"
```

**After:**
```yaml
environment:
  timeout: 30
  contentType: "application/json"

requests:
  - name: "Request 1"
    url: "/api/users"
    timeout: "{{timeout}}"
    headers:
      Content-Type: "{{contentType}}"
```

---

## Alternatives Considered

### Alternative 1: Allow Full YAML (No Restrictions)

**Rejected because:**
- Security risks (tag-based code execution)
- Billion laughs attack via alias expansion
- Parser inconsistencies across tools
- Harder to debug (anchors obscure diffs)

### Alternative 2: Use JSON Instead

**Rejected because:**
- Less human-readable (no comments, strict quoting)
- Harder to write multiline strings (scripts)
- Postman uses JSON but it's machine-generated

**Compromise:** Support JSON as input format (alternative to YAML):
```bash
strex run collection.json  # Auto-detect JSON
```

### Alternative 3: Use TOML Instead

**Rejected because:**
- Less familiar to API testing users
- Nested structures more verbose than YAML
- No clear advantage over restricted YAML

---

## Future Considerations

### Allow Optional Includes (v0.3+)

```yaml
# collection.yaml
name: "Main Collection"

includes:
  - "./shared/auth.yaml"
  - "./shared/common-headers.yaml"

requests:
  # ... rest
```

**Benefit:** Reduce repetition without anchors/aliases.

**Challenge:** Must validate included files, handle circular dependencies.

---

## Related Decisions

- **ADR-0001**: Architecture — chose YAML as primary format
- **ADR-0002**: Execution Model — defines variable interpolation syntax
- **ADR-0004**: Script Safety — defines scripting subset within YAML

---

## References

- [YAML 1.2 Specification](https://yaml.org/spec/1.2.2/)
- [YAML Security Issues](https://en.wikipedia.org/wiki/Billion_laughs_attack)
- [serde_yaml Documentation](https://docs.rs/serde_yaml/)
- [JSON Schema](https://json-schema.org/)
