# Development Workflow

## Branch Naming

| Prefix | Use for | Example |
|--------|---------|---------|
| `feat/` | New features | `feat/yaml-parser` |
| `fix/` | Bug fixes | `fix/variable-isolation-leak` |
| `chore/` | Tooling, deps, config | `chore/add-clippy-deny` |
| `docs/` | Documentation only | `docs/update-adr-0001` |
| `spike/` | Throwaway proof-of-concept | `spike/rquickjs-timeout` |

## Conventional Commits

Format: `<type>(<scope>): <description>`

**Scope** is the crate name: `core`, `script`, `cli`, or `docs`.

```
feat(core): add YAML strict validation rejecting anchors
fix(script): enforce 64MB memory limit on QuickJS runtime
test(cli): add E2E test for DNS resolution failure exit code
docs(adr): update ADR-0001 with --concurrency flag decision
chore(core): add thiserror and serde_yaml dependencies
refactor(core): split parser.rs into parser.rs and validator.rs
```

Valid types:

| Type | Use for |
|------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `test` | Adding or fixing tests |
| `docs` | Documentation only |
| `chore` | Tooling, dependencies, config |
| `refactor` | Code change with no behavior change |
| `perf` | Performance improvement |

## Development Phases

### Phase 1: Spikes

Small, throwaway proof-of-concept branches. The goal is to validate a technical approach — not to produce production-quality code.

- Full Definition of Done does **not** apply to spike branches.
- Use `spike/` branch prefix.
- Output: spike branch + a brief note on findings (PR description or a comment in the relevant ADR).
- **Transition to Phase 2** when the spike confirms the approach is viable and the team agrees to proceed.

The three spikes defined in `docs/VALIDATION-REPORT.md` are:

1. `spike/rquickjs-timeout` — Tokio + rquickjs integration, timeout enforcement, worker thread model
2. `spike/yaml-safety` — YAML strict subset validation, duplicate key detection, serde_yaml behavior
3. `spike/data-concurrency` — Semaphore-based CSV iteration, variable isolation across concurrent iterations

### Phase 2: MVP

Production code. The full Definition of Done applies from the first commit. No exceptions.

## When to Update an ADR

Update a relevant ADR when your implementation:
- Deviates from the decision recorded in the ADR
- Extends or narrows the decision (e.g. adds a new flag, changes a default)
- Reveals that a recorded decision was incorrect

Rules:
1. Update the ADR in the **same commit** as the code change — not in a separate later commit.
2. Mention the ADR in the commit message body if the change is non-trivial.
3. If the change is significant enough to represent a new decision, create a new ADR rather than amending an existing one.

ADRs are living documents, not frozen historical records.

## PR Expectations

- **Small, focused PRs.** One feature or fix per PR.
- Do not bundle unrelated changes (e.g., a new feature + an unrelated refactor in the same PR).
- PR title follows Conventional Commits format.
- PR description must:
  - Summarize what changed and why.
  - Link to the relevant ADR if an architectural decision is involved.
  - List any manual testing steps if E2E tests don't cover the full scenario.
- All Definition of Done items must be satisfied before requesting review.
