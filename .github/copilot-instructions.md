# Copilot Instructions for PluresDB

## Project Overview

PluresDB is a **P2P Graph Database with SQLite Compatibility** - a local-first, offline-first database for modern applications. The project is built as a Rust-first monorepo with language bindings for Deno and Node.js.

### Tech Stack
- **Rust**: Core database engine, storage layer, and P2P synchronization (workspace in `crates/`)
- **TypeScript/Deno**: Primary runtime with Deno as the main development platform
- **Node.js**: Compatibility layer with better-sqlite3 API compatibility
- **Svelte**: Web UI for database management (in `web/svelte/`)
- **Express**: HTTP API server for REST endpoints

### Key Features
- 95% SQLite API compatibility for easy migration
- P2P graph database with CRDT conflict resolution
- Built-in vector search and similarity search
- End-to-end encryption for P2P data sharing
- Local-first with offline support

## Repository Structure

```
pluresdb/
├── .github/              # GitHub configuration and workflows
├── crates/               # Rust workspace
│   ├── pluresdb-core/    # Core database engine
│   ├── pluresdb-storage/ # Storage layer
│   ├── pluresdb-sync/    # P2P synchronization
│   ├── pluresdb-cli/     # CLI application
│   ├── pluresdb-node/    # Node.js bindings
│   └── pluresdb-deno/    # Deno bindings
├── legacy/               # TypeScript/Deno source (transitioning to Rust)
├── web/                  # Svelte web UI
├── tests/                # Test files
├── scripts/              # Build and release automation
├── packaging/            # Distribution packages (MSI, winget, etc.)
├── docs/                 # Documentation
└── examples/             # Usage examples
```

## Coding Standards

### TypeScript/JavaScript
- **Target**: ES2022
- **Module System**: CommonJS for Node.js compatibility, ESM for Deno
- **Style**: Follow ESLint configuration in `eslint.config.js`
- **Formatting**: 
  - For TypeScript/Node.js: Use Prettier with settings from `.prettierrc.cjs` (print width: 100)
  - For Deno: Use `deno fmt` with settings from `deno.json` (line width: 80)
  - 2 spaces for indentation
  - Double quotes (not single quotes)
  - Trailing commas: always
  - Semicolons: required
- **Type Safety**: Use TypeScript strict mode - avoid `any` when possible
- **Naming**: Use descriptive variable names; prefix unused variables with `_`
- **Imports**: Group imports logically (builtin/external, internal, parent/sibling)

### Rust
- **Edition**: 2021
- **Formatting**: Use `rustfmt` for all Rust code
- **Error Handling**: Use `anyhow::Result` for errors, `thiserror` for custom error types
- **Async**: Use `tokio` runtime with full features
- **Documentation**: Document public APIs with doc comments (`///`)

### General Principles
- **Security First**: Never commit secrets; sanitize all user inputs
- **Payload Sanitization**: All incoming data must be scrubbed to prevent prototype pollution and function injection
- **Local-First**: Prioritize offline functionality; sync is secondary
- **Minimal Dependencies**: Only add new dependencies when absolutely necessary
- **Backward Compatibility**: Maintain 95% SQLite API compatibility

## Architecture Guidelines

### Database Layer
- Core engine in Rust (`crates/pluresdb-core/`)
- Storage abstraction supports multiple backends (RocksDB, Sled, SQLite via rusqlite)
- All database operations go through the core API

### API Layers
- **SQLite-Compatible API**: Primary interface for compatibility (`legacy/sqlite-compat.ts`)
- **better-sqlite3 API**: Synchronous-style API for Node.js (`legacy/better-sqlite3.ts`)
- **REST API**: Express-based HTTP server for web apps
- **WebSocket API**: Real-time updates and synchronization

### P2P Architecture
- **Identity**: Public key infrastructure for peer identification (ed25519-dalek)
- **Encryption**: End-to-end encryption with AES-GCM
- **CRDT**: Conflict-free replicated data types for distributed sync
- **Transport**: libp2p for P2P networking

### Module Boundaries
- Keep Rust core independent of JavaScript/TypeScript layers
- TypeScript layer should wrap Rust APIs, not reimplement logic
- Web UI communicates via REST/WebSocket only, never direct database access

## Build and Test Workflows

### Development Setup
```bash
# Install dependencies
npm install

# For Rust development on Windows
pwsh ./scripts/setup-libclang.ps1 -ConfigureCurrentProcess
```

### Building
```bash
# Build everything (library + web UI)
npm run build

# Build library only (TypeScript compilation)
npm run build:lib

# Build web UI only
npm run build:web

# Rust workspace
cargo build --workspace
```

### Testing
```bash
# Run all Deno tests
npm test
# or: deno test -A --unstable-kv

# Run specific test suite
deno test -A --unstable-kv legacy/tests/unit/
deno test -A --unstable-kv legacy/tests/integration/
deno test -A --unstable-kv legacy/tests/security/

# Azure relay tests
npm run test:azure:relay

# Node.js test
node tests/better-sqlite3.test.js

# Rust tests
cargo test --workspace
```

### Verification (Pre-commit)
```bash
# Full verification pipeline
npm run verify  # Builds TypeScript + runs all Deno tests

# Linting
npm run lint

# Format checking
npm run fmt:check

# Format code
npm run fmt
```

### Running Locally
```bash
# Development mode (auto-reload)
npm run dev

# Production mode
npm start
# or: node dist/cli.js serve
```

## Contribution Workflow

### Commit Messages
We use [Conventional Commits](https://www.conventionalcommits.org/) for automated versioning:

- `feat:` - New feature (minor version bump)
- `fix:` - Bug fix (patch version bump)
- `docs:` - Documentation changes
- `style:` - Code style/formatting changes
- `refactor:` - Code refactoring without behavior change
- `perf:` - Performance improvements
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks
- `ci:` - CI/CD changes
- `build:` - Build system changes

For breaking changes, add `!` after type or include `BREAKING CHANGE:` in footer:
```
feat!: redesign storage API

BREAKING CHANGE: Storage API now requires async/await
```

### Pull Request Process
1. Fork and create a feature branch
2. Make minimal, focused changes
3. Add tests for new features
4. Update documentation as needed
5. Ensure all tests pass (`npm run verify`)
6. Lint and format code (`npm run lint && npm run fmt`)
7. Open PR with clear description
8. Address review feedback promptly

### Release Process
Releases are fully automated via CI/CD:
- Commit messages determine version bump
- CHANGELOG.md is auto-generated
- Packages published to npm, JSR (Deno), Docker Hub
- Windows packages (MSI, winget) are built and released
- See [RELEASE_PROCESS.md](../RELEASE_PROCESS.md) for details

## Security Guidelines

### Critical Security Rules
1. **Never commit secrets** - Use environment variables or secure vaults
2. **Sanitize all inputs** - Prevent prototype pollution and injection attacks
3. **Validate peer data** - Don't trust incoming P2P data without validation
4. **Use encryption** - All P2P communications must be encrypted
5. **Report security issues privately** - Email security@plures.dev (never open public issues)

### Security Best Practices
- Keep dependencies updated (Dependabot is enabled)
- Use TLS/SSL for network communications in production
- Enable audit logs in production environments
- Implement proper access controls
- Follow the principle of least privilege

### Vulnerability Reporting
- **DO NOT** open public GitHub issues for security vulnerabilities
- Use private vulnerability reporting or email security@plures.dev
- Include: description, reproduction steps, impact, and optional fixes
- Expect acknowledgment within 48 hours

## Important Notes

### Windows Development
- Rust development on Windows requires `libclang` for bindgen dependencies (zstd-sys)
- Run `pwsh ./scripts/setup-libclang.ps1 -ConfigureCurrentProcess` to auto-configure
- Script installs LLVM via winget/choco and sets `LIBCLANG_PATH`

### License
- **AGPL v3**: All contributions must be compatible with AGPL v3
- All modifications to PluresDB must remain open source
- Commercial users should review AGPL requirements

### Documentation References
- [README.md](../README.md) - Project overview and quick start
- [CONTRIBUTING.md](../CONTRIBUTING.md) - Detailed contribution guide
- [SECURITY.md](../SECURITY.md) - Security policy and best practices
- [docs/WINDOWS_GETTING_STARTED.md](../docs/WINDOWS_GETTING_STARTED.md) - Windows setup guide
- [RELEASE_PROCESS.md](../RELEASE_PROCESS.md) - Release automation details
- [docs/TESTING.md](../docs/TESTING.md) - Testing guidelines and benchmarks

## Common Tasks

### Adding a New Feature
1. Check if it fits the local-first, SQLite-compatible philosophy
2. Write tests first (TDD approach recommended)
3. Implement in Rust core if it's database-level functionality
4. Add TypeScript bindings if needed for Node/Deno APIs
5. Update documentation and examples
6. Ensure security implications are addressed

### Fixing a Bug
1. Write a failing test that reproduces the bug
2. Fix the bug with minimal code changes
3. Verify the test passes
4. Check for similar bugs in related code
5. Update relevant documentation if behavior changes

### Updating Dependencies
1. Check for security advisories before updating
2. Update `package.json` and/or `Cargo.toml`
3. Run full test suite after updates
4. Test compatibility with examples
5. Document breaking changes if any

### Adding Tests
- Place unit tests in `legacy/tests/unit/`
- Place integration tests in `legacy/tests/integration/`
- Place security tests in `legacy/tests/security/`
- Place performance tests in `legacy/tests/performance/`
- Use descriptive test names that explain what is being tested
- Follow existing test patterns in the repository

## VSCode Extension Integration

PluresDB is designed to be a drop-in SQLite replacement for VSCode extensions:
- Use `SQLiteCompatibleAPI` from `pluresdb`
- 95% API compatibility with SQLite
- Supports same database operations
- See `examples/vscode-extension-integration.ts` for patterns
