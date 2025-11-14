# Deprecated Workflows

These workflows have been deprecated and replaced by the new automated release system.

## Replacement Workflows

| Old Workflow | Replaced By | Notes |
|--------------|-------------|-------|
| `test.yml` | `.github/workflows/ci.yml` | Testing is now part of the main CI workflow |
| `publish-npm.yml` | `.github/workflows/release.yml` | npm publishing is now automated in the release workflow |
| `build-packages.yml` | `.github/workflows/release.yml` | Binary building is now part of the release workflow |
| `publish-packages.yml` | `.github/workflows/release.yml` | Package publishing is now automated in the release workflow |
| `publish-deno.yml` | `.github/workflows/release.yml` | Deno publishing is now part of the release workflow |
| `publish-docker.yml` | `.github/workflows/release.yml` | Docker publishing is now part of the release workflow |

## New Automated Release System

The new release system provides:

- **Automatic version bumping** based on commit messages (Conventional Commits)
- **Automatic changelog generation** from commit history
- **Coordinated releases** across npm, Docker Hub, and Deno
- **Binary packages** for Windows, macOS, and Linux
- **GitHub releases** with release notes and downloads

See [RELEASE_PROCESS.md](../../RELEASE_PROCESS.md) for details.

## Why the Change?

The old workflows had several issues:

1. **Manual version bumping** - Maintainers had to manually update versions
2. **Separate triggers** - Different workflows triggered at different times
3. **No changelog automation** - CHANGELOG.md had to be manually updated
4. **Inconsistent versioning** - Easy to forget to update all version files
5. **No coordination** - Workflows could run independently causing version mismatches

The new system solves all these problems by:

1. Using conventional commits to automatically determine version bumps
2. Updating all version files in sync (package.json, Cargo.toml)
3. Generating changelog entries automatically
4. Coordinating all publishing steps in a single release workflow
5. Creating git tags and GitHub releases automatically

## Migration

No action is needed. The new workflows are already active and will handle:

- CI/CD on pushes to main (`.github/workflows/ci.yml`)
- Releases when tags are created (`.github/workflows/release.yml`)

## Removing Old Workflows

These deprecated workflows will be removed in a future cleanup. They are kept here temporarily for reference.
