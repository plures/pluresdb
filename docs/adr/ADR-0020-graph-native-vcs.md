# ADR-0020: Graph-Native Development Environment (WS-2)

**Status:** Accepted  
**Date:** 2026-08-08  
**Author:** kbristol  

## Context

Traditional version control systems (Git, Mercurial) model history as textual line-diffs
over a filesystem tree. This works for unstructured text but loses semantic information
when applied to structured data like PluresDB graphs or Praxis (.px) ASTs:

- Line-based diffs cannot express "renamed a node" vs "deleted + created a node"
- Three-way textual merge produces syntactically valid but semantically broken results
- CI-time validation catches constraint violations too late — after the commit exists
- Graph data must be flattened to files to participate in version control at all

PluresDB already stores structured graphs with CRDT semantics and reactive procedures.
Building a graph-native VCS layer on top of PluresDB avoids the semantic impedance
mismatch while preserving interop with existing Git-based tooling (IDEs, agents, CI).

## Decision

**Build graph-native version control in PluresDB; expose Git-compatible projections for interop.**

The graph IS the source of truth. Git compatibility is a projection layer, not the
internal model. We do NOT recreate filesystem-oriented git internally.

### Architecture Layers

1. **Semantic changesets** — structured diffs over the graph/AST, not textual line diffs.
   Each changeset captures node/edge additions, removals, property mutations, and
   relationship changes as first-class operations.

2. **Commit DAG** — commits are PluresDB graph nodes. Parent edges form the DAG.
   Commit metadata (author, timestamp, message, validation state) stored as node
   properties.

3. **Refs and workspaces** — branches, tags, and working copies are reactive graph
   pointers (PluresDB procedures can subscribe to ref changes).

4. **Graph-native merge** — conflict detection and resolution operates on graph
   structure (concurrent node edits, edge conflicts), not textual three-way merge.

5. **Validation-gated commits** — Praxis constraints evaluate at write-time. A commit
   that violates constraints is rejected before it enters the DAG, not after CI runs.

6. **Git projection layer** — import/export boundary that materializes the graph state
   as a filesystem tree for tools expecting a working directory. Round-trips through
   `git fast-import`/`fast-export` for interop.

### Key Constraints

- Changesets and DAG semantics MUST be expressed through WS-1 semantic core primitives
  (Praxis-lang RFC-0001) once accepted.
- Must reconcile with prior design in `pares-radix` (`ADR-0025-hyperswarm-git-forge.md`,
  `ADR-0026-self-shaping-pim.md`) — same problem space, one coherent model.
- P2P sync of commit history via Hyperswarm (consistent with existing PluresDB sync).

### Deferred Until

Implementation is explicitly deferred until WS-1 RFC-0001 (semantic core) is accepted,
since changeset/DAG semantics should be expressed through the new core primitives.

## Consequences

- PluresDB gains version-control-as-a-feature without external VCS dependencies.
- Structured diffs enable semantic conflict resolution (fewer false conflicts).
- Validation-gated commits shift correctness checks left (write-time, not CI-time).
- Git projection adds maintenance cost but preserves ecosystem compatibility.
- Implementation blocked on WS-1; this ADR captures the binding design decision only.

## References

- `praxis-lang#3` — WS-1 semantic core evolution (dependency)
- `pares-radix` commit `2e8e0aa` (2026-06-26): ADR-0025, ADR-0026, git-repo capability
- PluresDB CRDT store and reactive procedures (existing infrastructure)
