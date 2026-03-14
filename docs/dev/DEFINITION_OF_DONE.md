# Definition of Done

A task is **not done** until every item in both checklists below is satisfied.

---

## Functional Definition of Done

*Did we build the right thing?*

- [ ] The feature works end-to-end against a `wiremock` mock HTTP server — not just in unit tests.
- [ ] The happy path produces the correct output and exit code (`0` for all passed, `1` for assertion failures, `2` for runtime errors).
- [ ] All error cases defined in [ADR-0002](../adr/0002-execution-model-and-error-taxonomy.md) that the feature can produce are tested and return the correct exit code and output format.
- [ ] The feature stays within the MVP scope defined in [ADR-0001](../adr/0001-project-architecture-and-tech-stack.md). No scope creep.
- [ ] If the feature involves variable scoping, concurrent variable isolation is verified (two iterations run, no state leaks between them).
- [ ] CLI output matches the defined formats where applicable: human-readable console (default), JSON (`--output report.json`), JUnit XML (`--output report.xml --format junit`).

---

## Technical Definition of Done

*Did we build it right?*

- [ ] `cargo test` passes with zero failures across all crates.
- [ ] `cargo clippy -- -D warnings` passes clean.
- [ ] `cargo fmt --check` passes clean.
- [ ] All new `pub` items have `///` doc comments (including error fields and enum variants).
- [ ] No `unwrap()` or `expect()` calls in non-test code.
- [ ] Any new dependency added to `Cargo.toml` has a comment above it explaining why it was chosen.
- [ ] All commit messages follow the Conventional Commits format (`<type>(<scope>): <description>`). See [WORKFLOW.md](WORKFLOW.md).
- [ ] If the implementation deviates from or extends an ADR decision, the relevant ADR is updated in the same commit as the code change.

---

## Blocked State

A task is **blocked** — not done — if any item above is unmet.

Partial implementations must be flagged with a `// TODO(#<issue-number>): <description>` comment. Never leave silent incomplete work without a linked issue.

Example:
```rust
// TODO(#42): handle multipart form bodies — only JSON and plain text supported today
pub fn parse_body(body: &BodyConfig) -> Result<reqwest::Body, CollectionError> {
    // ...
}
```
