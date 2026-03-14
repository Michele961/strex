# Architecture Decision Records (ADR)

This directory contains Architecture Decision Records for the Strex project.

## What is an ADR?

An Architecture Decision Record (ADR) captures an important architectural decision made along with its context and consequences.

## Format

Each ADR follows this structure:

- **Status**: Proposed | Accepted | Deprecated | Superseded
- **Date**: When the decision was made
- **Context**: The issue motivating this decision
- **Decision**: The change we're proposing or have agreed to
- **Consequences**: The results of applying this decision

## Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [0001](./0001-project-architecture-and-tech-stack.md) | Project Architecture and Tech Stack | Accepted | 2026-03-14 |
| [0002](./0002-execution-model-and-error-taxonomy.md) | Execution Model and Error Taxonomy | Accepted | 2026-03-14 |
| [0003](./0003-strex-yaml-subset-definition.md) | Strex YAML Subset Definition | Accepted | 2026-03-14 |
| [0004](./0004-script-safety-model.md) | Script Safety Model | Accepted | 2026-03-14 |

## Creating a New ADR

1. Copy the template: `cp template.md XXXX-title.md`
2. Fill in the sections
3. Submit for review
4. Update this index
