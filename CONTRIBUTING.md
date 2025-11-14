# Contributing to PluresDB

Thank you for your interest in contributing to PluresDB! This guide will help you get started.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/pluresdb.git
   cd pluresdb
   ```
3. **Install dependencies**:
   ```bash
   npm install
   ```
4. **Set up development environment**:
   - Install [Deno](https://deno.land/) v2.x
   - Install [Node.js](https://nodejs.org/) v20+
   - For Rust components, ensure you have Rust toolchain installed

## Development Workflow

### Making Changes

1. **Create a branch** for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** following our coding standards:
   - Write clean, readable code
   - Add tests for new features
   - Update documentation as needed

3. **Follow commit conventions**:
   We use [Conventional Commits](https://www.conventionalcommits.org/) for automated releases:
   
   ```bash
   # Bug fixes
   git commit -m "fix: resolve database connection timeout"
   
   # New features
   git commit -m "feat: add vector similarity search"
   
   # Breaking changes
   git commit -m "feat!: redesign API for better performance
   
   BREAKING CHANGE: The query API has changed. See docs for migration guide."
   ```

4. **Test your changes**:
   ```bash
   npm test
   npm run lint
   npm run fmt:check
   ```

5. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

6. **Open a Pull Request** on GitHub

## Commit Message Guidelines

We use conventional commits to automate versioning and changelog generation. Your commit messages should follow this format:

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Types

- `feat:` - New feature (minor version bump)
- `fix:` - Bug fix (patch version bump)
- `docs:` - Documentation changes
- `style:` - Code style changes (formatting, etc.)
- `refactor:` - Code refactoring
- `perf:` - Performance improvements
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks
- `ci:` - CI/CD changes
- `build:` - Build system changes

### Breaking Changes

For breaking changes, add `!` after the type or include `BREAKING CHANGE:` in the footer:

```bash
git commit -m "feat!: change database schema format"
```

or

```bash
git commit -m "feat: redesign storage engine

BREAKING CHANGE: The storage format has changed. Databases created with
version 1.x need to be migrated using the migration tool."
```

## Code Style

- **TypeScript/JavaScript**: Follow the ESLint configuration
- **Rust**: Follow standard Rust formatting (rustfmt)
- **Formatting**: Run `npm run fmt` before committing

## Testing

- Write tests for new features
- Ensure all tests pass: `npm test`
- Tests should be clear and maintainable

## Pull Request Process

1. **Update documentation** if you're changing functionality
2. **Add tests** for new features
3. **Ensure CI passes** - All checks must pass
4. **Request review** from maintainers
5. **Address feedback** promptly
6. **Squash commits** if requested to keep history clean

## Release Process

Releases are automated! When your PR is merged to `main`:

1. CI automatically analyzes your commit messages
2. Determines version bump (major, minor, or patch)
3. Updates CHANGELOG.md
4. Creates and pushes a git tag
5. Publishes to npm, Docker Hub, and Deno
6. Creates a GitHub release with binary packages

For details, see [RELEASE_PROCESS.md](RELEASE_PROCESS.md).

## Project Structure

```
pluresdb/
├── .github/
│   └── workflows/        # GitHub Actions workflows
├── crates/               # Rust workspace
│   ├── pluresdb-core/    # Core database engine
│   ├── pluresdb-storage/ # Storage layer
│   ├── pluresdb-sync/    # P2P synchronization
│   └── ...
├── web/                  # Web UI (Svelte)
├── src/                  # TypeScript/Deno source
├── scripts/              # Build and release scripts
├── docs/                 # Documentation
└── tests/                # Test files
```

## Development Commands

```bash
# Build the project
npm run build

# Build library only
npm run build:lib

# Build web UI
npm run build:web

# Run tests
npm test

# Lint code
npm run lint

# Format code
npm run fmt

# Check formatting
npm run fmt:check

# Run dev server
npm run dev

# Pre-release checks
npm run release-check
```

## Code of Conduct

- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- Help maintain a positive community

## Questions?

- Check existing [issues](https://github.com/plures/pluresdb/issues)
- Open a [new issue](https://github.com/plures/pluresdb/issues/new) for questions
- Join discussions in pull requests

## License

By contributing to PluresDB, you agree that your contributions will be licensed under the [GNU Affero General Public License v3.0 (AGPL v3)](LICENSE).

Thank you for contributing to PluresDB! 🎉
