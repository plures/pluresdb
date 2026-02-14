# Publishing Guide for @plures/pluresdb-native

This document provides step-by-step instructions for publishing the N-API bindings to npm.

## Prerequisites

1. **GitHub Actions Approval**
   - Repository admin must approve the `build-napi.yml` workflow
   - Navigate to: Actions → Build N-API Bindings → Review pending workflows

2. **npm Account & Token**
   - npm account with publish access to `@plures` scope
   - npm authentication token stored in GitHub Secrets as `NPM_TOKEN`

## Option 1: Automated Publishing (Recommended)

The automated approach uses GitHub Actions to build for all platforms and publish.

### Step 1: Trigger CI Builds

Push to main or create a tag to trigger the workflow:

```bash
# Option A: Merge PR to main
# This will trigger builds but NOT publish (no tag)

# Option B: Create and push tag (recommended for alpha)
git tag v2.0.0-alpha.1
git push origin v2.0.0-alpha.1
```

### Step 2: Monitor CI Builds

1. Go to: https://github.com/plures/pluresdb/actions
2. Click on the running "Build N-API Bindings" workflow
3. Verify all 6 platform builds succeed:
   - ✅ Build - x86_64-unknown-linux-gnu
   - ✅ Build - aarch64-unknown-linux-gnu
   - ✅ Build - x86_64-apple-darwin
   - ✅ Build - aarch64-apple-darwin
   - ✅ Build - x86_64-pc-windows-msvc
   - ✅ Build - aarch64-pc-windows-msvc

### Step 3: Verify npm Publish

If you pushed a tag and `NPM_TOKEN` is configured:
- The workflow will automatically publish to npm
- Check: https://www.npmjs.com/package/@plures/pluresdb-native

If `NPM_TOKEN` is NOT configured:
- The workflow performs a dry-run publish
- See "Option 2" below for manual publishing

## Option 2: Manual Publishing

If you need to publish manually or the automated workflow doesn't have npm credentials:

### Step 1: Download Artifacts

After CI builds complete:

```bash
# Install GitHub CLI
gh auth login

# Download all platform binaries
cd crates/pluresdb-node
gh run download --dir artifacts

# Or download from Actions UI and extract to crates/pluresdb-node/
```

### Step 2: Move Binaries

```bash
# Move all .node files to package root
mv artifacts/**/*.node .

# Verify all platforms present
ls -lh *.node
# Should see:
# pluresdb-node.linux-x64-gnu.node
# pluresdb-node.linux-arm64-gnu.node
# pluresdb-node.darwin-x64.node
# pluresdb-node.darwin-arm64.node
# pluresdb-node.win32-x64-msvc.node
# pluresdb-node.win32-arm64-msvc.node
```

### Step 3: Publish to npm

```bash
# Make sure you're in the package directory
cd crates/pluresdb-node

# Login to npm (if not already)
npm login

# Dry run to verify package contents
npm publish --dry-run --access public

# Publish for real
npm publish --access public --tag alpha
```

## Option 3: Local Development Build

For testing locally before publishing:

```bash
cd crates/pluresdb-node

# Build for your platform
npm install
npm run build

# Test
npm test

# Create tarball for manual distribution
npm pack
# Creates: plures-pluresdb-native-2.0.0-alpha.1.tgz

# Install in another project
cd /path/to/other/project
npm install /path/to/plures-pluresdb-native-2.0.0-alpha.1.tgz
```

## Verifying the Published Package

After publishing to npm:

### 1. Install in a Test Project

```bash
mkdir test-pluresdb-native
cd test-pluresdb-native
npm init -y
npm install @plures/pluresdb-native@alpha
```

### 2. Test Basic Functionality

Create `test.js`:

```javascript
const { PluresDatabase } = require('@plures/pluresdb-native');

console.log('Testing @plures/pluresdb-native...');

// Test 1: Basic CRUD
const db = new PluresDatabase('test-actor');
db.put('test-1', { message: 'Hello from N-API!' });
const result = db.get('test-1');
console.log('✅ CRUD works:', result);

// Test 2: SQL (with database file)
const sqlDb = new PluresDatabase('test-actor', './test.db');
sqlDb.exec('CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)');
sqlDb.exec("INSERT INTO users (name) VALUES ('Alice')");
const users = sqlDb.query('SELECT * FROM users WHERE name = ?', ['Alice']);
console.log('✅ SQL works:', users.rows);

console.log('All tests passed! ✅');
```

Run it:

```bash
node test.js
```

### 3. Test on Multiple Platforms

If possible, test on:
- Linux (x64, arm64)
- macOS (Intel, Apple Silicon)
- Windows (x64)

## Version Management

### Alpha Releases

Use for testing before stable release:

```bash
# Publish with alpha tag
npm publish --tag alpha

# Users install with:
npm install @plures/pluresdb-native@alpha
```

### Beta Releases

After alpha testing is successful:

```bash
# Update version to beta
npm version 2.0.0-beta.1 --no-git-tag-version

# Rebuild all platforms via CI
git tag v2.0.0-beta.1
git push origin v2.0.0-beta.1

# Publish with beta tag
npm publish --tag beta
```

### Stable Releases

After beta testing:

```bash
# Update version to stable
npm version 2.0.0 --no-git-tag-version

# Rebuild all platforms via CI
git tag v2.0.0
git push origin v2.0.0

# Publish with latest tag (default)
npm publish --tag latest
```

## Troubleshooting

### Build Fails on Specific Platform

1. Check CI logs for the failing platform
2. Common issues:
   - Missing cross-compilation tools (install via apt/brew)
   - Architecture not supported by dependencies
   - Rust toolchain not installed for target

### Binary Not Loading

1. Verify correct platform binary is present:
   ```bash
   ls -la node_modules/@plures/pluresdb-native/*.node
   ```

2. Check Node.js version (requires ≥20.0.0):
   ```bash
   node --version
   ```

3. Rebuild for your platform:
   ```bash
   cd node_modules/@plures/pluresdb-native
   npm run build
   ```

### npm Publish Permission Denied

1. Verify you're logged in:
   ```bash
   npm whoami
   ```

2. Verify you have publish access to `@plures` scope:
   ```bash
   npm access list packages @plures
   ```

3. Contact npm organization admin to grant publish permissions

## Post-Publishing Tasks

After successful publish:

1. **Update Documentation**
   - Add installation instructions to main README
   - Update version badges
   - Add release notes

2. **Create GitHub Release**
   ```bash
   gh release create v2.0.0-alpha.1 \
     --title "N-API Bindings Alpha Release" \
     --notes "First alpha release of native Node.js bindings"
   ```

3. **Announce Release**
   - Update issue #(issue number) with package link
   - Notify superlocalmemory team (plures/superlocalmemory#5)
   - Post in community channels

4. **Monitor for Issues**
   - Watch npm downloads
   - Monitor GitHub issues
   - Check for platform-specific problems

## Support

For questions or issues:
- GitHub Issues: https://github.com/plures/pluresdb/issues
- Documentation: See `crates/pluresdb-node/README.md`
- Build Summary: See `NAPI_BUILD_SUMMARY.md`
