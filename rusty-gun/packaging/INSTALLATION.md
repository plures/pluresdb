# Installation Guide

Rusty Gun can be installed using various package managers and methods. Choose the method that works best for your system.

## Quick Start

### Docker (Recommended - Easiest)
```bash
# Pull and run with Docker
docker run -p 34567:34567 -p 34568:34568 rusty-gun/rusty-gun:latest

# Or with persistent storage
docker run -p 34567:34567 -p 34568:34568 -v rusty-gun-data:/app/data rusty-gun/rusty-gun:latest

# Open web UI
open http://localhost:34568  # macOS
start http://localhost:34568  # Windows
xdg-open http://localhost:34568  # Linux
```

### Windows (winget)
```powershell
winget install rusty-gun.rusty-gun
```

### macOS (Homebrew)
```bash
brew install rusty-gun/rusty-gun/rusty-gun
```

### Linux (NixOS)
```bash
nix-env -iA nixpkgs.rusty-gun
```

### Deno
```bash
deno install -A -n rusty-gun https://deno.land/x/rusty_gun@v1.0.0/src/main.ts
```

## Detailed Installation Methods

### 1. Windows Package Manager (winget)

The easiest way to install Rusty Gun on Windows is using winget:

```powershell
# Install Rusty Gun
winget install rusty-gun.rusty-gun

# Start the server
rusty-gun serve

# Open web UI
start http://localhost:34568
```

**Requirements:**
- Windows 10 version 1709 (build 16299) or later
- Windows Package Manager (winget) - included in Windows 10/11

### 2. Windows MSI Installer

Download and install the MSI package:

1. Download `rusty-gun.msi` from [GitHub Releases](https://github.com/rusty-gun/rusty-gun/releases)
2. Double-click the MSI file to start the installer
3. Follow the installation wizard
4. Rusty Gun will be installed to `C:\Program Files\RustyGun\`
5. Start the server from Start Menu or command line

### 3. macOS Homebrew

Install using Homebrew:

```bash
# Add the tap (if not already added)
brew tap rusty-gun/rusty-gun

# Install Rusty Gun
brew install rusty-gun

# Start the server
rusty-gun serve

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
wget https://github.com/rusty-gun/rusty-gun/releases/download/v1.0.0/rusty-gun-linux-amd64.deb

# Install
sudo dpkg -i rusty-gun-linux-amd64.deb

# Start the server
rusty-gun serve
```

#### Red Hat/CentOS/Fedora (RPM)
```bash
# Download the .rpm package
wget https://github.com/rusty-gun/rusty-gun/releases/download/v1.0.0/rusty-gun-linux-amd64.rpm

# Install
sudo rpm -i rusty-gun-linux-amd64.rpm

# Start the server
rusty-gun serve
```

#### Arch Linux (AUR)
```bash
# Using yay
yay -S rusty-gun

# Or using paru
paru -S rusty-gun

# Start the server
rusty-gun serve
```

### 5. NixOS

#### Using Nix Package Manager
```bash
# Install with nix-env
nix-env -iA nixpkgs.rusty-gun

# Or using nix-shell
nix-shell -p rusty-gun

# Start the server
rusty-gun serve
```

#### Using Nix Flake
```bash
# Clone the repository
git clone https://github.com/rusty-gun/rusty-gun.git
cd rusty-gun

# Enter development shell
nix develop

# Start the server
rusty-gun serve
```

#### NixOS Configuration
Add to your `configuration.nix`:

```nix
{ pkgs, ... }:
{
  environment.systemPackages = with pkgs; [
    rusty-gun
  ];
}
```

### 6. Deno

Install as a Deno module:

```bash
# Install globally
deno install -A -n rusty-gun https://deno.land/x/rusty_gun@v1.0.0/src/main.ts

# Start the server
rusty-gun serve

# Or run directly without installation
deno run -A https://deno.land/x/rusty_gun@v1.0.0/src/main.ts serve
```

**Requirements:**
- Deno 1.40.0 or later

### 7. Docker (Recommended)

Docker is the easiest way to get started with Rusty Gun. No installation required!

#### Quick Start with Docker

```bash
# Pull and run the latest image
docker run -p 34567:34567 -p 34568:34568 rusty-gun/rusty-gun:latest

# With persistent storage
docker run -p 34567:34567 -p 34568:34568 -v rusty-gun-data:/app/data rusty-gun/rusty-gun:latest
```

#### Using Docker Compose (Recommended)

Create a `docker-compose.yml` file:

```yaml
version: '3.8'
services:
  rusty-gun:
    image: rusty-gun/rusty-gun:latest
    ports:
      - "34567:34567"  # API port
      - "34568:34568"  # Web UI port
    volumes:
      - rusty-gun-data:/app/data
      - rusty-gun-config:/app/config
    environment:
      - RUSTY_GUN_PORT=34567
      - RUSTY_GUN_WEB_PORT=34568
      - RUSTY_GUN_HOST=0.0.0.0
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "deno", "run", "-A", "--allow-net", "src/healthcheck.ts"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  rusty-gun-data:
  rusty-gun-config:
```

Then run:
```bash
# Start Rusty Gun
docker-compose up -d

# View logs
docker-compose logs -f

# Stop Rusty Gun
docker-compose down
```

#### Production Deployment

For production, use the production configuration:

```bash
# Clone the repository
git clone https://github.com/rusty-gun/rusty-gun.git
cd rusty-gun/packaging/docker

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

1. Go to [GitHub Releases](https://github.com/rusty-gun/rusty-gun/releases)
2. Download the package for your platform:
   - Windows: `rusty-gun-windows-x64.zip`
   - macOS: `rusty-gun-macos-x64.tar.gz` or `rusty-gun-macos-arm64.tar.gz`
   - Linux: `rusty-gun-linux-x64.tar.gz` or `rusty-gun-linux-arm64.tar.gz`
3. Extract the archive
4. Run the installer script or binary directly

## Verification

After installation, verify that Rusty Gun is working:

```bash
# Check version
rusty-gun --version

# Start the server
rusty-gun serve

# In another terminal, test the API
curl http://localhost:34567/api/config

# Open the web UI
open http://localhost:34568  # macOS
start http://localhost:34568  # Windows
xdg-open http://localhost:34568  # Linux
```

## Configuration

Rusty Gun can be configured using:

1. **Command line arguments:**
   ```bash
   rusty-gun serve --port 8080 --host 0.0.0.0
   ```

2. **Configuration file:**
   ```bash
   rusty-gun config set port 8080
   rusty-gun config set host 0.0.0.0
   ```

3. **Environment variables:**
   ```bash
   export RUSTY_GUN_PORT=8080
   export RUSTY_GUN_HOST=0.0.0.0
   rusty-gun serve
   ```

## Troubleshooting

### Common Issues

1. **Port already in use:**
   ```bash
   # Use a different port
   rusty-gun serve --port 8080
   ```

2. **Permission denied:**
   ```bash
   # Make sure the binary is executable
   chmod +x rusty-gun
   ```

3. **Web UI not loading:**
   - Check if the server is running
   - Verify the port is correct
   - Check firewall settings

4. **Database not persisting:**
   - Check write permissions in the data directory
   - Verify disk space is available

### Getting Help

- **Documentation:** [GitHub Wiki](https://github.com/rusty-gun/rusty-gun/wiki)
- **Issues:** [GitHub Issues](https://github.com/rusty-gun/rusty-gun/issues)
- **Discussions:** [GitHub Discussions](https://github.com/rusty-gun/rusty-gun/discussions)
- **Discord:** [Join our Discord](https://discord.gg/rusty-gun)

## Uninstallation

### Windows (winget)
```powershell
winget uninstall rusty-gun.rusty-gun
```

### macOS (Homebrew)
```bash
brew uninstall rusty-gun
```

### Linux (Package Manager)
```bash
# Ubuntu/Debian
sudo apt remove rusty-gun

# Red Hat/CentOS/Fedora
sudo rpm -e rusty-gun

# Arch Linux
sudo pacman -R rusty-gun
```

### Manual Uninstallation
1. Stop the Rusty Gun server
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
