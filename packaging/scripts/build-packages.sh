#!/bin/bash

# Build Packages Script for PluresDB
# This script builds packages for all supported platforms and package managers

set -e

VERSION="1.0.0"
OUTPUT_DIR="dist"
SKIP_TESTS=false
SKIP_WEBUI=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --skip-webui)
            SKIP_WEBUI=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --version VERSION     Set version (default: 1.0.0)"
            echo "  --output-dir DIR      Set output directory (default: dist)"
            echo "  --skip-tests          Skip running tests"
            echo "  --skip-webui          Skip building web UI"
            echo "  -h, --help            Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "🚀 Building PluresDB Packages v$VERSION"

# Create output directory
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

# Function to run tests
run_tests() {
    if [ "$SKIP_TESTS" = false ]; then
        echo "🧪 Running tests..."
        cd ../../
        deno test -A
        if [ $? -ne 0 ]; then
            echo "❌ Tests failed!"
            exit 1
        fi
        cd packaging/scripts
    fi
}

# Function to build web UI
build_webui() {
    if [ "$SKIP_WEBUI" = false ]; then
        echo "🎨 Building web UI..."
        cd ../../web/svelte
        npm install
        npm run build
        if [ $? -ne 0 ]; then
            echo "❌ Web UI build failed!"
            exit 1
        fi
        cd ../../../packaging/scripts
    fi
}

# Function to build Deno binary
build_deno_binary() {
    echo "🔨 Building Deno binary..."
    cd ../../
    deno compile -A --output "packaging/scripts/$OUTPUT_DIR/pluresdb" src/main.ts
    if [ $? -ne 0 ]; then
        echo "❌ Deno binary build failed!"
        exit 1
    fi
    cd packaging/scripts
}

# Function to create Linux package
create_linux_package() {
    echo "📦 Creating Linux package..."
    
    local arch=$(uname -m)
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local package_dir="$OUTPUT_DIR/$os-$arch"
    
    mkdir -p "$package_dir"
    
    # Copy binary
    cp "$OUTPUT_DIR/pluresdb" "$package_dir/"
    chmod +x "$package_dir/pluresdb"
    
    # Copy web UI
    cp -r "../../web/dist" "$package_dir/web"
    
    # Copy config files
    cp "../../deno.json" "$package_dir/"
    cp "../../src/config.ts" "$package_dir/"
    
    # Copy README and LICENSE
    cp "../../README.md" "$package_dir/"
    cp "../../LICENSE" "$package_dir/"
    
    # Create installer script
    cat > "$package_dir/install.sh" << 'EOF'
#!/bin/bash
echo "Installing PluresDB..."
echo ""
echo "PluresDB is a P2P Graph Database with SQLite Compatibility"
echo ""
echo "Features:"
echo "- Local-first data storage"
echo "- P2P synchronization"
echo "- SQLite-compatible API"
echo "- Vector search and embeddings"
echo "- Encrypted data sharing"
echo "- Cross-device sync"
echo "- Comprehensive web UI"
echo ""
echo "Starting PluresDB server..."
echo ""
echo "Web UI will be available at: http://localhost:34568"
echo "API will be available at: http://localhost:34567"
echo ""
echo "Press Ctrl+C to stop the server"
echo ""
./pluresdb serve --port 34567
EOF
    chmod +x "$package_dir/install.sh"
    
    # Create tarball
    tar -czf "$OUTPUT_DIR/pluresdb-$os-$arch.tar.gz" -C "$package_dir" .
    rm -rf "$package_dir"
}

# Function to create macOS package
create_macos_package() {
    echo "📦 Creating macOS package..."
    
    local arch=$(uname -m)
    local package_dir="$OUTPUT_DIR/macos-$arch"
    
    mkdir -p "$package_dir"
    
    # Copy binary
    cp "$OUTPUT_DIR/pluresdb" "$package_dir/"
    chmod +x "$package_dir/pluresdb"
    
    # Copy web UI
    cp -r "../../web/dist" "$package_dir/web"
    
    # Copy config files
    cp "../../deno.json" "$package_dir/"
    cp "../../src/config.ts" "$package_dir/"
    
    # Copy README and LICENSE
    cp "../../README.md" "$package_dir/"
    cp "../../LICENSE" "$package_dir/"
    
    # Create installer script
    cat > "$package_dir/install.sh" << 'EOF'
#!/bin/bash
echo "Installing PluresDB..."
echo ""
echo "PluresDB is a P2P Graph Database with SQLite Compatibility"
echo ""
echo "Features:"
echo "- Local-first data storage"
echo "- P2P synchronization"
echo "- SQLite-compatible API"
echo "- Vector search and embeddings"
echo "- Encrypted data sharing"
echo "- Cross-device sync"
echo "- Comprehensive web UI"
echo ""
echo "Starting PluresDB server..."
echo ""
echo "Web UI will be available at: http://localhost:34568"
echo "API will be available at: http://localhost:34567"
echo ""
echo "Press Ctrl+C to stop the server"
echo ""
./pluresdb serve --port 34567
EOF
    chmod +x "$package_dir/install.sh"
    
    # Create tarball
    tar -czf "$OUTPUT_DIR/pluresdb-macos-$arch.tar.gz" -C "$package_dir" .
    rm -rf "$package_dir"
}

# Function to create Deno package
create_deno_package() {
    echo "📦 Creating Deno package..."
    
    local deno_dir="$OUTPUT_DIR/deno"
    mkdir -p "$deno_dir"
    
    # Copy source files
    cp -r "../../src" "$deno_dir/"
    cp -r "../../examples" "$deno_dir/"
    cp "../../README.md" "$deno_dir/"
    cp "../../LICENSE" "$deno_dir/"
    cp "../deno/deno.json" "$deno_dir/"
    
    # Create tarball
    tar -czf "$OUTPUT_DIR/pluresdb-deno.tar.gz" -C "$deno_dir" .
    rm -rf "$deno_dir"
}

# Function to create NixOS package
create_nixos_package() {
    echo "📦 Creating NixOS package..."
    
    local nix_dir="$OUTPUT_DIR/nixos"
    mkdir -p "$nix_dir"
    
    # Copy Nix files
    cp ../nixos/* "$nix_dir/"
    
    # Create tarball
    tar -czf "$OUTPUT_DIR/pluresdb-nixos.tar.gz" -C "$nix_dir" .
    rm -rf "$nix_dir"
}

# Function to create Homebrew formula
create_homebrew_formula() {
    echo "📦 Creating Homebrew formula..."
    
    local formula_dir="$OUTPUT_DIR/homebrew"
    mkdir -p "$formula_dir"
    
    # Create formula
    cat > "$formula_dir/pluresdb.rb" << EOF
class PluresDB < Formula
  desc "P2P Graph Database with SQLite Compatibility"
  homepage "https://github.com/pluresdb/pluresdb"
  url "https://github.com/pluresdb/pluresdb/releases/download/v$VERSION/pluresdb-macos-\#{Hardware::CPU.arch}.tar.gz"
  sha256 "PLACEHOLDER_SHA256"
  license "MIT"

  depends_on "deno"

  def install
    bin.install "pluresdb"
    libexec.install "web"
    libexec.install "deno.json"
    libexec.install "config.ts"
  end

  test do
    system "#{bin}/pluresdb", "--version"
  end
end
EOF
    
    # Create tarball
    tar -czf "$OUTPUT_DIR/pluresdb-homebrew.tar.gz" -C "$formula_dir" .
    rm -rf "$formula_dir"
}

# Main execution
main() {
    run_tests
    build_webui
    build_deno_binary
    
    # Detect OS and create appropriate package
    case "$(uname -s)" in
        Linux*)
            create_linux_package
            ;;
        Darwin*)
            create_macos_package
            create_homebrew_formula
            ;;
        *)
            echo "❌ Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac
    
    create_deno_package
    create_nixos_package
    
    echo "✅ All packages built successfully!"
    echo "📁 Output directory: $OUTPUT_DIR"
    
    # List created files
    echo ""
    echo "📋 Created packages:"
    ls -la "$OUTPUT_DIR" | grep -v "^total" | awk '{print "  - " $9}'
    
    echo ""
    echo "🚀 Next steps:"
    echo "  1. Test the packages"
    echo "  2. Upload to GitHub Releases"
    echo "  3. Submit Homebrew formula to homebrew-core"
    echo "  4. Submit NixOS package to nixpkgs"
}

# Run main function
main "$@"
