# Copilot Instructions

## Organization Standards

You are working in the **plures** organization. Before making changes, understand our standards and architecture.

### Source of Truth
- **Development guide:** https://github.com/plures/development-guide
  - `standards/` — commit conventions, CI/CD, PR workflow, repo setup, code style
  - `practices/` — copilot delegation, merge sweeps, local-first development
  - `design/` — architecture decisions, design patterns
  - `lessons-learned/` — past mistakes to avoid (READ THESE)

### Architecture Principles (NON-NEGOTIABLE)

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

**Design-Dojo Mandate** — ALL UI must use design-dojo components:
- No raw HTML elements (`<button>`, `<aside>`, `<nav>`) in application code
- Every UI component must come from `@plures/design-dojo`
- If a component doesn't exist in design-dojo, build it there first, then import
- Never work around a missing component by building it locally
- Schema-driven: components generated from praxis rules and PluresDB schemas

**Praxis-Composed Applications** — apps MUST be wholly composed of praxis primitives:
- Every decision = a Rule with a Contract
- Every state change = an Event processed by the Engine
- Every UI component = design-dojo, generated from schemas
- Every data operation = PluresDB graph write, automatic via praxis persistence layer
- No imperative logic. No ad-hoc conditionals. No raw HTML.

### Automation Rules (ABSOLUTE)

**Automation changes go straight to code.** Never create GitHub issues for workflow/CI/release pipeline/lifecycle changes. Implement directly — commit and push. Issues are for feature work and bugs only. This is non-negotiable and has been violated repeatedly — stop.

**Zero nudges. Ever.** No `@copilot` comments, no retry comments, no "please implement" comments, no `createComment` calls on issues. Nudges don't work and pollute history. If stalled: close → recreate → reassign.

**Single assignment authority.** Only the lifecycle workflow assigns Copilot. Never assign manually from other workflows or scripts (race condition evidence from netops-toolkit #20/#21).

**Roadmap-aware versioning.** The release pipeline supports `target_version` input for milestone-driven releases. When a milestone closes, the lifecycle workflow extracts the semver from the milestone title and triggers a release at that exact version. Do NOT manually bump versions.

### Plures Stack Reference

| Component | Purpose | Language | Key Features |
|-----------|---------|----------|--------------|
| **pluresdb** | Distributed database | Rust | CRDT store, HNSW vectors, reactive procedures, P2P sync |
| **praxis** | Business logic engine | Rust/TypeScript | Expectations, ADRs, decision ledger, event lifecycle |
| **plureslm** | Long-term memory | TypeScript | Native embeddings (BGE-small), MCP server, graph traversal |
| **chronos** | State chronicle | TypeScript | Causal diffs, temporal queries, PluresDB writer |
| **unum** | Reactive bindings | TypeScript | Svelte 5 bindings for PluresDB |
| **design-dojo** | UI component library | Svelte 5 | Sidebar, StatusBar, Button, Tabs, ActivityBar, TitleBar |
| **pares-agens** | Agent runtime | Rust | Telegram, cerebellum, delegation, PluresDB memory |
| **pares-radix** | Application shell | TypeScript | Plugin loader, inference engine, UX contracts |
| **pares-modulus** | Plugin registry | TypeScript | Gated submissions, manifest validation, security scan |

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

Reusable release workflow from `plures/.github`:

```yaml
name: Release
on:
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      bump:
        type: choice
        options: [patch, minor, major]
      target_version:
        description: 'Exact version (e.g. 1.0.0). Overrides bump logic.'
        type: string
        required: false
jobs:
  release:
    uses: plures/.github/.github/workflows/release-reusable.yml@main
    with:
      bump: ${{ inputs.bump || '' }}
      target_version: ${{ inputs.target_version || '' }}
    secrets: inherit
```

The pipeline auto-detects project type, syncs ALL version files (Cargo.toml, package.json, tauri.conf.json, deno.json), and publishes to registries.

Version bumps are automatic from conventional commits. Milestone-driven releases use `target_version`.

### Copilot Issue Requirements (ADR-0004)

For Copilot SWE agent to process issues, BOTH are required:
- **Label**: At least one label (e.g., `enhancement`, `bug`)
- **Type**: Issue type set (Feature, Bug, Task)

Without both, Copilot **silently cancels**. No error, no notification — just nothing happens.

### What NOT to Do

**Code Quality:**
- ❌ NO `#[allow(...)]` or `#![allow(...)]` suppressions — fix the underlying issue
- ❌ NO `// eslint-disable` — fix the lint violation
- ❌ NO bare `println!`, `dbg!`, or `console.log` in production code — use structured tracing
- ❌ NO manual version bumps — release workflow handles this
- ❌ NO raw HTML elements in app code — use design-dojo components

**Process:**
- ❌ NO sub-PRs that depend on other PRs — merge parent first
- ❌ NO touching files outside the requested scope
- ❌ NO skipping tests or adding `#[ignore]`/`skip` to make CI pass
- ❌ NO creating GitHub issues for automation/workflow/CI changes — implement directly
- ❌ NO nudging Copilot with comments — close and recreate if stalled

**Architecture:**
- ❌ NO cron jobs for orchestration — use reactive procedures
- ❌ NO polling loops — subscribe to events
- ❌ NO bare if/else business logic — use Praxis expectations
- ❌ NO local UI components — contribute to design-dojo first

### When in Doubt
1. Check the development guide
2. Look for existing ADRs in `.praxis/decisions/`
3. Ask before breaking established patterns
