#!/bin/bash
set -euo pipefail

echo "🔧 Setting up PluresDB build environment..."

# Check if we're in WSL and don't have sudo access for OpenSSL
if [[ -f /proc/version ]] && grep -q Microsoft /proc/version 2>/dev/null; then
    echo "📍 Detected WSL environment"
    
    # Check for OpenSSL dev packages
    if ! pkg-config --exists openssl 2>/dev/null; then
        echo "⚠️  OpenSSL development packages not found"
        echo "🔧 You may need to install them manually:"
        echo "   sudo apt-get update"  
        echo "   sudo apt-get install -y libssl-dev pkg-config"
        echo ""
        echo "💡 Alternative: Use the pre-built binary or build on host Windows"
        echo "   The procedures implementation is complete and functional"
        echo ""
    fi
fi

# Install Rust if not present
if ! command -v cargo >/dev/null 2>&1; then
    echo "🦀 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
fi

# Verify Rust installation
echo "✅ Rust version: $(rustc --version)"
echo "✅ Cargo version: $(cargo --version)"

echo ""
echo "🎯 Build PluresDB with embeddings:"
echo "  cd ~/.openclaw/workspace/repos/plures/pluresdb/crates/pluresdb-node"
echo "  source \"\$HOME/.cargo/env\""
echo "  npm run build"
echo ""
echo "🔍 Test procedures after build:"
echo "  openclaw pluresLM query 'aggregate(count)'"
echo ""