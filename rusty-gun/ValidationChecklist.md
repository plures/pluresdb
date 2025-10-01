# Validation Checklist

This checklist tracks implementation and verification of the roadmap items. Each
item has concrete, testable criteria.

## Core Graph Storage (Deno.Kv)

- [ ] Able to open Deno.Kv and persist nodes across process restarts
  - How to validate: `deno run -A examples/basic-usage.ts`, restart, then
    `db.get()` returns prior values
- [x] CRUD: `put`, `get`, `delete` behave as expected
  - Tests: `src/tests/core.test.ts` pass
- [x] Iteration: can list all nodes via storage iterator
  - Verified via internal use in `vectorSearch` and mesh `sync_request` snapshot

## CRDT Conflict Resolution

- [x] Vector clock increments on each local `put`
  - Test verifies VC increments for local peer
- [x] Deterministic merge on equal timestamps, LWW on differing timestamps
  - Tests added for equal timestamps (field-wise) and newer-wins

## Subscriptions

- [x] `on(id, cb)` invoked on updates and deletes for `id`
  - Test: `subscription receives updates` passes
- [ ] `off(id, cb)` stops receiving events

## Networking (WebSocket Mesh)

- [x] Node can serve on a port and accept WebSocket connections
  - `deno run -A src/main.ts serve` prints listening URL
- [x] `sync_request` triggers a full snapshot send
  - Verified by integration test
- [x] Remote `put`/`delete` merge locally and emit subscription events

## Vector Embeddings & Search

- [x] Auto-embed vector on `put` if `data.text` or `data.content` present (or
      provided `vector` used)
- [x] `vectorSearch(query: string | number[], limit)` returns top-k by cosine
      similarity
  - Verified by tests
  - [ ] Optional ANN index integration (future): swap out brute-force index with ANN

## CLI & Tasks

- [x] `deno.json` tasks: `dev`, `test`, `fmt`, `lint`, `check`, `compile` work
  - Note: Deno warns about ignored compiler options `target`,
    `useDefineForClassFields` (non-blocking)
- [x] `deno run -A src/main.ts serve --port 8080` starts a node

## Documentation & Examples

- [x] `examples/basic-usage.ts` runs without errors
- [x] README includes quick start and API outline

## Packaging (Initial)

- [x] `deno task compile` produces a working `rusty-gun` binary
- [x] Binary can `serve` and accept WebSocket connections
- [x] Basic CRUD via compiled binary verified (scripted)

## Type System (Stage 1)

- [x] Nodes may optionally include `type` string field
- [x] Basic conventions documented (e.g., `type: "Person"`)
 - [x] Convenience helpers: `setType`, `instancesOf`

## Tests & Quality

- [x] All tests pass: `deno task test`
- [x] Code formatted and linted cleanly

## UI Phase 1 - Foundation & UX Polish ✅ COMPLETE

- [x] Component architecture (Svelte components with stores, SSE-backed cache)
- [x] Dark/light mode toggle with persistence
- [x] CodeMirror JSON editor integrated
- [x] Virtualized node list with filter
- [x] Toast notifications for user feedback
- [x] Keyboard navigation (arrow keys, Enter/Space for selection)
- [x] ARIA labels, roles, and landmark regions across all components
- [x] Sort controls (ID, Type) with visual indicators
- [x] Screen reader support (sr-only class, aria-live regions)
- [x] Editor formatting (Pretty/Compact JSON)
- [x] Copy-as-cURL functionality
- [x] Revert changes functionality with change tracking
- [x] Color contrast verification (WCAG AA compliance)
  - GitHub-inspired color palette with verified 4.5:1 contrast ratios
  - Enhanced focus indicators for keyboard navigation
  - Improved muted colors for better readability
- [x] JSON Schema validation inline in CodeMirror
  - Real-time validation as you type
  - Inline error/warning indicators
  - JSON syntax validation with position-aware errors

## Future Milestones (Not yet implemented)

- [ ] Advanced CRDT parity with HAM
- [ ] ANN index for vector search
- [ ] Rule engine (Prolog/Datalog integration)
  - [x] Minimal rule engine scaffold and basic classification rule
- [ ] Auth/Encryption (SEA-like)
- [ ] Windows Winget/MSI and Nix packaging
