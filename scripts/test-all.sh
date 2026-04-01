#!/usr/bin/env bash
set -euo pipefail

echo "=== Native tests ==="
cargo test --workspace

echo ""
echo "=== WASM build check ==="
cargo check -p pluresdb-core --target wasm32-unknown-unknown --no-default-features
cargo check -p pluresdb-wasm --target wasm32-unknown-unknown

echo ""
echo "=== Clippy ==="
cargo clippy --workspace -- -D warnings

echo ""
echo "=== Format check ==="
cargo fmt --all -- --check

echo ""
echo "✅ All checks passed"
