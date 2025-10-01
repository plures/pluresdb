# Roadmap (Next-Level Web UI and Product)

## Current Status (as of today)
- Core engine: CRUD, subscriptions, CRDT merge with per-field state, vector search + in-memory index, mesh sync, rules scaffold.
- CLI: serve/put/get/delete/vsearch/type/instances/list; config print/set.
- HTTP: CRUD/search/list/instances/config; SSE stream.
- UI (Svelte): Componentized (NodeList with virtualization, NodeDetail with CodeMirror JSON editor, SearchPanel, SettingsPanel), stores + SSE, dark mode toggle (persisted), toasts.
- Packaging: Dockerfile; Windows zip packaging (placeholder) via PowerShell script; MSI planned.

Phase 1 complete! Next priorities:
- Graph view, vector explorer for Phase 3.
- Type & Schema Explorer, History for Phase 2.
- Query/rules builders and ops dashboards per roadmap.

This roadmap focuses on evolving Rusty Gun from functional to delightful, inspired by modern DB UIs (Supabase Studio, Prisma Studio, Directus, Hasura Console, Neo4j Bloom, Weaviate Console, RedisInsight, MongoDB Compass).

## Phase 1 — UI Foundation & UX Polish ✅ COMPLETE
- Component Architecture: Svelte components (Explorer, Detail, Graph, Search, Settings), centralized stores, SSE-backed cache. ✅
- Styling & Theming: Pico.css with WCAG AA compliant color overrides; dark/light mode; responsive grid. ✅
- Editor: CodeMirror with JSON editing; inline schema validation; pretty/compact formatting; copy-as-cURL; revert changes. ✅
- Lists at Scale: Virtualized node list; fast filter (id/type/text), sort (ID/Type); selection via keyboard (arrow keys). ✅
- Feedback: Toasts for all actions, aria-live regions for screen readers. ✅
- Accessibility: Keyboard-first nav, ARIA labels, roles, landmarks, WCAG AA contrast ratios. ✅

Deliverables:
- Polished data explorer with reactive detail editor and saved layout. ✅
- Basic theming, dark mode toggle, and virtualized lists. ✅
- Keyboard-accessible UI with comprehensive ARIA support. ✅
- Production-ready accessibility (WCAG AA compliant). ✅
- Real-time inline JSON Schema validation. ✅

## Phase 2 — Data Modeling & Insight (2 → 4 weeks)
- Type & Schema Explorer: Visual type list; per-type schema editor (optional JSON Schema), required fields, hints.
- History & Time Travel: Per-node version history, diff, restore; audit trail.
- CRDT Inspector: Conflict viewer (field-level state, merge result), force-choose resolution when needed.
- Import/Export: CSV/JSON line-delimited; per-type mapping wizard; bulk upsert with preview.

Near-term additions:
- JSON Schema validation integrated into CodeMirror (Phase 1 spillover).
- Pretty/compact formatting and “Validate JSON” action.

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
- Packaging: Windows MSI via NSIS/WiX; Winget; Docker image; Docker Compose example with volumes. [in progress: Dockerfile; zip packaging]
- Updates: In-app update check; release channel.

Deliverables:
- Secure local auth, tokens; installer and container workflows.

## Cross-Cutting Enhancements
- Docs & Examples: Guided tours, example datasets, one-click demo.
- E2E Tests: Playwright flows (CRUD, search, rules, import/export); CI.
- Plugin Hooks: UI/engine extension points (e.g., custom embeddings, panels).

## Milestone Checklist (selected)
- ✅ UI polish: CodeMirror editor, virtualized lists, dark mode, toasts, keyboard nav, ARIA labels, sort controls, WCAG AA contrast, inline schema validation - **PHASE 1 COMPLETE**
- Schema & history: Type explorer, version diff/restore
- Graph & vector: Graph view, KNN inspector, ANN toggle (HNSW)
- Query & rules: Visual builder, scheduler, notebooks (optional)
- Ops: Mesh dashboard, storage/index manager, profiling
- Security & packaging: Auth/RBAC, MSI/Winget, Docker/Compose [in progress: Docker]
- DX & QA: Docs, tours, Playwright suites, plugin hooks

## Notes on References
- Supabase Studio/Directus for data studio patterns, policies/roles UI
- Prisma Studio for model-centric editing
- Hasura Console for schema/policy UX and saved queries
- Neo4j Bloom for graph exploration metaphors
- Weaviate/RedisInsight/Compass for vector search and key browsing patterns
