#!/bin/bash

# PluresDB Installation Script
# This script installs PluresDB on various platforms

set -e

VERSION="1.0.0"
INSTALL_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/pluresdb"
DATA_DIR="$HOME/.local/share/pluresdb"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored output
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Detect operating system
detect_os() {
    case "$(uname -s)" in
        Linux*)
            OS="linux"
            ;;
        Darwin*)
            OS="macos"
            ;;
        CYGWIN*|MINGW32*|MSYS*|MINGW*)
            OS="windows"
            ;;
        *)
            print_error "Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64)
            ARCH="x64"
            ;;
        arm64|aarch64)
            ARCH="arm64"
            ;;
        i386|i686)
            ARCH="x86"
            ;;
        *)
            print_error "Unsupported architecture: $(uname -m)"
            exit 1
            ;;
    esac
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Download and install binary
install_binary() {
    local platform="$1"
    local arch="$2"
    local url="https://github.com/pluresdb/pluresdb/releases/download/v$VERSION/pluresdb-$platform-$arch.tar.gz"
    
    print_info "Downloading PluresDB v$VERSION for $platform-$arch..."
    
    # Create temporary directory
    local temp_dir=$(mktemp -d)
    cd "$temp_dir"
    
    # Download and extract
    if command_exists curl; then
        curl -L "$url" | tar -xz
    elif command_exists wget; then
        wget -qO- "$url" | tar -xz
    else
        print_error "Neither curl nor wget found. Please install one of them."
        exit 1
    fi
    
    # Create directories
    mkdir -p "$INSTALL_DIR"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$DATA_DIR"
    
    # Install binary
    cp pluresdb "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/pluresdb"
    
    # Install web UI
    cp -r web "$DATA_DIR/"
    
    # Install config files
    cp deno.json "$CONFIG_DIR/"
    cp config.ts "$CONFIG_DIR/"
    
    # Cleanup
    cd /
    rm -rf "$temp_dir"
    
    print_success "PluresDB installed successfully!"
}

# Install using package manager
install_with_package_manager() {
    local os="$1"
    
    case "$os" in
        linux)
            if command_exists apt; then
                print_info "Installing via apt..."
                # Add repository and install
                print_warning "APT installation not yet available. Please use manual installation."
                return 1
            elif command_exists yum; then
                print_info "Installing via yum..."
                print_warning "YUM installation not yet available. Please use manual installation."
                return 1
            elif command_exists pacman; then
                print_info "Installing via pacman..."
                print_warning "Pacman installation not yet available. Please use manual installation."
                return 1
            fi
            ;;
        macos)
            if command_exists brew; then
                print_info "Installing via Homebrew..."
                brew tap pluresdb/pluresdb
                brew install pluresdb
                return 0
            fi
            ;;
        windows)
            if command_exists winget; then
                print_info "Installing via winget..."
                winget install pluresdb.pluresdb
                return 0
            fi
            ;;
    esac
    
    return 1
}

# Install using Deno
install_with_deno() {
    if command_exists deno; then
        print_info "Installing via Deno..."
        deno install -A -n pluresdb "https://deno.land/x/pluresdb@v$VERSION/src/main.ts"
        return 0
    fi
    return 1
}

# Add to PATH
add_to_path() {
    local shell_rc=""
    
    case "$SHELL" in
        */bash)
            shell_rc="$HOME/.bashrc"
            ;;
        */zsh)
            shell_rc="$HOME/.zshrc"
            ;;
        */fish)
            shell_rc="$HOME/.config/fish/config.fish"
            ;;
        *)
            shell_rc="$HOME/.profile"
            ;;
    esac
    
    if [[ "$PATH" != *"$INSTALL_DIR"* ]]; then
        print_info "Adding $INSTALL_DIR to PATH in $shell_rc"
        echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$shell_rc"
        print_success "Please restart your shell or run: source $shell_rc"
    fi
}

# Main installation function
main() {
    print_info "Installing PluresDB v$VERSION..."
    
    # Detect system
    detect_os
    detect_arch
    
    print_info "Detected: $OS-$ARCH"
    
    # Try package manager first
    if install_with_package_manager "$OS"; then
        print_success "Installed via package manager!"
        return 0
    fi
    
    # Try Deno
    if install_with_deno; then
        print_success "Installed via Deno!"
        return 0
    fi
    
    # Fall back to binary installation
    print_info "Installing binary..."
    install_binary "$OS" "$ARCH"
    
    # Add to PATH
    add_to_path
    
    print_success "Installation complete!"
    print_info "Run 'pluresdb serve' to start the server"
    print_info "Web UI will be available at: http://localhost:34568"
    print_info "API will be available at: http://localhost:34567"
}

# Show help
show_help() {
    echo "PluresDB Installation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --version VERSION    Install specific version (default: $VERSION)"
    echo "  --install-dir DIR    Installation directory (default: $INSTALL_DIR)"
    echo "  --config-dir DIR     Configuration directory (default: $CONFIG_DIR)"
    echo "  --data-dir DIR       Data directory (default: $DATA_DIR)"
    echo "  --help               Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                   # Install latest version"
    echo "  $0 --version 1.0.0   # Install specific version"
    echo "  $0 --install-dir /usr/local/bin  # Custom installation directory"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --install-dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --config-dir)
            CONFIG_DIR="$2"
            shift 2
            ;;
        --data-dir)
            DATA_DIR="$2"
            shift 2
            ;;
        --help)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Run main function
main "$@"
