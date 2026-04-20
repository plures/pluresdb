# Copilot Instructions — Plures Organization

## Source of Truth

You are working in the **plures** organization. Before making changes, understand our standards.

- **Development guide**: https://github.com/plures/development-guide
  - `standards/` — commit conventions, CI/CD, PR workflow, repo setup, code style
  - `practices/` — copilot delegation, reactive architecture, automation-first, local-first
  - `design/` — PARES-AGENS.md, THREE-AGENT-COGNITIVE-ARCHITECTURE.md, DEVELOPMENT-COORDINATOR.md
  - `lessons-learned/` — past mistakes to avoid (READ THESE)
  - `best-practices/praxis-adoption.md` — how to integrate Praxis

## Architecture Rules (NON-NEGOTIABLE)

### 1. Reactive Over Polling
Every state transition is event-driven. No cron for core lifecycle logic. Procedures over imperative code.
See: https://github.com/plures/development-guide/blob/main/practices/reactive-architecture.md

### 2. Praxis Gates on Decisions
No bare `if/else` for business logic. Decisions go through Praxis rules with typed facts, events, and derivations. Every decision becomes auditable.
See: https://github.com/plures/development-guide/blob/main/best-practices/praxis-adoption.md

### 3. Structured Observability Is Mandatory
Every module MUST have structured logging:
- **Rust**: Use `tracing` macros (`tracing::info!`, `tracing::error!`) — never `println!` or `dbg!`
- **TypeScript/Node**: Use structured logger — never bare `console.log` in production code
- **Every I/O boundary**: HTTP calls, DB queries, file ops must log URL/path, status, and response summary
- One debug line is NOT observability. Log operational events at `info` level.

### 4. Automation First
Write code so the user doesn't have to do anything. No manual steps, no "now open this file and change line 42." Fresh environment → one command → success.
See: https://github.com/plures/development-guide/blob/main/practices/automation-first.md

## Plures Stack Reference

| Package/Crate | Purpose | Repo |
|---|---|---|
| `pluresdb` | Graph DB + vector search + reactive procedures | https://github.com/plures/pluresdb |
| `@plures/praxis` | Typed logic engine (facts → rules → events → state) | https://github.com/plures/development-guide/blob/main/tools/praxis.md |
| `plureslm` | Memory recall/capture with native embeddings | Used across repos |
| `chronos` | Graph-native state chronicle (causal diffs) | https://github.com/plures/chronos |
| `design-dojo` | UI component library (Svelte 5) | https://github.com/plures/design-dojo |
| `unum` | Svelte 5 reactive bindings for PluresDB | https://github.com/plures/unum |

## Conventional Commits (REQUIRED)

```
<type>[optional scope]: <description>
```
Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, `build`
Breaking changes: add `!` after type or `BREAKING CHANGE:` in footer.

**PR Titles** — use conventional commit format (they become the squash commit message).

**Squash merge** — always. Clean single commit on `main`.

**Tests required** — all new features need tests. Bug fixes need a failing test first.

## Release Pipeline

Reusable release workflow from `plures/.github`. Do NOT manually bump versions.
Version bumps are automatic from conventional commits.

## What NOT To Do

- Do NOT add `#[allow(...)]` or `eslint-disable` to suppress warnings — fix the underlying issue
- Do NOT create sub-PRs that depend on other PRs
- Do NOT touch files outside the requested scope
- Do NOT manually bump version numbers
- Do NOT add bare `println!`, `dbg!`, or `console.log` — use structured logging
- Do NOT write imperative business logic — use Praxis rules where applicable
- Do NOT skip structured logging on any I/O boundary
- Do NOT add `skip` annotations to make CI pass
