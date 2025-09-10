# Roadmap (Next-Level Web UI and Product)

This roadmap focuses on evolving Rusty Gun from functional to delightful, inspired by modern DB UIs (Supabase Studio, Prisma Studio, Directus, Hasura Console, Neo4j Bloom, Weaviate Console, RedisInsight, MongoDB Compass).

## Phase 1 — UI Foundation & UX Polish (Now → 2 weeks)
- Component Architecture: Svelte components (Explorer, Detail, Graph, Search, Settings), centralized stores, SSE-backed cache.
- Styling & Theming: Pico.css → custom theme; dark/light mode; responsive grid.
- Editor: Swap textarea for Monaco/CodeMirror with JSON schema validation; diff view.
- Lists at Scale: Virtualized node list; fast filter (id/type/text), sort; selection via keyboard.
- Feedback: Toasts, inline validation errors, optimistic updates, undo for delete.
- Accessibility: Keyboard-first nav, ARIA labels, contrast checks.

Deliverables:
- Polished data explorer with reactive detail editor and saved layout.
- Basic theming, dark mode toggle, and virtualized lists.

## Phase 2 — Data Modeling & Insight (2 → 4 weeks)
- Type & Schema Explorer: Visual type list; per-type schema editor (optional JSON Schema), required fields, hints.
- History & Time Travel: Per-node version history, diff, restore; audit trail.
- CRDT Inspector: Conflict viewer (field-level state, merge result), force-choose resolution when needed.
- Import/Export: CSV/JSON line-delimited; per-type mapping wizard; bulk upsert with preview.

Deliverables:
- Schema/type explorer and node history UI with diffs.
- Import/export wizard with validation.

## Phase 3 — Graph & Vector Exploration (4 → 6 weeks)
- Graph View: Interactive graph (Cytoscape/Sigma); filter by type/edge; search-to-highlight; lasso select.
- Vector Explorer: Embedding inspector; nearest neighbors panel; toggle indexes (brute-force / HNSW) and metrics.
- Faceted Search: Filter by type, time, tag; saved searches; quick actions.

Deliverables:
- Graph canvas synced to selection; vector search panel with KNN previews.

## Phase 4 — Query, Rules & Automations (6 → 8 weeks)
- Query Builder: Visual filter builder (AND/OR, field ops), saved queries; raw DSL mode.
- Rules Builder: Visual conditions → actions (set property, create relation), mapped to internal rule engine.
- Tasks: Scheduled jobs (re-embed, cleanup), with logs and run-now.
- Notebooks (Optional): Scriptable cells (TS/JS) to run queries/updates with output.

Deliverables:
- Visual query UI, rules designer v1, basic scheduler.

## Phase 5 — Mesh, Performance & Ops (8 → 10 weeks)
- Mesh Panel: Peer list; connection state; bandwidth, message rates; snapshot/sync controls; logs view.
- Storage & Indexes: KV stats; compaction; index manager (vector index type, dims); backup/restore.
- Profiling: Slow operations, large nodes, top talkers; suggestions (index, split node).

Deliverables:
- Ops dashboards for mesh, storage, and performance.

## Phase 6 — Security, Packaging & Deploy (10 → 12 weeks)
- Auth & Roles: Local login, API tokens, RBAC by type/action; UI for roles/policies.
- Packaging: Windows MSI via NSIS/WiX; Winget; Docker image; Docker Compose example with volumes.
- Updates: In-app update check; release channel.

Deliverables:
- Secure local auth, tokens; installer and container workflows.

## Cross-Cutting Enhancements
- Docs & Examples: Guided tours, example datasets, one-click demo.
- E2E Tests: Playwright flows (CRUD, search, rules, import/export); CI.
- Plugin Hooks: UI/engine extension points (e.g., custom embeddings, panels).

## Milestone Checklist (selected)
- UI polish: Monaco editor, virtualized lists, dark mode
- Schema & history: Type explorer, version diff/restore
- Graph & vector: Graph view, KNN inspector, ANN toggle (HNSW)
- Query & rules: Visual builder, scheduler, notebooks (optional)
- Ops: Mesh dashboard, storage/index manager, profiling
- Security & packaging: Auth/RBAC, MSI/Winget, Docker/Compose
- DX & QA: Docs, tours, Playwright suites, plugin hooks

## Notes on References
- Supabase Studio/Directus for data studio patterns, policies/roles UI
- Prisma Studio for model-centric editing
- Hasura Console for schema/policy UX and saved queries
- Neo4j Bloom for graph exploration metaphors
- Weaviate/RedisInsight/Compass for vector search and key browsing patterns
