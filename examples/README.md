# Examples

Runnable strex collections against real public APIs. No authentication required, no setup needed.

---

## `jsonplaceholder.yaml`

Demonstrates the core strex workflow against [JSONPlaceholder](https://jsonplaceholder.typicode.com) — a free, public REST API designed for testing.

**What it covers:**

| Concept | Where |
|---------|-------|
| `environment` block for base URL | top of file |
| `variables` for cross-request state | steps 1 → 2 → 4 |
| `post_script` to extract values from a response | steps 1, 2, 3 |
| `pre_script` for pre-request logic | — |
| `jsonPath` assertions with `exists`, `equals`, `contains` | all steps |
| `status` assertions | all steps |
| `body` with `type: json` | step 3 |
| `assert()` in scripts | step 5 |
| `console.log()` for debug output | steps 1, 2, 3, 5 |

**Run it:**

```bash
strex run examples/jsonplaceholder.yaml
```

Expected output:

```
GET  Get user            ✓
GET  Get posts by user   ✓
POST Create post         ✓
GET  Get post by id      ✓
GET  Get post comments   ✓

5 requests · 5 passed · 0 failed
```

**Run with JSON output:**

```bash
strex run examples/jsonplaceholder.yaml --format json | jq '.requests[] | {name, passed, status}'
```

**Validate the collection without making HTTP requests:**

```bash
strex validate examples/jsonplaceholder.yaml
```

---

## Adding your own examples

To add an example:

1. Create a `.yaml` file in this directory following the [collection format](../docs/user/CLI.md#collection-format).
2. Add a section to this README describing what it demonstrates and how to run it.
3. Prefer public APIs that require no credentials. If an API key is needed, read it from the environment:

```yaml
environment:
  apiKey: "{{API_KEY}}"   # user sets: export API_KEY=your-key
```
