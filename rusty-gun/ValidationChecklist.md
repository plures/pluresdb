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
- [ ] Basic CRUD via compiled binary verified (scripted)

## Type System (Stage 1)

- [x] Nodes may optionally include `type` string field
- [x] Basic conventions documented (e.g., `type: "Person"`)

## Tests & Quality

- [x] All tests pass: `deno task test`
- [x] Code formatted and linted cleanly

## Future Milestones (Not yet implemented)

- [ ] Advanced CRDT parity with HAM
- [ ] ANN index for vector search
- [ ] Rule engine (Prolog/Datalog integration)
- [ ] Auth/Encryption (SEA-like)
- [ ] Windows Winget/MSI and Nix packaging
