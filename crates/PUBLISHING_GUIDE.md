# Publishing Guide for PluresDB Crates

This guide covers the process for publishing all PluresDB Rust crates to crates.io.

## Prerequisites

1. **crates.io Account**
   - Create account at https://crates.io
   - Verify email address
   - Generate API token: https://crates.io/me

2. **Cargo Login**
   ```bash
   cargo login <your-api-token>
   ```

3. **Verify Ownership**
   - Ensure you have publish rights for the `plures` organization
   - Verify all crates have correct metadata

## Publishing Order

Crates must be published in dependency order:

1. ✅ **pluresdb-core** (already published)
2. ✅ **pluresdb-sync** (already published)
3. **pluresdb-storage** (depends on core)
4. **pluresdb** (depends on core, storage, sync) - unified main crate
5. **pluresdb-cli** (depends on core, storage, sync)
6. **pluresdb-node** (depends on core, sync) - publishes to npm
7. **pluresdb-deno** (depends on core, sync) - publishes to JSR

## Publishing to crates.io

### 1. pluresdb-storage

```bash
cd crates/pluresdb-storage

# Verify it builds
cargo build --release

# Run tests
cargo test

# Check package
cargo package

# Verify package contents
cargo package --list

# Publish
cargo publish
```

### 2. pluresdb (unified main crate)

```bash
cd crates/pluresdb

# Verify it builds
cargo build --release

# Run tests
cargo test

# Check package
cargo package

# Verify package contents
cargo package --list

# Publish
cargo publish
```

### 3. pluresdb-cli

```bash
cd crates/pluresdb-cli

# Verify it builds
cargo build --release

# Run tests
cargo test

# Check package
cargo package

# Verify package contents
cargo package --list

# Publish
cargo publish
```

## Publishing to npm (pluresdb-node)

### Prerequisites

1. **npm Account**
   - Create account at https://www.npmjs.com
   - Login: `npm login`
   - Verify organization access: `@plures/pluresdb-native`

2. **Build for All Platforms**

```bash
cd crates/pluresdb-node

# Install dependencies
npm install

# Build for all platforms
npm run build

# Test locally
npm test

# Publish
npm publish --access public
```

### Multi-platform Build

The `napi` tool will automatically build for all platforms specified in `package.json`:
- x86_64-apple-darwin
- aarch64-apple-darwin
- x86_64-unknown-linux-gnu
- aarch64-unknown-linux-gnu
- x86_64-pc-windows-msvc
- aarch64-pc-windows-msvc

## Publishing to JSR (pluresdb-deno)

### Prerequisites

1. **JSR Account**
   - Create account at https://jsr.io
   - Login: `deno login`

2. **Generate Bindings and Publish**

```bash
cd crates/pluresdb-deno

# Build the library
cargo build --release

# Generate TypeScript bindings
deno_bindgen --release

# Create mod.ts wrapper (if not exists)
# See: crates/pluresdb-deno/mod.ts.example

# Publish to JSR
deno publish
```

## Verification Checklist

Before publishing each crate, verify:

### For Rust Crates (crates.io)

- [ ] `Cargo.toml` has correct metadata
- [ ] Version matches workspace version
- [ ] All dependencies are published or use path dependencies
- [ ] `cargo build --release` succeeds
- [ ] `cargo test` passes
- [ ] `cargo package` succeeds
- [ ] README.md exists (if applicable)
- [ ] License is specified
- [ ] Repository URL is correct

### For Node.js (npm)

- [ ] `package.json` has correct metadata
- [ ] Version matches workspace version
- [ ] `npm run build` succeeds
- [ ] `npm test` passes
- [ ] All platform binaries are built
- [ ] TypeScript definitions are included
- [ ] README.md exists

### For Deno (JSR)

- [ ] `deno.json` has correct metadata
- [ ] Bindings are generated
- [ ] `mod.ts` wrapper exists
- [ ] `deno publish --dry-run` succeeds
- [ ] README.md exists

## Version Management

All crates use workspace version from root `Cargo.toml`:
- Current version: **1.4.2**

When updating version:
1. Update `Cargo.toml` workspace version
2. Update `package.json` version (for Node.js)
3. Update `deno.json` version (for Deno)
4. Update `CHANGELOG.md`

## Troubleshooting

### Common Issues

1. **"crate already exists"**
   - Check if crate was already published
   - Use `cargo yank` to remove if needed

2. **"dependency not found"**
   - Ensure dependencies are published first
   - Check dependency versions match

3. **"package size too large"**
   - Check for unnecessary files
   - Use `.cargoignore` to exclude files

4. **"authentication failed"**
   - Re-run `cargo login` or `npm login`
   - Check API token is valid

## Post-Publishing

After publishing:

1. **Update Documentation**
   - Update main README.md with installation instructions
   - Update CHANGELOG.md

2. **Verify Installation**
   ```bash
   # Rust crates
   cargo install pluresdb-cli
   
   # Node.js
   npm install @plures/pluresdb-native
   
   # Deno
   deno add jsr:@plures/pluresdb
   ```

3. **Announce**
   - Update project status
   - Notify users of new packages

## Publishing Script

Create a script to automate publishing:

```bash
#!/bin/bash
# scripts/publish-crates.sh

set -e

echo "Publishing PluresDB crates..."

# Publish in dependency order
cd crates/pluresdb-storage && cargo publish && cd ../..
cd crates/pluresdb-cli && cargo publish && cd ../..

echo "All crates published successfully!"
```

## Security Notes

- Never commit API tokens
- Use environment variables for tokens
- Verify package contents before publishing
- Review all dependencies for security issues

