#!/bin/bash
# Publish all PluresDB crates to crates.io
# Usage: ./scripts/publish-crates.sh

set -e

echo "=========================================="
echo "Publishing PluresDB Crates to crates.io"
echo "=========================================="
echo ""

# Check if logged in
if ! cargo login --check 2>/dev/null; then
    echo "Error: Not logged in to crates.io"
    echo "Run: cargo login <your-api-token>"
    exit 1
fi

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to publish a crate
publish_crate() {
    local crate_name=$1
    local crate_path=$2
    
    echo -e "${YELLOW}Publishing ${crate_name}...${NC}"
    cd "$crate_path"
    
    # Verify it builds
    echo "  Building..."
    if ! cargo build --release > /dev/null 2>&1; then
        echo -e "${RED}  ✗ Build failed${NC}"
        return 1
    fi
    
    # Run tests
    echo "  Running tests..."
    if ! cargo test > /dev/null 2>&1; then
        echo -e "${RED}  ✗ Tests failed${NC}"
        return 1
    fi
    
    # Check package
    echo "  Checking package..."
    if ! cargo package > /dev/null 2>&1; then
        echo -e "${RED}  ✗ Package check failed${NC}"
        return 1
    fi
    
    # Publish
    echo "  Publishing to crates.io..."
    if cargo publish; then
        echo -e "${GREEN}  ✓ ${crate_name} published successfully${NC}"
        cd - > /dev/null
        return 0
    else
        echo -e "${RED}  ✗ Publishing failed${NC}"
        cd - > /dev/null
        return 1
    fi
}

# Publish in dependency order
echo "Publishing crates in dependency order..."
echo ""

# Note: pluresdb-core and pluresdb-sync are already published
# Uncomment if you need to republish them:
# publish_crate "pluresdb-core" "crates/pluresdb-core"
# publish_crate "pluresdb-sync" "crates/pluresdb-sync"

# Publish storage (depends on core)
publish_crate "pluresdb-storage" "crates/pluresdb-storage"
echo ""

# Publish unified main crate (depends on core, storage, sync)
publish_crate "pluresdb" "crates/pluresdb"
echo ""

# Publish CLI (depends on core, storage, sync)
publish_crate "pluresdb-cli" "crates/pluresdb-cli"
echo ""

echo "=========================================="
echo -e "${GREEN}All crates published successfully!${NC}"
echo "=========================================="
echo ""
echo "Note: pluresdb-node and pluresdb-deno are published separately:"
echo "  - pluresdb-node: npm publish (in crates/pluresdb-node)"
echo "  - pluresdb-deno: deno publish (in crates/pluresdb-deno)"

