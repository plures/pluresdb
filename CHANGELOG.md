## [2.16.2] — 2026-03-30

- fix(ci): auto-fix Copilot PR titles to follow Conventional Commits format (#263) (ade7f2a)

## [2.16.1] - 2026-03-30

### Fixed

- resolve conventional commit title check failure (#261)


## [2.16.0] — 2026-03-29

- feat: performance benchmark suite for CRUD/query/sync (#256) (91a2a31)
- fix(ci): resolve conventional commit title check failure (#261) (bee772a)
- chore(deps-dev): bump typescript from 5.9.3 to 6.0.2 (#260) (2843b1c)
- chore(deps): bump @plures/praxis from 2.4.35 to 2.4.39 (#259) (67036ec)

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial changelog

## [2.15.21] - 2026-03-28

### Developer Experience

- dual license — Rust crates BSL 1.1, TypeScript MIT


## [2.15.20] - 2026-03-28

### Developer Experience

- standardize license to MIT
- standardize license to MIT


## [2.15.19] - 2026-03-28

### Fixed

- npm audit fix — update path-to-regexp (high severity)


## [2.15.18] - 2026-03-28

### Developer Experience

- standardize copilot-pr-lifecycle.yml to canonical version


## [2.15.17] - 2026-03-27

### Documentation

- API reference + migration guides for Node/Deno/Rust (#255)


## [2.15.16] - 2026-03-27

### Documentation

- improve api-documented coverage to ≥90% (#236)


## [2.15.15] - 2026-03-27

### Developer Experience

- standardize changelog format


## [2.15.14] - 2026-03-27

### Developer Experience

- standardize CI workflow


## [2.15.13] - 2026-03-27

### Fixed

- lint-clean 0% → 100% — expand ESLint scope to full codebase (#233)


## [2.15.12] - 2026-03-27

### Fixed

- downgrade typescript to ^5.8.0 to unblock CI and release pipeline (#231)

### Developer Experience

- bump proc-macro2 from 1.0.101 to 1.0.106 (#230)
- bump tempfile from 3.23.0 to 3.27.0 (#228)
- bump serde_json from 1.0.145 to 1.0.149 (#227)
- bump clap from 4.5.48 to 4.5.60 (#226)
- bump uuid from 1.18.1 to 1.23.0 (#224)
- bump axum from 0.8.6 to 0.8.8 (#225)
- bump hyperswarm from 4.16.0 to 4.17.0 (#223)
- bump typescript from 5.9.3 to 6.0.2 (#222)
- bump rusqlite from 0.32.1 to 0.39.0 (#221)
- bump ws from 8.18.3 to 8.20.0 (#220)
- bump fastembed from 5.11.0 to 5.13.0 (#219)
- bump @types/vscode from 1.104.0 to 1.110.0 (#218)
- bump prettier-plugin-svelte from 3.4.0 to 3.5.1 (#217)
- bump thiserror from 1.0.69 to 2.0.17 (#216)
- bump @types/node from 22.19.11 to 25.5.0 (#215)
- bump @plures/praxis from 1.4.0 to 2.4.35 (#214)
- bump cors from 2.8.5 to 2.8.6 (#213)


## [2.15.11] - 2026-03-27

### Developer Experience

- bump prettier from 3.6.2 to 3.8.1 (#212)
- bump @types/express from 5.0.5 to 5.0.6 (#211)


## [2.15.10] - 2026-03-27

### Fixed

- remediate no-known-vulns dimension — dependabot, audit CI, bincode 1→2 (#207)
- lint-clean 0% → 100% (ESLint + Clippy) (#210)


## [2.15.9] - 2026-03-26

### Developer Experience

- apply org-standard automation files (#205)


## [2.15.8] - 2026-03-26

### Fixed

- remediate brace-expansion ReDoS/DoS vulnerabilities in dev toolchain (GHSA-f886-m6hf-6m8v) (#206)


## [2.15.7] - 2026-03-26

### Documentation

- improve api-documented coverage from 0% → 90%+ (#202)


## [2.15.6] - 2026-03-26

### Fixed

- bump ajv 6.12.6 → 6.14.0 (GHSA-2g4f-4pwh-qvx6 ReDoS) (#199)


## [2.15.5] - 2026-03-26

### Developer Experience

- auto-close stale ci-feedback issues for merged PRs (#198)


## [2.15.4] - 2026-03-26

### Developer Experience

- auto-correct PR titles that fail conventional commit check (#196)


## [2.15.3] - 2026-03-26

### Other

- [WIP] Fix CI failures on PR #190 (#193)


## [2.15.2] - 2026-03-26

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 1 update (#195)


## [2.15.1] - 2026-03-26

### Documentation

- improve api-documented dimension from 0% → 90% (#190)


## [2.15.0] - 2026-03-25

### Added

- add auto-approve workflow for Copilot runs
- add Copilot PR Lifecycle workflow with level/strategic buckets


## [2.14.1] - 2026-03-21

### Developer Experience

- bump rustls-webpki in the cargo group across 1 directory (#180)


## [2.14.0] - 2026-03-21

### Added

- Adopt @plures/praxis for declarative logic management (#179)


## [2.13.0] - 2026-03-21

### Added

- add hybrid storage backend with ObjectBridge (#165)


## [2.12.1] - 2026-03-20

### Developer Experience

- bump flatted (#177)


## [2.12.0] - 2026-03-20

### Added

- AI-specific procedures — decision audit, RL extraction, self-tuning (#167)


## [2.11.0] - 2026-03-19

### Added

- cognitive architecture step types (#163)


## [2.10.0] - 2026-03-18

### Added

- improve repository best practices compliance (40% → 90%+) (#151)

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 1 update (#152)


## [2.9.10] - 2026-03-10

### Documentation

- fix npm package references and clarify version channels


## [2.9.9] - 2026-03-07

### Developer Experience

- add PR lane event relay to centralized merge FSM


## [2.9.8] - 2026-03-06

### Developer Experience

- bump bytes in the cargo group across 1 directory (#149)


## [2.9.7] - 2026-03-06

### Fixed

- add dist/ and scripts/postinstall.js to npm package files whitelist (#148)


## [2.9.6] — 2026-03-04



## [2.9.5] — 2026-03-04

- chore(release): 2.9.4 (368ab69)
- fix(release): add per-branch concurrency lock to prevent duplicate tag races (e9127cc)

## [2.9.4] - 2026-03-04

### Fixed

- add per-branch concurrency lock to prevent duplicate tag races


## [2.9.3] — 2026-03-04



## [2.9.2] — 2026-03-04



# Changelog

## [2.9.1] - 2026-03-04

### Fixed

- use github.event.inputs for push-compatible reusable release call


## [2.9.0] - 2026-03-03

### Added

- durable event contracts for Praxis analysis lifecycle (#146)


## [2.8.0] - 2026-03-02

### Added

- PluresDB/pluresLM architectural separation + Rust GUN.js networking (#144)


## [2.7.0] - 2026-03-02

### Added

- GUN.js wire-compat SEA, RAD adapter, blob CAS, git replication protocol (#142)


## [2.6.0] - 2026-03-01

### Added

- add training data processing procedures (#140)


## [2.5.6] - 2026-03-01

### Changed

- make embeddings + vector index eventual (async), not in put() hot path (#139)


## [2.5.5] - 2026-03-01

### Fixed

- add explicit `none` option to workflow_dispatch bump input (#137)


## [2.5.4] - 2026-03-01

### Fixed

- harden org release workflow migration (#135)

### Developer Experience

- disable automatic triggers on deprecated publish workflows (#132)


## [2.5.3] - 2026-03-01

### Fixed

- add permissions to release.yml caller workflow (#133)


## [2.5.2] - 2026-02-28

### Developer Experience

- migrate pluresdb to org release workflow (#129)


## [2.5.1] - 2026-02-28

### Fixed

- release pipeline should publish reliably (#128)


## [2.5.0] - 2026-02-28

### Added

- Rust sync transport + minimal GUN-compatible wire protocol (phase 1) (#127)


## [2.4.0] - 2026-02-28

### Added

- add document storage and enrichment procedures (#125)


## [2.3.1] - 2026-02-27

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 3 updates (#122)


## [2.3.0] - 2026-02-26

### Added

- Pares Agens procedure execution API in pluresdb-procedures (#120)

### Developer Experience

- repo cleanup (#118)


## [2.2.0] - 2026-02-25

### Added

- Implement Phase 2B — graph clustering, path finding, PageRank, and stats (#117)


## [2.1.3] - 2026-02-25

### Other

- Add Phase 2A graph operations and comprehensive test coverage (#115)


## [2.1.2] - 2026-02-25

### Fixed

- NAPI binary build with embeddings support (#113)


## [2.1.1] - 2026-02-24

### Other

- cargo fix --lib -p pluresdb-storage


## [2.1.0] - 2026-02-24

### Added

- implement pluresdb-procedures crate — Phase 1 (DSL parser + core ops) (#111)


## [2.0.0] - 2026-02-24

### Other

- feat!: Remove SQLite dependency — wire CrdtStore to pluresdb-storage (v2.0.0) (#109)


## [2.0.0] - 2026-02-24

### BREAKING CHANGES

- **`CrdtStore::with_persistence` now accepts `Arc<dyn StorageEngine>`** instead
  of `Arc<Database>`.  The method now returns `Self` (infallible) instead of
  `Result<Self, DatabaseError>`.
- **`rusqlite` is no longer a default dependency** of `pluresdb-core`.  The
  `Database`, `SqlValue`, `QueryResult`, `ExecutionResult`, `DatabasePath`,
  `DatabaseOptions`, and `DatabaseError` types are only available when the
  `sqlite-compat` cargo feature is enabled.
- **`pluresdb-node`**: `query()` and `exec()` methods return an error at runtime
  unless the crate is compiled with the `sqlite-compat` feature.  The
  constructor now opens a sled store instead of a SQLite file when `db_path`
  is provided.
- **Version bumped from 1.15.0 → 2.0.0**.

### Added

- `sqlite-compat` feature flag in `pluresdb-core`, `pluresdb-node`,
  `pluresdb-cli`, and `pluresdb` umbrella crate.
- `pluresdb migrate-from-sqlite --source <path> --target <dir>` CLI command
  (requires `sqlite-compat` feature) to migrate v1.x SQLite databases to sled.
- New tests in `pluresdb-core` covering storage-engine-backed persistence
  (`MemoryStorage`) without requiring SQLite.
- `MIGRATION.md` upgrade guide for v1.x → v2.0 consumers.

### Changed

- `CrdtStore` persistence layer now uses `pluresdb-storage::StorageEngine`
  (sled-backed) instead of a SQLite `Database`.
- `MemoryStorage` now uses `parking_lot::RwLock` for synchronous interior
  access, eliminating the tokio runtime dependency from storage reads/writes.
- `SledStorage` now uses synchronous `sled::Db::flush()` instead of
  `flush_async()`.
- README updated: removed "SQLite Compatibility" from tagline; updated
  architecture table and quick-start example.
- Workspace and crate descriptions updated to remove "SQLite Compatibility".

### Removed

- `SQLITE_ACTOR` constant from `pluresdb-core` (no longer needed — full
  `NodeRecord` is serialised and deserialised from storage).

---

## [Unreleased]

### Breaking Changes

- ⚠️ **Renamed `GunDB` class to `PluresDB`** - The primary database class in `legacy/core/database.ts` has been renamed from `GunDB` to `PluresDB`. Consumers must update their imports: `import { PluresDB } from "pluresdb"`. A deprecated `GunDB` re-export is available for one major version to ease migration.
- Renamed `demo/gun-comparison.html` to `demo/benchmark-comparison.html`.

### Added

- ✅ **Complete implementation of pluresdb-node** - Full Node.js bindings with N-API
  - Complete CRUD operations
  - SQL query support (query, exec)
  - Metadata access (getWithMetadata)
  - Type filtering (listByType)
  - Text search with scoring
  - Vector search placeholder
  - Database statistics
  - Subscription infrastructure
  - Comprehensive TypeScript definitions
  - Full test suite

- ✅ **Complete implementation of pluresdb-deno** - Full Deno bindings with deno_bindgen
  - Complete CRUD operations
  - SQL query support (query, exec)
  - Metadata access (getWithMetadata)
  - Type filtering (listByType)
  - Text search with scoring
  - Vector search placeholder
  - Database statistics
  - SyncBroadcaster integration
  - Automatic TypeScript bindings generation
  - Comprehensive test suite

- ✅ **pluresdb-storage** - Ready for publishing
  - Complete storage abstraction layer
  - MemoryStorage and SledStorage backends
  - Encryption support
  - WAL and replay system
  - Full documentation

- ✅ **pluresdb-cli** - Ready for publishing
  - Complete command-line interface
  - All CRUD operations
  - SQL query execution
  - Search and vector search
  - Type system commands
  - Network commands
  - Configuration management
  - Maintenance commands
  - API server with Axum
  - Full documentation

- Added crates.io publishing workflow for Rust crates (pluresdb-core, pluresdb-storage, pluresdb-sync, pluresdb-cli)
- Added CARGO_REGISTRY_TOKEN configuration to release workflow
- Added comprehensive publishing guide (crates/PUBLISHING_GUIDE.md)
- Added README files for all crates

### Changed

- Updated release workflow to publish to crates.io in addition to npm, Docker Hub, JSR, and GitHub Releases
- Synchronized version across package.json (1.3.8), Cargo.toml (1.3.8), and deno.json (1.3.8)
- Updated RELEASE_PROCESS.md to document crates.io publishing workflow
- Updated README.md with crates.io badge and installation instructions
- Enhanced release channel documentation to include all platforms: npm, crates.io, JSR, Docker Hub, winget, and GitHub Releases
- Updated CI workflow to sync deno.json version during automated version bumps

### Fixed

- Fix release workflow entry point paths (legacy/main.ts)
- Fix Dockerfile to use correct entry point
- Fix Windows binary compilation in release workflow
- Add error checking for binary compilation failures
- Fix compilation error in pluresdb-cli (Result type in closure)

## [1.15.0] - 2026-02-24

### Added

- eliminate startup hydration — query SQLite directly from CrdtStore (#107)


## [1.14.1] - 2026-02-24

### Fixed

- add ./embedded export to package.json


## [1.14.0] - 2026-02-24

### Added

- add SQLite persistence to CrdtStore


## [1.13.0] - 2026-02-23

### Added

- expose embed() and embeddingDimension() via NAPI bindings


## [1.12.3] - 2026-02-23

### Fixed

- FastEmbedder compile errors — Mutex for interior mutability, manual Debug impl


## [1.12.2] - 2026-02-23

### Developer Experience

- remove transient progress/completion markdown from root and docs/ (#97)


## [1.12.1] - 2026-02-23

### Documentation

- comprehensive documentation overhaul — architecture, API reference, getting started (#99)


## [1.12.0] - 2026-02-23

### Added

- make pluresdb-node NAPI bindings the primary Node.js interface (#105)


## [1.11.1] - 2026-02-23

### Fixed

- resolve pluresdb-wasm IndexedDB web-sys API compatibility issues (#103)

### Other

- breaking: rename GunDB → PluresDB across the codebase (#95)


## [1.11.0] - 2026-02-22

### Added

- auto-embedding on insert via pluggable EmbedText backend (fastembed, feature-gated) (#93)


## [1.10.0] - 2026-02-21

### Added

- native HNSW vector index and similarity search (#92)


## [1.9.9] - 2026-02-21

### Developer Experience

- bump the npm_and_yarn group across 2 directories with 1 update (#89)


## [1.9.8] - 2026-02-18

### Other

- Add Transport trait abstraction for P2P sync with hyperswarm-rs integration points (#87)


## [1.9.7] - 2026-02-17

### Other

- Complete Rust core migration for V2.0 - 10x performance, 80% memory reduction (#85)


## [1.9.6] - 2026-02-17

### Other

- Skip Hyperswarm P2P tests in CI environments (#84)


## [1.9.5] - 2026-02-16

### Fixed

- Azure Credentials Not Configured - Scheduled Tests Skipped #81


## [1.9.4] - 2026-02-16

### Other

- Skip Hyperswarm test in Deno to fix release pipeline (#80)


## [1.9.3] - 2026-02-16

### Documentation

- add comprehensive DESIGN.md and ROADMAP.md


## [1.9.2] - 2026-02-14

### Other

- Fix TypeScript type errors and Deno compatibility issues blocking release (#78)


## [1.9.1] - 2026-02-14

### Developer Experience

- bump qs in the npm_and_yarn group across 1 directory (#76)

### Other

- Build and publish @plures/pluresdb-native N-API binary (#75)


## [1.9.0] - 2026-02-14

### Added

- P2P sync transport via Hyperswarm (DHT discovery + NAT holepunching) (#73)


## [1.8.0] - 2026-02-14

### Added

- pluggable sync transports — Azure relay (WSS:443), Vercel relay, Hyperswarm direct (#72)


## [1.7.1] - 2026-02-10

### Other

- Fix Azure relay tests authentication detection (#67)


## [1.7.0] - 2026-02-09

### Added

- add embedded export for pure embedded database usage (#69)


## [1.6.11] - 2026-01-30

### Other

- Add PowerShell and Bash modules for command history tracking with P2P sync (#66)


## [1.6.10] - 2026-01-26

### Other

- Fix npm publish failures from Deno/TypeScript import incompatibility (#64)


## [1.6.9] - 2026-01-26

### Fixed

- npm publish failures in unified-api.ts (#62)


## [1.6.8] - 2026-01-26

### Other

- Fix Deno compilation errors: process global access and duplicate exports (#60)


## [1.6.7] - 2026-01-26

### Other

- Initial plan (#58)


## [1.6.6] - 2026-01-25

### Fixed

- Remove duplicate closing button tag in ExampleDatasets.svelte (#56)


## [1.6.5] - 2026-01-25

### Other

- Complete local-first integration roadmap and refactor README.md (#54)


## [1.6.4] - 2026-01-25

### Other

- Document accurate local-first integration status: 90% complete (Rust done, TS integration pending) (#52)


## [1.6.3] - 2026-01-25

### Other

- Complete local-first integration: WASM IndexedDB, IPC shared memory, Tauri demos (#50)


## [1.6.2] - 2026-01-25

### Other

- Add WASM and IPC integration infrastructure for local-first operation (#48)


## [1.6.1] - 2026-01-25

### Other

- Add local-first integration API with runtime auto-detection (#46)


## [1.6.0] - 2026-01-25

### Added

- Complete project roadmap with guided tour, example datasets, E2E tests, and plugin system (#44)


## [1.5.8] - 2026-01-25

### Other

- Update roadmap with checklist items for enhancements


## [1.5.7] - 2026-01-25

### Other

- Update roadmap with optional notebooks for queries


## [1.5.6] - 2026-01-25

### Fixed

- gracefully skip Azure relay tests when credentials not configured + migrate to OIDC (#41)


## [1.5.5] - 2026-01-25

### Developer Experience

- bump lodash (#40)


## [1.5.4] - 2026-01-11

### Other

- Fix Azure Login authentication in relay tests workflow (#36)


## [1.5.3] - 2026-01-11

### Fixed

- Remove path dependencies from pluresdb crate for publishing


## [1.5.2] - 2026-01-11

### Other

- [WIP] Fix release-check errors in CI job (#38)


## [1.5.1] - 2026-01-11

### Documentation

- Add release trigger instructions

### Developer Experience

- Bump version to 1.4.3 for new release
- publish instr
- Add pluresdb unified crate to release workflow publishing order

### Other

- Release ready for publishing


## [1.5.0] - 2026-01-11

### Added

- Complete all missing crate implementations and add unified pluresdb crate


## [1.4.2] - 2026-01-10

### Other

- Update dependencies for create-github-release step


## [1.4.1] - 2026-01-10

### Other

- Update npm badge to reflect new scope


## [1.4.0] - 2026-01-10

### Added

- add crates.io publishing and synchronize versions across all manifests (#34)


## [1.3.8] - 2026-01-10

### Other

- Complete Pre-flight Hardening: PluresDB as Exclusive Local-First Agent Memory Store (#29)


## [1.3.7] - 2026-01-10

### Other

- Fix Azure Login authentication format for azure/login@v1 (#30)


## [1.3.6] - 2026-01-10

### Other

- Add Copilot instructions for repository (#32)


## [1.3.5] - 2026-01-10

### Developer Experience

- bump qs in the npm_and_yarn group across 1 directory (#26)


## [1.3.4] - 2025-12-29

### Other

- Change package name to @plures/pluresdb


## [1.3.3] - 2025-12-29

### Other

- Add Azure infrastructure automation and P2P relay testing (#25)


## [1.3.2] - 2025-12-27

### Other

- Sync versions to 1.3.1 and update README to reference changelog (#23)


## [1.3.1] - 2025-12-27

### Other

- Bump version from 1.0.1 to 1.3.0


## [1.3.0] - 2025-12-25

### Added

- implement Node.js N-API bindings for Rust core


## [1.2.10] - 2025-12-16

### Other

- Implement missing P2P API methods from README (#21)


## [1.2.9] - 2025-12-16

### Other

- Align README with current release channels and monorepo structure (#19)


## [1.2.8] - 2025-12-16

### Developer Experience

- bump body-parser (#17)


## [1.2.7] - 2025-11-15

### Developer Experience

- bump js-yaml (#16)


## [1.2.6] - 2025-11-15

### Other

- Fix formatting in dependabot.yml


## [1.2.5] - 2025-11-14

### Developer Experience

- update Cargo.lock to version 1.2.4


## [1.2.4] - 2025-11-14

### Other

- Add secrets configuration documentation


## [1.2.3] - 2025-11-14

### Other

- Fix GitHub Actions workflow: Replace invalid secret checks with step outputs


## [1.2.2] - 2025-11-14

### Fixed

- correct release workflow paths and improve reliability


## [1.2.1] - 2025-11-14

### Fixed

- correct entry point paths and improve release workflow reliability


## [1.2.0] - 2025-11-14

### Added

- Complete P2P Ecosystem & Comprehensive Packaging System
- Complete Phase 1 UI with WCAG AA accessibility and inline schema validation

### Developer Experience

- capture alt imp
- capture alt imp
- attribute
- checkpoint
- release 1.0.1
- apply merge-driven renames/branding (rusty-gun → pluresdb) and update packaging scripts
- remove tracked build artifacts (target/) and update .gitignore
- checkpoint
- reorg
- checkpoint
- plan
- checkpoint

### Other

- Fix release tag push by separating commit and tag operations (#15)
- Automate version bumping, tagging, and multi-platform releases (#13)
- Upgrade to Deno 2.x and latest package versions (#11)
- Complete the rust implementation (#9)
- Remove legacy rusty-gun directory and rebrand to PluresDB (#6)
- Fix CI failures: deno formatting, lint configuration, and test workflow (#8)
- Merge pull request #4 from plures/copilot/develop-personal-database
- Revert legacy/cli.ts to use Node.js APIs instead of Deno APIs
- Fix Deno lint errors: replace process with Deno, fix unused error vars, remove unnecessary async
- Fix CI failures: add npm lint/fmt scripts and upgrade Deno to v2.x
- Add issue response document summarizing Windows personal database readiness
- Add comprehensive Windows personal database status document
- Add Windows-specific documentation and launcher files
- Fix TypeScript compilation errors and add Windows documentation
- Initial plan
- Merge branch 'feature/better-sqllite3-support' of https://github.com/plures/pluresdb
- Merge pull request #1 from plures/copilot/restructure-pluresdb-project
- Add restructuring summary document
- Update config files to reference legacy directory instead of src
- Restructure project: move src to legacy, create pluresdb-node and pluresdb-deno crates
- Initial plan
- Merge branch 'main' of https://github.com/kayodebristol/rusty-gun
- Resolve merge conflicts: accept current branch (ours) for all files
- Merge pull request #5 from kayodebristol/revert-3-copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Revert "[WIP] Rename nested 'rusty-gun' folder to 'pluresdb' and update references"
- Merge pull request #4 from kayodebristol/copilot/fix-db1e13fe-eea5-47d2-8ddf-18e1b5f69493
- Create five GitHub Actions workflows as specified
- Initial plan
- Merge pull request #3 from kayodebristol/copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Initial plan
- Merge pull request #2 from kayodebristol/copilot/fix-edf5eacd-b439-43e7-af90-57eac7d6efb7
- Complete PluresDB rebrand: update packaging files, env vars, and Svelte components
- Update all branding references from rusty-gun to PluresDB throughout the codebase
- Remove .githooks, azure directories and update core branding to PluresDB
- Initial plan
- Phase 2 Complete: Data Modeling & Insight
- checkpoint
- new roadmap
- new reactive ui
- Checkpoint Continue Deno version
- Initial
- first commit


## [1.1.0] - 2025-11-14

### Added

- Complete P2P Ecosystem & Comprehensive Packaging System
- Complete Phase 1 UI with WCAG AA accessibility and inline schema validation

### Developer Experience

- capture alt imp
- capture alt imp
- attribute
- checkpoint
- release 1.0.1
- apply merge-driven renames/branding (rusty-gun → pluresdb) and update packaging scripts
- remove tracked build artifacts (target/) and update .gitignore
- checkpoint
- reorg
- checkpoint
- plan
- checkpoint

### Other

- Automate version bumping, tagging, and multi-platform releases (#13)
- Upgrade to Deno 2.x and latest package versions (#11)
- Complete the rust implementation (#9)
- Remove legacy rusty-gun directory and rebrand to PluresDB (#6)
- Fix CI failures: deno formatting, lint configuration, and test workflow (#8)
- Merge pull request #4 from plures/copilot/develop-personal-database
- Revert legacy/cli.ts to use Node.js APIs instead of Deno APIs
- Fix Deno lint errors: replace process with Deno, fix unused error vars, remove unnecessary async
- Fix CI failures: add npm lint/fmt scripts and upgrade Deno to v2.x
- Add issue response document summarizing Windows personal database readiness
- Add comprehensive Windows personal database status document
- Add Windows-specific documentation and launcher files
- Fix TypeScript compilation errors and add Windows documentation
- Initial plan
- Merge branch 'feature/better-sqllite3-support' of https://github.com/plures/pluresdb
- Merge pull request #1 from plures/copilot/restructure-pluresdb-project
- Add restructuring summary document
- Update config files to reference legacy directory instead of src
- Restructure project: move src to legacy, create pluresdb-node and pluresdb-deno crates
- Initial plan
- Merge branch 'main' of https://github.com/kayodebristol/rusty-gun
- Resolve merge conflicts: accept current branch (ours) for all files
- Merge pull request #5 from kayodebristol/revert-3-copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Revert "[WIP] Rename nested 'rusty-gun' folder to 'pluresdb' and update references"
- Merge pull request #4 from kayodebristol/copilot/fix-db1e13fe-eea5-47d2-8ddf-18e1b5f69493
- Create five GitHub Actions workflows as specified
- Initial plan
- Merge pull request #3 from kayodebristol/copilot/fix-cf2f0af2-221d-47ce-8064-4d58ed05c1d6
- Initial plan
- Merge pull request #2 from kayodebristol/copilot/fix-edf5eacd-b439-43e7-af90-57eac7d6efb7
- Complete PluresDB rebrand: update packaging files, env vars, and Svelte components
- Update all branding references from rusty-gun to PluresDB throughout the codebase
- Remove .githooks, azure directories and update core branding to PluresDB
- Initial plan
- Phase 2 Complete: Data Modeling & Insight
- checkpoint
- new roadmap
- new reactive ui
- Checkpoint Continue Deno version
- Initial
- first commit


## [1.0.1] - 2025-10-03 — Core Security Hardening

### Changed

- Added payload sanitization before persistence to strip prototype pollution vectors and coerce injected functions into safe string placeholders.
- Hardened `PluresDB#get` responses with sanitized clones, ensuring consumer code receives benign `toString` implementations and no inherited attacker-controlled state.
- Expanded the security regression suite so the type-confusion prevention scenario now exercises the sanitization path and passes under `npm run verify` (51 tests green).

## [Unreleased] - Phase 1 UI Completion ✅

**Phase 1 is now COMPLETE!** All planned UI foundation and UX polish items have been implemented.

## [Unreleased] - Phase 1 Part 2: Accessibility & Validation

### Added - UI Foundation & UX Polish ✅

- **Accessibility Enhancements**
  - Keyboard navigation with arrow keys, Enter/Space for selection across all panels
  - Comprehensive ARIA labels, roles, and landmark regions throughout the UI
  - Screen reader support with sr-only class and aria-live regions for dynamic content
  - Semantic HTML structure with proper heading hierarchy
- **Node List Improvements**
  - Sort controls for ID and Type with visual indicators (↑/↓)
  - Enhanced keyboard navigation (ArrowUp/ArrowDown to navigate, Enter/Space to select)
  - Proper listbox/option ARIA roles for better assistive technology support
- **Editor Enhancements**
  - Copy-as-cURL button to generate curl commands for API calls
  - Revert changes button with change tracking (disabled when no changes)
  - Visual indication of unsaved changes
  - Tooltips on all editor action buttons
- **Search Panel Improvements**
  - Keyboard navigation for search results (Enter/Space to select)
  - Live result count announcement for screen readers
  - Proper ARIA labels for search input and results

- **Settings Panel Improvements**
  - Live save status announcement for screen readers
  - Descriptive help text for all configuration fields
  - Enhanced ARIA descriptions for all inputs

- **Main Navigation**
  - Proper menubar role with aria-current for active view indication
  - Enhanced dark mode toggle with descriptive aria-label

### Changed

- Reorganized editor toolbar into two rows (formatting actions + node actions)
- Delete button now has outline style to differentiate destructive action
- Improved reactive text handling to preserve unsaved changes when navigating

### Technical

- Built with Svelte 4, CodeMirror 6, Vite 5
- All components now follow WCAG 2.1 accessibility guidelines
- Proper separation of concerns with dedicated stores and components

### Added - Accessibility & Validation ✅

- **WCAG AA Color Contrast**
  - GitHub-inspired color palette with verified 4.5:1 minimum contrast ratios
  - Enhanced primary colors: #0969da (light) / #58a6ff (dark)
  - Improved muted text colors for better readability
  - Enhanced focus indicators (2px outline with offset)
  - Semantic colors for success/error/warning states
  - Accessible disabled states with proper opacity

- **Inline JSON Schema Validation**
  - Real-time validation in CodeMirror as you type
  - Inline error markers with CodeMirror's linter system
  - JSON syntax validation with position-aware error messages
  - Schema validation warnings when schema is provided
  - Automatic revalidation when schema or content changes
  - Clear error messages showing path and validation issue

### Technical Improvements

- Added @codemirror/lint package for inline diagnostics
- Integrated Ajv JSON Schema validator with CodeMirror linter
- Custom CSS variables for WCAG AA compliant color system
- Reactive schema updates trigger editor reconfiguration

### Phase 1 Status: ✅ COMPLETE

All Phase 1 deliverables have been implemented and tested. The UI now has:

- Comprehensive accessibility (keyboard nav, ARIA, WCAG AA colors)
- Real-time JSON Schema validation
- Professional data explorer with all planned features

## Previous Releases

See git history for earlier changes.
