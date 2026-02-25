# Release Process

This document describes the automated release process for pluresdb.

## Overview

The release process is fully automated using GitHub Actions workflows. The automation:

1. Detects version bump type from commit messages
2. Updates CHANGELOG.md with categorized commits
3. Bumps version in package.json, Cargo.toml, and deno.json
4. Creates and pushes git tags
5. Publishes to npm, crates.io, Docker Hub, JSR (Deno), and GitHub Packages
6. Creates GitHub releases with binary packages

## Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/) to determine version bumps:

- `feat:` or `feat(scope):` → **minor** version bump (new features)
- `fix:` or `fix(scope):` → **patch** version bump (bug fixes)
- `BREAKING CHANGE:` in commit body or `!` before `:` → **major** version bump (breaking changes)
- Other types (`chore:`, `docs:`, `style:`, `refactor:`, `test:`, `ci:`, `build:`) → included in changelog

### Examples

```bash
# Patch bump (1.0.1 → 1.0.2)
git commit -m "fix: resolve memory leak in sync engine"

# Minor bump (1.0.1 → 1.1.0)
git commit -m "feat: add support for custom vector search indices"

# Major bump (1.0.1 → 2.0.0)
git commit -m "feat!: redesign P2P protocol for better performance

BREAKING CHANGE: The P2P protocol has been completely redesigned. 
Nodes running version 1.x will not be able to communicate with version 2.x nodes."
```

## Automated Release Flow

### 1. CI Workflow (on push to main)

When code is pushed to the `main` branch:

1. **Build and Test** job runs:
   - Checks code formatting
   - Runs linter
   - Builds the project
   - Runs tests

2. **Release Check** job runs:
   - Validates package versions are consistent
   - Checks CHANGELOG.md structure
   - Analyzes commit messages

3. **Version Bump and Tag** job runs:
   - Analyzes commits since last tag
   - Determines version bump type
   - Updates package.json and Cargo.toml
   - Updates CHANGELOG.md with new version
   - Creates commit with message: `chore(release): X.Y.Z`
   - Creates git tag `vX.Y.Z`
   - Pushes commit and tag to GitHub

### 2. Release Workflow (on tag push)

When a tag matching `v*` is pushed:

1. **Verify Release** job:
   - Confirms the tag commit is on main branch
   - Extracts version number

2. **Publish to npm** job:
   - Builds the project
   - Publishes to npm registry (if NPM_TOKEN is configured)

3. **Publish to crates.io** job:
   - Publishes Rust crates in dependency order:
     - pluresdb-core (base CRDT and data structures)
     - pluresdb-storage (storage abstraction layer)
     - pluresdb-sync (P2P synchronization)
     - pluresdb-cli (command-line interface)
   - Skipped if CARGO_REGISTRY_TOKEN not configured

4. **Build Packages** job (parallel for Windows, macOS, Linux):
   - Compiles Deno binary
   - Packages with web UI, documentation, and installer scripts
   - Uploads artifacts

5. **Create GitHub Release** job:
   - Downloads all binary packages
   - Creates GitHub release with:
     - Release notes from CHANGELOG.md
     - Binary downloads for all platforms
     - Auto-generated release notes

6. **Publish to Docker** job:
   - Builds multi-arch Docker image (amd64, arm64)
   - Pushes to Docker Hub (if credentials configured)

7. **Publish to Deno** job:
   - Placeholder for JSR/Deno Land publishing (automatic via webhook)

## Manual Release

If you need to manually trigger a release:

### Option 1: Via GitHub UI

1. Go to the [Release workflow](../../actions/workflows/release.yml)
2. Click "Run workflow"
3. Enter the tag name (e.g., `v1.0.2`)
4. Click "Run workflow"

### Option 2: Via Command Line

```bash
# Create and push a tag manually
git tag v1.0.2
git push origin v1.0.2
```

## Pre-Release Checklist

Before pushing to main, ensure:

- [ ] All tests pass locally: `npm test`
- [ ] Code is properly formatted: `npm run fmt`
- [ ] No linting errors: `npm run lint`
- [ ] Commits follow conventional commit format
- [ ] Breaking changes are documented in commit messages

You can run pre-release checks manually:

```bash
npm run release-check
```

## Troubleshooting

### Tag Creation Failed

If the automated tag creation fails, an issue will be automatically created. To fix:

1. Check the workflow logs for errors
2. Manually create the tag if needed:
   ```bash
   git tag v1.0.2 <commit-sha>
   git push origin v1.0.2
   ```

### Release Not Publishing

Check that required secrets are configured in GitHub:

- `NPM_TOKEN` - for npm publishing
- `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN` - for Docker publishing

### Version Mismatch

If package.json, Cargo.toml, and deno.json versions are out of sync, the `release-check` job will fail with a detailed error message.

**Symptoms:**
- CI fails with "❌ Version mismatch between package.json and Cargo.toml" or similar
- The error message shows the version in each file

**Resolution:**

1. **Identify the correct version** - Determine which version is correct based on your intended release
2. **Update all files to match**:
   - Update `package.json`: Change the `version` field (line ~3)
   - Update `Cargo.toml`: Change the `version` field in `[workspace.package]` section (line ~16)
   - Update `deno.json`: Change the `version` field (line ~2)
3. **Verify the fix**: Run `npm run release-check` locally to confirm all versions match
4. **Commit the fix**: 
   ```bash
   git add package.json Cargo.toml deno.json
   git commit -m "fix: synchronize package versions"
   git push
   ```
5. The next CI run will succeed

**Prevention:**
- The automated release workflow updates all three files together
- Always use conventional commits to trigger automated version bumps
- Run `npm run release-check` before pushing to catch issues early

## Configuration

### Required Secrets

Configure these in GitHub repository settings → Secrets and variables → Actions:

| Secret | Required | Purpose |
|--------|----------|---------|
| `NPM_TOKEN` | Optional | Publishing to npm registry |
| `CARGO_REGISTRY_TOKEN` | Optional | Publishing to crates.io |
| `DOCKERHUB_USERNAME` | Optional | Docker Hub login |
| `DOCKERHUB_TOKEN` | Optional | Docker Hub authentication |

### Workflow Files

- `.github/workflows/ci.yml` - Main CI with version bumping
- `.github/workflows/release.yml` - Release and publishing
- `.github/workflows/test.yml` - Test suite (legacy, being replaced by ci.yml)

### Scripts

- `scripts/update-changelog.js` - Updates CHANGELOG.md with commits
- `scripts/release-check.js` - Pre-release validation checks

## Best Practices

1. **Use conventional commits** - This ensures proper version bumping
2. **Write descriptive commit messages** - They appear in the changelog
3. **Group related changes** - Use feature branches and squash commits
4. **Document breaking changes** - Use `BREAKING CHANGE:` in commit body
5. **Test before merging** - CI runs on PRs, but local testing is recommended

## Versioning Strategy

We follow [Semantic Versioning 2.0.0](https://semver.org/):

- **Major (X.0.0)** - Breaking changes
- **Minor (1.X.0)** - New features, backwards compatible
- **Patch (1.0.X)** - Bug fixes, backwards compatible

For versions < 1.0.0:
- Breaking changes bump minor version (0.X.0)
- Everything else bumps patch version (0.1.X)

## Support

For questions or issues with the release process:

1. Check the [workflow runs](../../actions)
2. Review the [issue tracker](../../issues)
3. Contact the maintainers
