/**
 * PluresDB Embedded - Pure embedded database (no server process required)
 *
 * Re-exports the native N-API bindings from @plures/pluresdb-native.
 * Requires the native `.node` binary to be built first:
 *   cd crates/pluresdb-node && npm run build
 *
 * Usage:
 *   const { PluresDatabase } = require('@plures/pluresdb/embedded');
 *   const db = new PluresDatabase('my-actor', '/tmp/my.db');
 */

const path = require('path');

let nativeModule;
try {
  // Try loading the platform-specific binary via napi-rs conventions
  const platformTriple = `${process.arch === 'x64' ? 'x86_64' : process.arch === 'arm64' ? 'aarch64' : process.arch}-${
    process.platform === 'linux' ? 'unknown-linux-gnu' :
    process.platform === 'darwin' ? 'apple-darwin' :
    process.platform === 'win32' ? 'pc-windows-msvc' : process.platform
  }`;

  try {
    // Try platform-specific package first (npm optional deps pattern)
    nativeModule = require(`@plures/pluresdb-native-${platformTriple}`);
  } catch {
    // Fall back to local build output in crates/pluresdb-node/
    nativeModule = require(path.join(__dirname, 'crates', 'pluresdb-node', `pluresdb-node.${process.platform}-${process.arch === 'x64' ? 'x86_64' : process.arch}-${process.platform === 'linux' ? 'gnu' : ''}.node`));
  }
} catch (err) {
  // Final fallback: try loading index.js from the native crate directory
  try {
    nativeModule = require(path.join(__dirname, 'crates', 'pluresdb-node', 'index.js'));
  } catch {
    throw new Error(
      `Failed to load PluresDB native bindings. ` +
      `The native .node binary must be built first:\n` +
      `  cd crates/pluresdb-node && npm run build\n` +
      `Or install the pre-built binary: npm install @plures/pluresdb-native\n` +
      `Original error: ${err.message}`
    );
  }
}

module.exports = nativeModule;
