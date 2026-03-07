# Phase 1 Workplan — GUN Parity First (Bit-Level Wire Compatibility)

Decision inputs (Paradox):
1) GUN parity first
2) Bit-level wire compatibility where feasible

## Phase 1 Objective
Deliver interoperable baseline so a GUN.js client can read/write against PluresDB via compatible wire + merge behavior.

## Scope (P1)
- Wire codec: canonical `get/put/ack` message shapes and field semantics
- Merge engine: HAM-compatible field-level conflict resolution behavior
- Relay: websocket compatibility endpoint for legacy-style clients
- Contract tests: golden vectors against captured GUN traffic

## Out of Scope (P1)
- Hyperswarm productionization
- Full SEA key lifecycle
- Procedure runtime and vector optimization

## Module Tasks

### A. `pluresdb-wire-gun` (new crate)
- Define message structs preserving GUN wire field names/order where required
- Add strict parser + canonical serializer
- Include compatibility mode for legacy edge cases

Deliverables:
- `crates/pluresdb-wire-gun`
- golden encode/decode tests

### B. `pluresdb-crdt` (new/isolated module)
- Implement HAM decision function with deterministic outcomes
- Field-level merge parity tests

Deliverables:
- HAM compatibility test vectors
- deterministic merge invariants

### C. Relay compatibility endpoint
- Add websocket endpoint accepting GUN-compatible messages
- Wire ↔ internal conversion with explicit compatibility mapping

Deliverables:
- e2e smoke test with JS GUN client fixture

## Test Strategy
- Contract tests:
  - decoder/encoder round-trip
  - canonical `ack` behavior
  - merge outcomes across concurrent updates
- Interop tests:
  - JS GUN client performs get/put against relay endpoint
- Regression harness:
  - replay packet corpus from legacy TS + GUN captures

## Exit Criteria
- JS GUN client read/write succeeds against PluresDB relay
- Wire compatibility tests green in CI
- HAM parity vectors pass deterministically

## Next Step (immediate)
1. Create `pluresdb-wire-gun` crate scaffold
2. Add first message schema + round-trip tests
3. Add fixture directory `tests/fixtures/gun-wire/`
