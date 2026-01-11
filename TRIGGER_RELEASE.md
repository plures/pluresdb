# Trigger Release Workflow

## Current Status

✅ **Committed:** All changes committed (2 commits)
- `6d70b20` - chore: Add pluresdb unified crate to release workflow publishing order
- `121fcd1` - feat: Complete all missing crate implementations and add unified pluresdb crate

✅ **Tagged:** v1.4.2 tag created locally

⚠️ **Push:** Requires authentication

## Option 1: Push via GitHub CLI (Recommended)

If you have GitHub CLI installed and authenticated:

```bash
# Push commits
gh auth refresh
git push origin main

# Push tag
git push origin v1.4.2
```

The release workflow will automatically trigger when the tag is pushed.

## Option 2: Manual Workflow Dispatch

If you can't push directly, you can trigger the workflow manually:

1. Go to: https://github.com/plures/pluresdb/actions/workflows/release.yml
2. Click "Run workflow"
3. Enter tag: `v1.4.2`
4. Click "Run workflow"

This will trigger the release workflow with the specified tag.

## Option 3: Push via SSH/HTTPS with Token

```bash
# Using SSH (if configured)
git push origin main
git push origin v1.4.2

# Or using HTTPS with token
git push https://<token>@github.com/plures/pluresdb.git main
git push https://<token>@github.com/plures/pluresdb.git v1.4.2
```

## What the Release Workflow Will Do

Once triggered, the workflow will:

1. ✅ Verify the tag is on main branch
2. 📦 Publish to npm (if NPM_TOKEN configured)
3. 📦 Publish Rust crates to crates.io (if CARGO_REGISTRY_TOKEN configured):
   - pluresdb-core (already published, will skip)
   - pluresdb-sync (already published, will skip)
   - pluresdb-storage (NEW)
   - pluresdb (NEW - unified crate)
   - pluresdb-cli (NEW)
4. 🐳 Build and publish Docker image (if DOCKERHUB credentials configured)
5. 📦 Build binary packages for Windows, macOS, Linux
6. 🏷️ Create GitHub Release with binaries and release notes

## Required Secrets

Make sure these secrets are configured in GitHub:

- `CARGO_REGISTRY_TOKEN` - For publishing to crates.io
- `NPM_TOKEN` - For publishing to npm
- `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN` - For Docker Hub

## Verify After Release

After the workflow completes, verify:

1. **crates.io**: https://crates.io/crates/pluresdb
2. **npm**: https://www.npmjs.com/package/pluresdb
3. **GitHub Release**: https://github.com/plures/pluresdb/releases

## Current Commits Ready to Release

```
6d70b20 - chore: Add pluresdb unified crate to release workflow publishing order
121fcd1 - feat: Complete all missing crate implementations and add unified pluresdb crate
```

## Tag Information

- **Tag:** v1.4.2
- **Points to:** 6d70b204bb939b59472709719f7f3711ffa5ce6f
- **Message:** Release v1.4.2: Complete all crate implementations and add unified pluresdb crate

