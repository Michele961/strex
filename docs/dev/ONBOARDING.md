# Developer Onboarding

Everything you need to clone, build, understand, and extend strex.

---

## Prerequisites

- **Rust stable** — install via [rustup](https://rustup.rs): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Git**

No other system dependencies. All C libraries (QuickJS for scripting) are vendored via Cargo.

---

## Clone and build

```bash
git clone https://github.com/your-org/strex
cd strex

# Debug build (fast to compile, slower to run)
cargo build

# Release build (the binary you'd actually ship)
cargo build --release
# → target/release/strex
```

---

## Run tests

```bash
# All tests across all crates
cargo test

# One crate only
cargo test -p strex-core
cargo test -p strex-cli

# One test by name (substring match)
cargo test variable_resolution

# With stdout visible (useful when debugging a failing test)
cargo test -- --nocapture
```

Before committing, always verify clippy and formatting pass:

```bash
cargo clippy -- -D warnings   # linting (warnings are errors)
cargo fmt --check              # formatting (use `cargo fmt` to fix)
```

Both checks run in CI and will block a merge if they fail.

---

## Frontend development (strex ui)

The web UI is a Svelte + Vite app in `crates/strex-ui/frontend/`. It is pre-built and the `dist/` output is committed — you only need Node.js if you're modifying the frontend.

**Prerequisites:** Node.js 20+

**Rebuild after editing Svelte files:**

```bash
cd crates/strex-ui/frontend
npm install      # first time only
npm run build    # writes to dist/ — commit the result
```

**Dev server with hot reload** (while the Rust server is running separately):

```bash
# Terminal 1: start the Rust backend
strex ui --port 7878

# Terminal 2: start the Vite dev server (proxies /api and /ws to port 7878)
cd crates/strex-ui/frontend
npm run dev
# open http://localhost:5173
```

Changes to `.svelte` files appear instantly in the dev server. When done, run `npm run build` and commit the updated `dist/`.

---

## Crate map

strex is a Cargo workspace with three crates:

| Crate | Path | Responsibility |
|-------|------|----------------|
| `strex-core` | `crates/strex-core/` | HTTP client, YAML parser, collection runner, variable interpolation, declarative assertions |
| `strex-script` | `crates/strex-script/` | Embedded QuickJS runtime, sandboxed JS API, memory and timeout limits |
| `strex-cli` | `crates/strex-cli/` | CLI argument parsing (`clap`), output formatting (console / JSON / JUnit XML), user-facing error display |

**How they relate:** `strex-cli` depends on `strex-core`. `strex-core` depends on `strex-script`. The CLI is a thin layer — it parses flags, calls into `strex-core`, and formats results. All HTTP execution, YAML parsing, and scripting logic lives in `strex-core` and `strex-script`.

---

## Project layout

```
strex/
├── Cargo.toml                  # Workspace root — lists member crates
├── Cargo.lock
├── README.md
│
├── crates/
│   ├── strex-core/
│   │   └── src/
│   │       ├── lib.rs          # Public API surface — re-exports only
│   │       ├── collection.rs   # Collection, Request, Body structs (serde)
│   │       ├── parser.rs       # YAML parsing + strict subset validation
│   │       ├── context.rs      # ExecutionContext — three-layer variable resolution
│   │       ├── interpolation.rs # {{variable}} template resolution
│   │       ├── runner.rs       # 7-phase request lifecycle orchestration
│   │       ├── http.rs         # reqwest HTTP client wrapper
│   │       ├── assertions.rs   # Declarative assertion evaluation
│   │       ├── data.rs         # Data-driven: CSV/JSON parsing, concurrent iteration
│   │       └── error.rs        # CollectionError, RequestError, DataError (thiserror)
│   │
│   ├── strex-script/
│   │   └── src/
│   │       ├── lib.rs          # Public API: execute_script, ScriptContext, ScriptResult
│   │       ├── executor.rs     # QuickJS runtime setup, spawn_blocking wrapper
│   │       ├── api.rs          # Injects globals: variables, response, env, data, assert*
│   │       ├── context.rs      # ScriptContext, ScriptResult, ScriptOptions types
│   │       └── error.rs        # ScriptError (thiserror)
│   │
│   └── strex-cli/
│       └── src/
│           ├── main.rs         # Tokio runtime, clap dispatch, exit code control
│           ├── cli.rs          # Cli, Command, RunArgs, ValidateArgs, OutputFormat
│           ├── commands/
│           │   ├── run.rs      # run subcommand: parse → load data → execute → format
│           │   └── validate.rs # validate subcommand: parse + variable-reference check
│           └── output/
│               ├── mod.rs      # RunResult, RunOutcome, format_failure, format dispatch
│               ├── console.rs  # Pretty console printer
│               ├── json.rs     # JSON serialization (serde_json::json! — no derive)
│               └── junit.rs    # JUnit XML serialization (string building — no XML lib)
│
└── docs/
    ├── adr/                    # Architecture Decision Records (see below)
    ├── dev/                    # Developer documentation (you are here)
    └── user/                   # User-facing documentation
```

---

## Where to start

**Fixing a bug in parsing or validation?**
Start at `crates/strex-core/src/parser.rs`. The YAML subset rules are documented in `docs/adr/0003-strex-yaml-subset-definition.md`.

**Fixing a bug in assertion evaluation?**
Start at `crates/strex-core/src/assertions.rs`. The error taxonomy is in `docs/adr/0002-execution-model-and-error-taxonomy.md`.

**Adding a new CLI flag?**
Add it to `crates/strex-cli/src/cli.rs` (the clap struct), wire it through in `crates/strex-cli/src/commands/run.rs` or `validate.rs`, and update `docs/user/CLI.md`.

**Changing script behaviour?**
The JS API globals (`variables`, `response`, `env`, `data`, `assert`) are injected in `crates/strex-script/src/api.rs`. The safety model is documented in `docs/adr/0004-script-safety-model.md`.

**Adding an output format?**
Add a new module in `crates/strex-cli/src/output/`, add a variant to `OutputFormat` in `cli.rs`, and wire it in `output/mod.rs::format()`.

---

## Deeper reading

| Document | What it covers |
|----------|---------------|
| [CODING_STANDARDS.md](CODING_STANDARDS.md) | Rust style, error handling rules, module layout, async rules |
| [TESTING.md](TESTING.md) | Unit, integration, and E2E test conventions; running tests |
| [WORKFLOW.md](WORKFLOW.md) | Branch naming, Conventional Commits, PR expectations |
| [DEFINITION_OF_DONE.md](DEFINITION_OF_DONE.md) | Checklist every feature must satisfy before merge |
| [ADR-0001](../adr/0001-project-architecture-and-tech-stack.md) | Why Rust, why QuickJS, why YAML |
| [ADR-0002](../adr/0002-execution-model-and-error-taxonomy.md) | Variable scoping, 7-phase lifecycle, error taxonomy |
| [ADR-0003](../adr/0003-strex-yaml-subset-definition.md) | Allowed and forbidden YAML constructs |
| [ADR-0004](../adr/0004-script-safety-model.md) | Worker thread model, memory limits, sandboxed JS API |
