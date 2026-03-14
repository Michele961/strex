# Strex — Claude Instructions

## What is Strex

Strex (stress + execution) is a CLI-first, Git-native API testing tool built in Rust — an open-source alternative to Postman's Collection Runner. It supports declarative YAML-based test collections, JavaScript scripting, data-driven testing (CSV/JSON), and CI-grade output formats (console, JSON, JUnit XML). **No code exists yet.** All architectural decisions are captured in ADRs under `docs/adr/`.

## Crate Map

| Crate | Path | Responsibility |
|-------|------|----------------|
| `strex-core` | `crates/strex-core/` | HTTP client, YAML parser, collection runner, variable interpolation, declarative assertions |
| `strex-script` | `crates/strex-script/` | QuickJS/rquickjs JS runtime, script sandboxing, memory and timeout limits |
| `strex-cli` | `crates/strex-cli/` | CLI argument parsing, output formatting (console/JSON/JUnit XML), user-facing error display |

## Key Rules

- **Error handling:** `thiserror` in `strex-core` and `strex-script`, `anyhow` in `strex-cli`. See [CODING_STANDARDS.md](docs/dev/CODING_STANDARDS.md).
- **No `unsafe` code** without an explicit justification comment at the call site.
- **No `unwrap()` or `expect()`** in non-test code — use `?` or an explicit error variant.
- **All `pub` items** must have `///` doc comments.
- **Tests required** for every feature before it is considered done. See [TESTING.md](docs/dev/TESTING.md).
- **`cargo clippy -- -D warnings` and `cargo fmt --check` must pass** before any commit.
- **Conventional Commits** required on every commit. See [WORKFLOW.md](docs/dev/WORKFLOW.md).

## What NOT to Do

- Do not add dependencies to `Cargo.toml` without a justification comment above the entry.
- Do not implement features outside the MVP scope defined in [ADR-0001](docs/adr/0001-project-architecture-and-tech-stack.md).
- Do not run scripts on Tokio async executor threads — use `spawn_blocking`. See [ADR-0004](docs/adr/0004-script-safety-model.md).
- Do not use `unwrap()` or `expect()` in non-test code.
- Do not add `anyhow` to `strex-core` or `strex-script`.
- Do not use YAML anchors, aliases, or merge keys in collection files. See [ADR-0003](docs/adr/0003-strex-yaml-subset-definition.md).

## ADR Index

| ADR | Title | Key Decision |
|-----|-------|-------------|
| [ADR-0001](docs/adr/0001-project-architecture-and-tech-stack.md) | Project Architecture and Tech Stack | All-Rust, Tokio, reqwest, rquickjs, YAML, MVP scope |
| [ADR-0002](docs/adr/0002-execution-model-and-error-taxonomy.md) | Execution Model and Error Taxonomy | Variable scoping, 7-phase request lifecycle, error taxonomy |
| [ADR-0003](docs/adr/0003-strex-yaml-subset-definition.md) | Strex YAML Subset Definition | Allowed/forbidden YAML constructs, strict validation rules |
| [ADR-0004](docs/adr/0004-script-safety-model.md) | Script Safety Model | Worker threads, memory/timeout limits, sandboxed JS API |

## Deep Docs

- [Coding Standards](docs/dev/CODING_STANDARDS.md) — Rust style, error handling, module layout, async rules
- [Testing](docs/dev/TESTING.md) — unit, integration, and E2E test conventions
- [Definition of Done](docs/dev/DEFINITION_OF_DONE.md) — functional and technical checklist
- [Workflow](docs/dev/WORKFLOW.md) — branch naming, commit format, PR expectations
