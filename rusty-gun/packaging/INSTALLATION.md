# Installation Guide

PluresDB can be installed using various package managers and methods. Choose the method that works best for your system.

## Quick Start

### Docker (Recommended - Easiest)
```bash
# Pull and run with Docker
docker run -p 34567:34567 -p 34568:34568 plures/pluresdb:latest

# Or with persistent storage
docker run -p 34567:34567 -p 34568:34568 -v pluresdb-data:/app/data plures/pluresdb:latest

# Open web UI
open http://localhost:34568  # macOS
start http://localhost:34568  # Windows
xdg-open http://localhost:34568  # Linux
```

### Windows (winget)
```powershell
winget install plures.pluresdb
```

### macOS (Homebrew)
```bash
brew install plures/pluresdb/pluresdb
```

### Linux (NixOS)
```bash
nix-env -iA nixpkgs.pluresdb
```

### Deno
```bash
deno install -A -n pluresdb https://deno.land/x/pluresdb@v1.0.0/src/main.ts
```

## Detailed Installation Methods

### 1. Windows Package Manager (winget)

The easiest way to install PluresDB on Windows is using winget:

```powershell
# Install PluresDB
winget install plures.pluresdb

# Start the server
pluresdb serve

# Open web UI
start http://localhost:34568
```

**Requirements:**
- Windows 10 version 1709 (build 16299) or later
- Windows Package Manager (winget) - included in Windows 10/11

### 2. Windows MSI Installer

Download and install the MSI package:

1. Download `pluresdb.msi` from [GitHub Releases](https://github.com/plures/pluresdb/releases)
2. Double-click the MSI file to start the installer
3. Follow the installation wizard
4. PluresDB will be installed to `C:\Program Files\PluresDB\`
5. Start the server from Start Menu or command line

### 3. macOS Homebrew

Install using Homebrew:

```bash
# Add the tap (if not already added)
brew tap plures/pluresdb

# Install PluresDB
brew install pluresdb

# Start the server
pluresdb serve

# Open web UI
open http://localhost:34568
```

**Requirements:**
- macOS 10.15 (Catalina) or later
- Homebrew package manager

### 4. Linux Package Managers

#### Ubuntu/Debian (APT)
```bash
# Download the .deb package
wget https://github.com/plures/pluresdb/releases/download/v1.0.0/pluresdb-linux-amd64.deb

# Install
sudo dpkg -i pluresdb-linux-amd64.deb

# Start the server
pluresdb serve
```

#### Red Hat/CentOS/Fedora (RPM)
```bash
# Download the .rpm package
wget https://github.com/plures/pluresdb/releases/download/v1.0.0/pluresdb-linux-amd64.rpm

# Install
sudo rpm -i pluresdb-linux-amd64.rpm

# Start the server
pluresdb serve
```

#### Arch Linux (AUR)
```bash
# Using yay
yay -S pluresdb

# Or using paru
paru -S pluresdb

# Start the server
pluresdb serve
```

### 5. NixOS

#### Using Nix Package Manager
```bash
# Install with nix-env
nix-env -iA nixpkgs.pluresdb

# Or using nix-shell
nix-shell -p pluresdb

# Start the server
pluresdb serve
```

#### Using Nix Flake
```bash
# Clone the repository
git clone https://github.com/plures/pluresdb.git
cd pluresdb

# Enter development shell
nix develop

# Start the server
pluresdb serve
```

#### NixOS Configuration
Add to your `configuration.nix`:

```nix
{ pkgs, ... }:
{
  environment.systemPackages = with pkgs; [
    pluresdb
  ];
}
```

### 6. Deno

Install as a Deno module:

```bash
# Install globally
deno install -A -n pluresdb https://deno.land/x/pluresdb@v1.0.0/src/main.ts

# Start the server
pluresdb serve

# Or run directly without installation
deno run -A https://deno.land/x/pluresdb@v1.0.0/src/main.ts serve
```

**Requirements:**
- Deno 1.40.0 or later

### 7. Docker (Recommended)

Docker is the easiest way to get started with PluresDB. No installation required!

#### Quick Start with Docker

```bash
# Pull and run the latest image
docker run -p 34567:34567 -p 34568:34568 plures/pluresdb:latest

# With persistent storage
docker run -p 34567:34567 -p 34568:34568 -v pluresdb-data:/app/data plures/pluresdb:latest
```

#### Using Docker Compose (Recommended)

Create a `docker-compose.yml` file:

```yaml
version: '3.8'
services:
  pluresdb:
    image: plures/pluresdb:latest
    ports:
      - "34567:34567"  # API port
      - "34568:34568"  # Web UI port
    volumes:
      - pluresdb-data:/app/data
      - pluresdb-config:/app/config
    environment:
      - PLURESDB_PORT=34567
      - PLURESDB_WEB_PORT=34568
      - PLURESDB_HOST=0.0.0.0
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "deno", "run", "-A", "--allow-net", "src/healthcheck.ts"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  pluresdb-data:
  pluresdb-config:
```

Then run:
```bash
# Start PluresDB
docker-compose up -d

# View logs
docker-compose logs -f

# Stop PluresDB
docker-compose down
```

#### Production Deployment

For production, use the production configuration:

```bash
# Clone the repository
git clone https://github.com/plures/pluresdb.git
cd pluresdb/packaging/docker

# Start with production settings
docker-compose -f docker-compose.prod.yml up -d

# With Nginx reverse proxy
docker-compose -f docker-compose.prod.yml --profile with-nginx up -d

# With Redis caching
docker-compose -f docker-compose.prod.yml --profile with-redis up -d
```

**Requirements:**
- Docker 20.10 or later
- Docker Compose 2.0 or later (optional but recommended)

**Benefits:**
- No installation required
- Consistent environment across platforms
- Easy updates and rollbacks
- Built-in health checks
- Production-ready configurations
- Automatic restarts

### 8. Manual Installation

Download the appropriate package for your system:

1. Go to [GitHub Releases](https://github.com/plures/pluresdb/releases)
2. Download the package for your platform:
   - Windows: `pluresdb-windows-x64.zip`
   - macOS: `pluresdb-macos-x64.tar.gz` or `pluresdb-macos-arm64.tar.gz`
   - Linux: `pluresdb-linux-x64.tar.gz` or `pluresdb-linux-arm64.tar.gz`
3. Extract the archive
4. Run the installer script or binary directly

## Verification

After installation, verify that PluresDB is working:

```bash
# Check version
pluresdb --version

# Start the server
pluresdb serve

# In another terminal, test the API
curl http://localhost:34567/api/config

# Open the web UI
open http://localhost:34568  # macOS
start http://localhost:34568  # Windows
xdg-open http://localhost:34568  # Linux
```

## Configuration

PluresDB can be configured using:

1. **Command line arguments:**
   ```bash
   pluresdb serve --port 8080 --host 0.0.0.0
   ```

2. **Configuration file:**
   ```bash
   pluresdb config set port 8080
   pluresdb config set host 0.0.0.0
   ```

3. **Environment variables:**
   ```bash
   export PLURESDB_PORT=8080
   export PLURESDB_HOST=0.0.0.0
   pluresdb serve
   ```

## Troubleshooting

### Common Issues

1. **Port already in use:**
   ```bash
   # Use a different port
   pluresdb serve --port 8080
   ```

2. **Permission denied:**
   ```bash
   # Make sure the binary is executable
   chmod +x pluresdb
   ```

3. **Web UI not loading:**
   - Check if the server is running
   - Verify the port is correct
   - Check firewall settings

4. **Database not persisting:**
   - Check write permissions in the data directory
   - Verify disk space is available

### Getting Help

- **Documentation:** [GitHub Wiki](https://github.com/plures/pluresdb/wiki)
- **Issues:** [GitHub Issues](https://github.com/plures/pluresdb/issues)
- **Discussions:** [GitHub Discussions](https://github.com/plures/pluresdb/discussions)
- **Discord:** [Join our Discord](https://discord.gg/pluresdb)

## Uninstallation

### Windows (winget)
```powershell
winget uninstall plures.pluresdb
```

### macOS (Homebrew)
```bash
brew uninstall pluresdb
```

### Linux (Package Manager)
```bash
# Ubuntu/Debian
sudo apt remove pluresdb

# Red Hat/CentOS/Fedora
sudo rpm -e pluresdb

# Arch Linux
sudo pacman -R pluresdb
```

### Manual Uninstallation
1. Stop the PluresDB server
2. Remove the installation directory
3. Remove configuration files (optional)
4. Remove data directory (optional)

## System Requirements

### Minimum Requirements
- **CPU:** 1 GHz processor
- **RAM:** 512 MB
- **Storage:** 100 MB free space
- **OS:** Windows 10, macOS 10.15, or Linux (kernel 3.10+)

### Recommended Requirements
- **CPU:** 2 GHz processor or better
- **RAM:** 2 GB or more
- **Storage:** 1 GB free space
- **OS:** Latest version of Windows, macOS, or Linux

### Network Requirements
- **Ports:** 34567 (API), 34568 (Web UI)
- **Firewall:** Allow incoming connections on these ports
- **Internet:** Required for P2P features and updates
