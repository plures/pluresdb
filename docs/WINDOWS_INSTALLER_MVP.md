# PluresDB Windows Installer - MVP Release Notes

## Current Development Status

### ✅ Completed Infrastructure

1. **Packaging Scripts**
   - ✅ `install.ps1` - PowerShell installation script with multiple package manager support
   - ✅ `packaging/scripts/build-packages.ps1` - Comprehensive build script for all platforms
   - ✅ `packaging/msi/pluresdb.wxs` - WiX MSI installer definition
   - ✅ `packaging/winget/pluresdb.yaml` - Windows Package Manager manifest

2. **Core Application**
   - ✅ TypeScript/Deno implementation complete
   - ✅ SQLite-compatible API
   - ✅ Web UI (24-tab management interface)
   - ✅ REST API and WebSocket support
   - ✅ Vector search and AI features
   - ✅ P2P networking capabilities

3. **Documentation**
   - ✅ Comprehensive README.md
   - ✅ Installation guide (packaging/INSTALLATION.md)
   - ✅ Windows getting started guide
   - ✅ API documentation

### 🚧 Current Limitations (MVP)

1. **Build Process**
   - The Windows executable needs to be compiled using `deno compile` on a Windows machine or with proper SSL certificates configured
   - The MSI installer requires WiX Toolset to be installed
   - Pre-built binaries are not yet available in GitHub Releases

2. **Testing Status**
   - Core functionality is implemented and unit tested
   - End-to-end Windows installation testing needs to be performed
   - MSI installer has been defined but not yet built and tested

3. **Distribution**
   - Winget manifest exists but package is not yet published to Microsoft's repository
   - MSI installer is defined but needs to be built with actual binaries
   - Docker images are available as an alternative

## Building the Windows Package

### Prerequisites

1. **Required Software**
   - Deno 2.5.6 or later
   - Node.js 20.x or later
   - (Optional) WiX Toolset 3.11+ for MSI creation

2. **Install Deno on Windows**
   ```powershell
   # Using PowerShell
   irm https://deno.land/install.ps1 | iex
   ```

### Build Steps

1. **Clone the Repository**
   ```powershell
   git clone https://github.com/plures/pluresdb.git
   cd pluresdb
   ```

2. **Install Dependencies**
   ```powershell
   npm install
   ```

3. **Build TypeScript**
   ```powershell
   npm run build:lib
   ```

4. **Build Web UI**
   ```powershell
   npm run build:web
   ```

5. **Compile Windows Executable**
   ```powershell
   # Compile for Windows
   deno compile -A --unstable-kv `
     --target x86_64-pc-windows-msvc `
     --output pluresdb.exe `
     legacy/main.ts
   ```

6. **Create Windows Package**
   ```powershell
   # Run the packaging script
   cd packaging/scripts
   .\build-packages.ps1 -Version 1.0.1
   ```

   This will create:
   - `dist/pluresdb-windows-x64.zip` - Portable ZIP package
   - `dist/pluresdb.msi` - MSI installer (if WiX is installed)

### Manual Package Creation

If the automated script fails, you can create a manual package:

```powershell
# Create package directory
$packageDir = "pluresdb-windows-x64"
New-Item -ItemType Directory -Path $packageDir

# Copy files
Copy-Item pluresdb.exe $packageDir\
Copy-Item -Recurse web\dist $packageDir\web
Copy-Item deno.json $packageDir\
Copy-Item README.md $packageDir\
Copy-Item LICENSE $packageDir\

# Create install script
@"
@echo off
echo PluresDB - P2P Graph Database
echo.
echo Starting PluresDB server...
echo Web UI: http://localhost:34568
echo API: http://localhost:34567
echo.
pluresdb.exe serve
"@ | Out-File -FilePath "$packageDir\start.bat" -Encoding ASCII

# Create ZIP
Compress-Archive -Path "$packageDir\*" -DestinationPath "pluresdb-windows-x64.zip"
```

## Installation Options for Users

### Option 1: Using the PowerShell Install Script

```powershell
# Download and run the installer
irm https://raw.githubusercontent.com/plures/pluresdb/main/install.ps1 | iex
```

The script will:
- Try to install via winget (if available)
- Try to install via Chocolatey (if available)
- Try to install via Scoop (if available)
- Fall back to downloading and extracting the ZIP package
- Add pluresdb to PATH
- Create a desktop shortcut

### Option 2: Using winget (Once Published)

```powershell
winget install plures.pluresdb
```

Note: The package needs to be submitted to the Microsoft winget-pkgs repository first.

### Option 3: Using MSI Installer

1. Download `pluresdb.msi` from releases
2. Double-click to run the installer
3. Follow the installation wizard
4. Launch from Start Menu

### Option 4: Portable ZIP

1. Download `pluresdb-windows-x64.zip` from releases
2. Extract to any folder
3. Run `start.bat` or `pluresdb.exe serve`
4. Access Web UI at http://localhost:34568

## Using Docker (Alternative)

If building the Windows executable is problematic, users can use Docker:

```powershell
# Pull and run
docker run -p 34567:34567 -p 34568:34568 plures/pluresdb:latest

# With persistent storage
docker run -p 34567:34567 -p 34568:34568 `
  -v pluresdb-data:/app/data `
  plures/pluresdb:latest
```

## Configuration

After installation, configure PluresDB:

```powershell
# Start the server
pluresdb serve

# Or with custom settings
pluresdb serve --port 34567 --data-dir C:\MyData\pluresdb

# Configure via CLI
pluresdb config set port 8080
pluresdb config set host localhost
```

## Verification

After installation, verify it's working:

```powershell
# Check version
pluresdb --version

# Start server
pluresdb serve

# In another terminal, test API
curl http://localhost:34567/api/config

# Open Web UI
start http://localhost:34568
```

## Known Issues and Workarounds

### Issue 1: Certificate Errors During Build

**Problem**: `deno compile` may fail with SSL certificate errors in some environments.

**Workaround**: 
- Build on actual Windows machine instead of Linux
- Or use `--cert` flag with proper certificates
- Or use pre-built binaries once available

### Issue 2: Port Already in Use

**Problem**: Ports 34567 or 34568 are already in use.

**Workaround**:
```powershell
# Use different ports
pluresdb serve --port 8080 --web-port 8081
```

### Issue 3: Firewall Blocking

**Problem**: Windows Firewall blocks connections.

**Workaround**:
```powershell
# Add firewall rule (run as Administrator)
netsh advfirewall firewall add rule name="PluresDB" `
  dir=in action=allow protocol=TCP localport=34567,34568
```

### Issue 4: Data Not Persisting

**Problem**: Database doesn't save between restarts.

**Workaround**:
```powershell
# Specify data directory explicitly
pluresdb serve --data-dir %USERPROFILE%\.pluresdb\data
```

## Next Steps for Production Release

1. **Build Automation**
   - [ ] Set up GitHub Actions CI/CD for Windows builds
   - [ ] Automate MSI creation in CI
   - [ ] Create release artifacts automatically
   - [ ] Sign executables and MSI with code signing certificate

2. **Testing**
   - [ ] End-to-end installation testing on Windows 10/11
   - [ ] Test MSI installation and uninstallation
   - [ ] Verify all features work on Windows
   - [ ] Performance testing on Windows

3. **Distribution**
   - [ ] Submit to Microsoft winget-pkgs repository
   - [ ] (Optional) Submit to Chocolatey
   - [ ] (Optional) Submit to Scoop buckets
   - [ ] Publish to GitHub Releases with binaries

4. **Documentation**
   - [ ] Create video tutorial for Windows users
   - [ ] Add screenshots to documentation
   - [ ] Create troubleshooting guide specific to Windows
   - [ ] Add examples for Windows-specific scenarios

5. **User Experience**
   - [ ] Create system tray icon
   - [ ] Add "Run at startup" option
   - [ ] Improve error messages for Windows users
   - [ ] Add Windows-specific shortcuts and integrations

## Contributing

To help complete the Windows installer MVP:

1. **Test Installation**: Try the installation methods above and report issues
2. **Build Packages**: Build and test the packages on your Windows machine
3. **Documentation**: Improve Windows-specific documentation
4. **Code Review**: Review the packaging scripts and MSI definitions
5. **CI/CD**: Help set up GitHub Actions for automated Windows builds

## Support

- **GitHub Issues**: https://github.com/plures/pluresdb/issues
- **Discussions**: https://github.com/plures/pluresdb/discussions
- **Documentation**: https://github.com/plures/pluresdb/wiki

## License

PluresDB is licensed under AGPL-3.0. See LICENSE file for details.

---

**Version**: 1.0.1  
**Status**: MVP - Core functionality complete, installer testing in progress  
**Last Updated**: November 6, 2025
