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
- [x] Unit tests cover core functionality (CRUD, subscriptions, vector search)
- [x] Integration tests cover mesh networking and API server
- [x] Performance tests validate throughput and memory usage
- [x] Security tests prevent injection attacks and validate input
- [x] Test coverage reporting configured and working
- [x] Benchmark suite for performance monitoring
- [x] Memory leak detection and prevention
- [x] Concurrent operation testing
- [x] Error handling and edge case testing

## VSCode Extension

- [x] Extension compiles without TypeScript errors
- [x] Extension packages successfully with vsce
- [x] ESLint configuration working (CommonJS format)
- [x] Extension activates without module errors
- [x] All VSCode API calls use correct types (vscode.Command objects)
- [x] Package size optimized with .vscodeignore
- [x] Repository and license metadata included

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

## UI Phase 2 - Data Modeling & Insight

- [x] Type & Schema Explorer
  - Visual type list with instance counts
  - Per-type schema editor with JSON Schema validation
  - Schema save/delete functionality
  - Type creation with sample instances
  - Real-time schema validation with error display
  - Responsive grid layout (types list + details)
  - Accessibility: ARIA labels, keyboard navigation

- [x] History & Time Travel
  - Per-node version history with timestamps
  - Visual diff between versions
  - Version restoration capability
  - Vector clock and field state inspection
  - Time-based version navigation
  - Metadata display for each version
  - Responsive grid layout (versions + details)
  - Accessibility: ARIA labels, keyboard navigation

- [x] CRDT Inspector
  - Conflict detection and visualization
  - Field-level state inspection
  - Vector clock analysis
  - Merge information display
  - Conflict resolution tools (UI ready)
  - Raw node data inspection
  - Responsive grid layout for different views
  - Accessibility: ARIA labels, keyboard navigation

- [x] Import/Export Wizard
  - JSON and CSV export formats
  - Type-based data filtering
  - CSV field mapping interface
  - Data preview and validation
  - Download and clipboard copy functionality
  - Batch import with progress tracking
  - Tab-based interface (Export/Import)
  - Accessibility: ARIA labels, keyboard navigation

## UI Phase 3 - Graph & Vector Exploration

- [x] Interactive Graph View
  - Cytoscape.js integration with multiple layout algorithms
  - Type-based filtering and color coding
  - Search-to-highlight functionality
  - Lasso selection mode
  - Node and edge interaction
  - Export to PNG functionality
  - Responsive design with mobile support

- [x] Vector Explorer
  - Vector search with similarity scoring
  - Embedding inspector with statistics
  - Nearest neighbors panel
  - Vector visualization and analysis
  - Export vector data functionality
  - Index type selection (Brute-force, HNSW future)
  - Real-time vector statistics

- [x] Faceted Search
  - Multi-faceted filtering (Type, Time, Tags, Text, Date Range)
  - Saved searches with persistence
  - Quick actions for common operations
  - Export search results
  - Advanced text search across all data
  - Time-based filtering with predefined ranges
  - Tag-based filtering and extraction

- [x] Graph Canvas Integration
  - Graph canvas synced to node selection
  - Vector search panel with KNN previews
  - Interactive node selection and highlighting
  - Real-time graph updates
  - Cross-component data synchronization

## UI Phase 4 - Query, Rules & Automations + Notebooks

- [x] Interactive Notebooks
  - Scriptable cells with JavaScript/TypeScript execution
  - Markdown cells for documentation and notes
  - Code execution with API access and sandboxed environment
  - Output display with formatted results
  - Cell management (add, delete, move up/down)
  - Notebook persistence with localStorage
  - Import/Export functionality for notebooks
  - Real-time execution with status indicators
  - Default welcome notebook with examples

- [x] Visual Query Builder
  - Visual query builder with drag-and-drop interface
  - AND/OR operations for complex queries
  - Field operations (equals, contains, starts with, etc.)
  - Saved queries with persistence
  - Raw DSL mode for advanced users
  - Query execution with results display
  - Export/Import functionality
  - Real-time query building with visual feedback

- [x] Rules Builder
  - Visual conditions → actions interface
  - Property setting and relation creation
  - Rule engine integration
  - Rule testing and validation
  - Rule management with enable/disable
  - Rule execution with logging
  - Export/Import functionality

- [x] Tasks Scheduler
  - Scheduled jobs (re-embed, cleanup, backup, custom)
  - Task logs and run-now functionality
  - Job management and monitoring
  - Automation workflows
  - Cron-like scheduling with presets
  - Real-time task monitoring
  - Export/Import functionality

## UI Phase 5 - Mesh, Performance & Ops

- [x] Mesh Panel
  - Peer list with connection state monitoring
  - Bandwidth and message rate tracking
  - Snapshot creation and management
  - Synchronization controls with progress tracking
  - Mesh logs with real-time monitoring
  - Auto-refresh functionality
  - Peer connection management (connect/disconnect)

- [x] Storage & Indexes Dashboard
  - Storage statistics with usage visualization
  - Index management (vector, text, numeric, composite)
  - Performance metrics for each index
  - Backup/restore functionality (full and incremental)
  - Compaction control with progress tracking
  - Storage usage visualization with progress bars
  - Index creation and deletion

- [x] Profiling Dashboard
  - Slow operations tracking with detailed information
  - Large nodes identification with access patterns
  - Top talkers monitoring with bandwidth and message counts
  - Performance suggestions with priority levels and actions
  - Tabbed interface for organized data viewing
  - Real-time monitoring with auto-refresh
  - Suggestion management (apply/dismiss)

## UI Phase 6 - Security, Packaging & Deploy

- [x] Security & Authentication
  - User management with role-based access control
  - Role management with permission assignment
  - Policy management with resource-based access control
  - API token management with expiration and revocation
  - Security settings with session timeout and password policies
  - RBAC implementation by type/action
  - Two-factor authentication support
  - API rate limiting configuration

- [x] Packaging & Deployment
  - [x] Docker containerization with image building and management
    - Multi-stage Dockerfile with optimized production image
    - Docker Compose configurations for development and production
    - Health check script for container monitoring
    - Nginx reverse proxy configuration for production
    - Easy-to-use run scripts for Windows and Unix systems
    - Comprehensive Docker documentation and examples
  - [x] Windows MSI packaging with installer creation
  - [x] Winget package preparation and publishing
  - [x] Update management with in-app update checking
  - [x] Deployment management with environment control
  - [x] Build logs with real-time progress tracking
  - [x] Health monitoring with status checks

## UI Billing System - Payment & Billing Management

- [x] Subscription Management
  - Plan selection with Free, Pro, and Enterprise tiers
  - Pricing tiers with monthly and yearly billing cycles
  - Plan features with detailed feature lists and limits
  - Subscription status tracking (active, cancelled, past_due, trialing)
  - Plan changes with seamless upgrades and downgrades
  - Cancellation management with end-of-period cancellation
  - Reactivation of cancelled subscriptions

- [x] Payment Processing
  - Multiple payment methods (Credit Card, Bank Account, PayPal)
  - Payment method management with add/remove functionality
  - Default payment method selection
  - Card information with masked display and expiry dates
  - Payment security with secure tokenization
  - Payment method validation with real-time checks

- [x] Usage Tracking & Metered Billing
  - Resource monitoring for nodes, storage, users, and API calls
  - Usage visualization with progress bars and percentage indicators
  - Limit enforcement with real-time usage tracking
  - Overage alerts when approaching limits
  - Unlimited plans support for enterprise customers
  - Bandwidth tracking for network usage monitoring

- [x] Invoice Management
  - Invoice generation with automatic billing
  - Invoice status tracking (paid, pending, failed, draft)
  - Invoice download with PDF generation
  - Payment history with detailed transaction records
  - Due date management with automated reminders
  - Invoice numbering with sequential numbering system

- [x] Billing Analytics
  - Revenue dashboard with monthly and total revenue tracking
  - Subscription metrics with active subscription counts
  - Churn rate analysis with customer retention metrics
  - ARPU tracking (Average Revenue Per User)
  - Growth rate analysis with percentage growth tracking
  - Business intelligence for data-driven decisions

## UI Phase 7 - P2P Ecosystem & Local-First Development

- [x] Identity & Discovery
  - [x] Identity node creation and management
  - [x] Public key generation and management
  - [x] Peer search and discovery interface
  - [x] Connection request system (send/receive/accept/reject)
  - [x] Peer profile management (name, email, location, tags)
  - [x] Acceptance policy configuration per device type
  - [x] Real-time peer status monitoring

- [x] Encrypted Data Sharing
  - [x] Node-level encryption with target public keys
  - [x] Encryption key management (RSA, ECDSA, Ed25519)
  - [x] Access policy creation and management
  - [x] Data sharing workflow (share/accept/reject/revoke)
  - [x] Sharing history and audit trail
  - [x] Conflict resolution for shared data
  - [x] Granular access control (read-only, read-write, admin)

- [x] Cross-Device Sync
  - [x] Automatic device discovery and connection
  - [x] Real-time data synchronization
  - [x] Conflict detection and resolution
  - [x] Offline operation queuing
  - [x] Sync status monitoring and metrics
  - [x] Device management (add/remove/connect/disconnect)
  - [x] Sync settings and policy configuration
  - [x] Performance metrics and bandwidth monitoring

- [x] Local-First Development
  - [x] Offline-first data storage
  - [x] Local operation queuing
  - [x] Automatic sync when online
  - [x] Conflict resolution strategies
  - [x] Data integrity validation
  - [x] Backup and restore capabilities
  - [x] Cross-platform compatibility

## Future Milestones (Not yet implemented)

- [ ] Advanced CRDT parity with HAM
- [ ] ANN index for vector search
- [ ] Rule engine (Prolog/Datalog integration)
  - [x] Minimal rule engine scaffold and basic classification rule
- [ ] Auth/Encryption (SEA-like)
- [x] Windows Winget/MSI and Nix packaging
