# NPM Publishing Validation

## Overview

This document explains the npm publishing validation system for PluresDB and how to use it.

## The Problem

PluresDB supports both Node.js/npm and Deno ecosystems, which have different import conventions:

- **TypeScript/Node.js**: Imports should NOT include `.ts` extensions (e.g., `import { foo } from "./bar"`)
- **Deno**: Imports MUST include `.ts` extensions (e.g., `import { foo } from "./bar.ts"`)

This creates a conflict when:
1. Files are compiled for npm using TypeScript
2. The same files are tested using Deno during the `npm test` command
3. Deno type-checking fails because imports lack `.ts` extensions

## The Solution

We use Deno's `--sloppy-imports` flag, which allows Deno to automatically resolve imports without explicit extensions, similar to Node.js behavior.

### Changes Made

1. **Updated test command** in `package.json`:
   ```json
   "test": "deno test -A --unstable-kv --sloppy-imports"
   ```

2. **Created validation script** at `scripts/validate-npm-publish.js`:
   - Validates the package before publishing to npm
   - Runs comprehensive checks on TypeScript compilation, file structure, and Deno compatibility

3. **Updated release workflow** (`.github/workflows/release.yml`):
   - Added validation step before npm publish
   - Ensures packages are verified before release

## Using the Validation Script

### Manual Validation

Run the validation script manually before publishing:

```bash
npm run validate-publish
```

### What It Checks

The validation script performs the following checks:

1. **package.json validation**: Ensures required fields are present
2. **TypeScript compilation**: Builds the library with `tsc`
3. **Required files**: Checks that all expected dist files exist
4. **Deno type checking**: Validates key files with Deno using `--sloppy-imports`
5. **Tests**: Runs the Deno test suite (if available)
6. **Package size**: Reports the size of the npm package

### Example Output

```
🚀 NPM Publish Validation

📦 Validating package.json...
✓ Package: @plures/pluresdb@1.6.9

🔨 Building TypeScript...
✓ TypeScript compilation

📁 Checking required files...
✓ Required file: dist/node-index.js
✓ Required file: dist/node-index.d.ts
...

🦕 Deno type checking...
✓ Deno type check: legacy/local-first/unified-api.ts
...

🧪 Running tests...
✓ Deno tests

📊 Package size check...
✓ Package size: 137.3 kB

📋 Validation Summary
✓ All critical checks passed! ✨

The package is ready to be published to npm.
```

## CI/CD Integration

The validation runs automatically in the release workflow:

1. When a tag is pushed (e.g., `v1.0.0`)
2. Before `npm publish` is executed
3. If validation fails, the publish is aborted

### Workflow Steps

```yaml
- name: Validate package before publish
  run: npm run validate-publish
  env:
    DENO_PATH: deno
```

## Import Conventions

To maintain compatibility with both ecosystems:

### Files Compiled for npm (in `tsconfig.json`)

Use imports **without** `.ts` extensions:

```typescript
// ✅ Correct for npm-compiled files
import { debugLog } from "../util/debug";
import { PluresNode } from "./node-wrapper";
```

### Deno-only Files (not in `tsconfig.json`)

Use imports **with** `.ts` extensions:

```typescript
// ✅ Correct for Deno-only files
import { debugLog } from "../util/debug.ts";
import { PluresDBLocalFirst } from "../../local-first/unified-api.ts";
```

## Troubleshooting

### "Cannot find module" errors in Deno

If you see errors like:
```
TS2307 [ERROR]: Cannot find module 'file:///.../debug'
```

**Solution**: Ensure `--sloppy-imports` flag is used:
```bash
deno check --sloppy-imports your-file.ts
deno test -A --unstable-kv --sloppy-imports
```

### "Cannot end with .ts extension" errors in TypeScript

If you see errors like:
```
TS5097: An import path can only end with a '.ts' extension when 'allowImportingTsExtensions' is enabled.
```

**Solution**: Remove `.ts` extensions from imports in files that are compiled for npm (listed in `tsconfig.json`).

### Package Size Warnings

If the validation warns about package size:

1. Check the `files` array in `package.json`
2. Ensure build artifacts and dependencies are excluded
3. Use `.npmignore` to exclude unnecessary files

## Testing Locally

Before pushing changes that affect imports or the build process:

1. **Clean build**:
   ```bash
   rm -rf dist node_modules
   npm ci
   ```

2. **Run validation**:
   ```bash
   npm run validate-publish
   ```

3. **Test the build**:
   ```bash
   npm run build
   npm test
   ```

4. **Dry run publish**:
   ```bash
   npm pack --dry-run
   ```

## References

- [Deno Sloppy Imports Documentation](https://docs.deno.com/runtime/manual/basics/modules/#sloppy-imports)
- [TypeScript Module Resolution](https://www.typescriptlang.org/docs/handbook/module-resolution.html)
- [npm prepublishOnly Hook](https://docs.npmjs.com/cli/v9/using-npm/scripts#life-cycle-scripts)
