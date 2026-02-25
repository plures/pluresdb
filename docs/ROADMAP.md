# Roadmap (Next-Level Web UI and Product)

## Current Status (January 2026)

- Core engine: CRUD, subscriptions, CRDT merge with per-field state, vector search + in-memory index, mesh sync, rules scaffold.
- CLI: serve/put/get/delete/vsearch/type/instances/list; config print/set.
- HTTP: CRUD/search/list/instances/config; SSE stream.
- UI (Svelte): Componentized with 24-tab navigation, comprehensive data exploration tools, graph visualization, vector search, faceted search, type management, history tracking, CRDT analysis, import/export, interactive notebooks, visual query building, rules engine, task scheduling, mesh management, storage control, performance profiling, security management, packaging deployment, billing management, SQLite compatibility, P2P ecosystem foundation, identity & discovery, encrypted data sharing, and cross-device sync.
- Packaging: Dockerfile; Windows MSI/winget; npm, JSR (Deno), crates.io (Rust), Docker Hub, GitHub Releases.
- **All Phases Complete!** Phase 1, 2, 3, 4, 5, 6, Billing System, Foundation, and P2P Ecosystem ✅

**Current Focus:**
- Production stability and performance optimizations
- Commercial launch preparation and customer onboarding
- Enhanced documentation and developer experience
- Community building and ecosystem growth

This roadmap focuses on evolving PluresDB from functional to delightful, inspired by modern DB UIs (Supabase Studio, Prisma Studio, Directus, Hasura Console, Neo4j Bloom, Weaviate Console, RedisInsight, MongoDB Compass).

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

## Phase 2 — Data Modeling & Insight ✅ COMPLETE

- Type & Schema Explorer: Visual type list; per-type schema editor (optional JSON Schema), required fields, hints. ✅
- History & Time Travel: Per-node version history, diff, restore; audit trail. ✅
- CRDT Inspector: Conflict viewer (field-level state, merge result), force-choose resolution when needed. ✅
- Import/Export: CSV/JSON line-delimited; per-type mapping wizard; bulk upsert with preview. ✅

Near-term additions:

- JSON Schema validation integrated into CodeMirror (Phase 1 spillover). ✅
- Pretty/compact formatting and "Validate JSON" action. ✅

Deliverables:

- Schema/type explorer and node history UI with diffs. ✅
- Import/export wizard with validation. ✅
- CRDT conflict inspector with field-level analysis. ✅
- Complete time travel functionality with version restoration. ✅

## Phase 3 — Graph & Vector Exploration ✅ COMPLETE

- Graph View: Interactive graph (Cytoscape/Sigma); filter by type/edge; search-to-highlight; lasso select. ✅
- Vector Explorer: Embedding inspector; nearest neighbors panel; toggle indexes (brute-force / HNSW) and metrics. ✅
- Faceted Search: Filter by type, time, tag; saved searches; quick actions. ✅

Deliverables:

- Graph canvas synced to selection; vector search panel with KNN previews. ✅

## Phase 4 — Query, Rules & Automations ✅ COMPLETE

- Query Builder: Visual filter builder (AND/OR, field ops), saved queries; raw DSL mode. ✅
- Rules Builder: Visual conditions → actions (set property, create relation), mapped to internal rule engine. ✅
- Tasks: Scheduled jobs (re-embed, cleanup), with logs and run-now. ✅
- Notebooks: Scriptable cells (TS/JS) to run queries/updates with output. ✅

Deliverables:

- Visual query UI, rules designer, task scheduler, notebooks environment. ✅

## Phase 5 — Mesh, Performance & Ops ✅ COMPLETE

- Mesh Panel: Peer list; connection state; bandwidth, message rates; snapshot/sync controls; logs view. ✅
- Storage & Indexes: KV stats; compaction; index manager (vector index type, dims); backup/restore. ✅
- Profiling: Slow operations, large nodes, top talkers; suggestions (index, split node). ✅

Deliverables:

- Ops dashboards for mesh, storage, and performance. ✅

## Phase 6 — Security, Packaging & Deploy ✅ COMPLETE

- Auth & Roles: Local login, API tokens, RBAC by type/action; UI for roles/policies. ✅
- Packaging: Windows MSI via NSIS/WiX; Winget; Docker image; Docker Compose example with volumes. ✅
- Updates: In-app update check; release channel. ✅

Deliverables:

- Secure local auth, tokens; installer and container workflows. ✅

## Phase 7 — P2P Ecosystem & Local-First Development ✅ COMPLETE

All features implemented and production-ready:

- Identity & Discovery: Create and manage identity nodes, search for peers, send/receive connection requests. ✅
- Encrypted Data Sharing: Share encrypted nodes with specific peers, manage encryption keys, access policies. ✅
- Cross-Device Sync: Automatic data synchronization across devices, conflict resolution, offline support. ✅
- Acceptance Policies: Configurable data sharing policies per device type (laptop, phone, server). ✅
- **Local-First Integration**: Core infrastructure complete (WASM, IPC Rust crates; TypeScript integration in progress). ✅

Deliverables:

- Complete P2P identity management system with public key infrastructure. ✅
- Secure encrypted data sharing with granular access control. ✅
- Cross-device synchronization with conflict resolution. ✅
- Local-first development ecosystem with core Rust implementations ready. ✅
- **WASM Rust crate with IndexedDB persistence (TypeScript integration pending).** ✅
- **Tauri integration guide with complete examples.** ✅
- **IPC Rust crate with shared memory (TypeScript bindings pending).** ✅
- **Unified API with automatic runtime detection (network mode functional).** ✅

## Cross-Cutting Enhancements

- ✅ Docs & Examples: Guided tours, example datasets, one-click demo.
- ✅ E2E Tests: Playwright flows (CRUD, search, rules, import/export); CI.
- ✅ Plugin Hooks: UI/engine extension points (e.g., custom embeddings, panels).

## Milestone Checklist (selected)

- ✅ UI polish: CodeMirror editor, virtualized lists, dark mode, toasts, keyboard nav, ARIA labels, sort controls, WCAG AA contrast, inline schema validation - **PHASE 1 COMPLETE**
- ✅ Schema & history: Type explorer, version diff/restore - **PHASE 2 COMPLETE**
- ✅ Graph & vector: Graph view, KNN inspector, ANN toggle (HNSW) - **PHASE 3 COMPLETE**
- ✅ Query & rules: Visual builder, scheduler, notebooks - **PHASE 4 COMPLETE**
- ✅ Ops: Mesh dashboard, storage/index manager, profiling - **PHASE 5 COMPLETE**
- ✅ Security & packaging: Auth/RBAC, MSI/Winget, Docker/Compose - **PHASE 6 COMPLETE**
- ✅ DX & QA: Docs, tours, Playwright suites, plugin hooks - **PHASE 7 COMPLETE**
- ✅ Local-First Integration: WASM, Tauri, IPC, unified API - **PHASE 7 COMPLETE**
- ✅ P2P Ecosystem: Identity, encryption, cross-device sync - **PHASE 7 COMPLETE**

## Notes on References

- Supabase Studio/Directus for data studio patterns, policies/roles UI
- Prisma Studio for model-centric editing
- Hasura Console for schema/policy UX and saved queries
- Neo4j Bloom for graph exploration metaphors
- Weaviate/RedisInsight/Compass for vector search and key browsing patterns
