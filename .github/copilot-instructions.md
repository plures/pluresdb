# Copilot Instructions

## Organization Standards

You are working in the **plures** organization. Before making changes, understand our standards and architecture.

### Source of Truth
- **Development guide:** https://github.com/plures/development-guide
  - `standards/` — commit conventions, CI/CD, PR workflow, repo setup, code style
  - `practices/` — copilot delegation, merge sweeps, local-first development
  - `design/` — architecture decisions, design patterns
  - `lessons-learned/` — past mistakes to avoid

### Architecture Principles

**Praxis-First Development** — decisions go through Praxis rules, not bare if/else:
- Decision ledger (ADRs) with evidence tables — tested facts, unknowns marked
- Expectations as constraints — severity = error, check functions enforce correctness
- Model before code — expand types, wire fetcher, then write expectations
- Never ship warnings — if it's wrong, it's an error

**Structured Observability** — tracing macros, not println/dbg:
- Every I/O boundary logged with structured spans
- Use `tracing::instrument` (Rust) or OpenTelemetry (TypeScript)
- NO bare `println!`, `dbg!`, or `console.log` in production code

**Reactive Architecture** — procedures over code, events over polling:
- PluresDB procedures for orchestration
- Event-driven workflows, not cron jobs
- Scheduled tasks are a reliability anti-pattern

### Plures Stack Reference

| Component | Purpose | Language | Key Features |
|-----------|---------|----------|--------------|
| **pluresdb** | Distributed database | Rust | Memory, procedures, sync, constraints |
| **praxis** | Business logic engine | Rust/TypeScript | Expectations, ADRs, models, event lifecycle |
| **plureslm** | Long-term memory | TypeScript | Native embeddings, MCP server, graph traversal |
| **chronos** | Time-series engine | Rust | Event streams, temporal queries |
| **unum** | Numerical computing | Rust | High-perf math, vectorization |
| **design-dojo** | Design exploration | TypeScript | Prototyping, UI experiments |

### Commit Standards (MANDATORY)

**Conventional Commits** — all commit messages MUST follow:
```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, `build`

**Breaking changes:** add `!` after type or `BREAKING CHANGE:` in footer

**PR Titles** — use conventional commit format (they become the squash commit message)

**Squash merge** — always. Clean single commit on `main`

**Tests required** — all new features need tests. All bug fixes need a failing test first.

### Release Pipeline

We use a **reusable release workflow** from `plures/.github`:

```yaml
# .github/workflows/release.yml — should exist in this repo
name: Release
on:
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      bump:
        type: choice
        options: [patch, minor, major]
jobs:
  release:
    uses: plures/.github/.github/workflows/release-reusable.yml@main
    with:
      bump: ${{ inputs.bump || '' }}
    secrets: inherit
```

The pipeline auto-detects project type and publishes to:
- **GitHub Packages** (`@plures/*`) — always (uses `GITHUB_TOKEN`)
- **npm** (npmjs.com) — if `NPM_TOKEN` secret exists
- **Cargo** (crates.io) — if `Cargo.toml` and `CARGO_REGISTRY_TOKEN`

Version bumps are automatic from conventional commits.

### What NOT to Do

**Code Quality:**
- ❌ NO `#[allow(...)]` or `#![allow(...)]` suppressions — fix the underlying issue
- ❌ NO `// eslint-disable` — fix the lint violation
- ❌ NO bare `println!`, `dbg!`, or `console.log` in production code — use structured tracing
- ❌ NO manual version bumps — release workflow handles this

**Process:**
- ❌ NO sub-PRs that depend on other PRs — merge parent first
- ❌ NO touching files outside the requested scope
- ❌ NO skipping tests or adding `#[ignore]`/`skip` to make CI pass

**Architecture:**
- ❌ NO cron jobs for orchestration — use reactive procedures
- ❌ NO polling loops — subscribe to events
- ❌ NO bare if/else business logic — use Praxis expectations

### When in Doubt
1. Check the development guide
2. Look for existing ADRs in `.praxis/decisions/`
3. Ask before breaking established patterns
