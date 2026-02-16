# Changelog

## [Unreleased]

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
- Hardened `GunDB#get` responses with sanitized clones, ensuring consumer code receives benign `toString` implementations and no inherited attacker-controlled state.
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
