# strex

A CLI-first API testing tool for developers who live in the terminal. Define HTTP collections in YAML, run them with a single command, and get CI-grade output.

strex is an open-source alternative to Postman's Collection Runner — no GUI, no proprietary format, no paywall.

---

## Features

- **Declarative YAML collections** — version-controlled, diff-friendly, human-readable
- **Declarative assertions** — status codes, JSON path, response headers
- **JavaScript scripting** — pre-request and post-request scripts with a sandboxed runtime
- **Data-driven testing** — run a collection against every row in a CSV or JSON file
- **Concurrent iterations** — `--concurrency N` runs multiple data rows in parallel
- **Three output formats** — `console` for humans, `json` for pipelines, `junit` for CI dashboards
- **Predictable exit codes** — `0` pass, `1` test failure, `2` infrastructure error

---

## Install

**macOS and Linux — Homebrew (recommended):**

```bash
brew tap Michele961/strex
brew install strex
```

**From source** (requires Rust stable):

```bash
git clone https://github.com/Michele961/strex
cd strex
cargo build --release
# Binary at: ./target/release/strex
```

Pre-built binaries for macOS (Intel + Apple Silicon) and Linux (x86_64 + ARM64) are attached to every [GitHub Release](https://github.com/Michele961/strex/releases).

---

## Quick start

Create a collection file:

```yaml
# github.yaml
name: GitHub API smoke test
version: "1.0"

environment:
  baseUrl: https://api.github.com

requests:
  - name: Get Octocat
    method: GET
    url: "{{baseUrl}}/users/octocat"
    assertions:
      - status: 200
      - jsonPath: "$.login"
        equals: octocat
```

Run it:

```bash
strex run github.yaml
```

```
GET  Get Octocat   ✓

1 requests · 1 passed · 0 failed
```

Failed assertions print inline:

```
GET  Get User   ✗
       assertion failed: status expected 200, got 404

1 requests · 0 passed · 1 failed
```

---

## Data-driven testing

Supply a CSV file to run the collection once per row:

```csv
username,role
alice,admin
bob,viewer
```

```bash
strex run permissions.yaml --data users.csv --concurrency 4
```

```
── Row 1 (username=alice, role=admin) ──────────────────
  GET  Check access   ✓
── Row 2 (username=bob, role=viewer) ───────────────────
  GET  Check access   ✗
    assertion failed: status expected 200, got 403

2 iterations · 1 passed · 1 failed
```

---

## Output formats

| Format | Flag | Use for |
|--------|------|---------|
| Console | `--format console` (default) | Local development |
| JSON | `--format json` | Downstream pipeline processing |
| JUnit XML | `--format junit` | Jenkins, GitHub Actions, GitLab CI |

```bash
# Write JUnit XML to a file for CI
strex run api.yaml --format junit --output results.xml
```

---

## Validate without running

Check a collection file for syntax errors and unresolved variable references without making any HTTP requests:

```bash
strex validate api.yaml
# valid: api.yaml (4 requests, 0 unresolved variables)
```

---

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | All assertions passed |
| `1` | One or more assertions failed (or a network error occurred during a request) |
| `2` | Pre-run error: bad collection file, unreadable data file, invalid flag |

---

## Documentation

- [CLI reference and tutorial](docs/user/CLI.md) — all commands, flags, collection format, output formats
- [Developer onboarding](docs/dev/ONBOARDING.md) — building from source, crate map, running tests
- [Examples](examples/README.md) — runnable collections against public APIs

---

## License

MIT
