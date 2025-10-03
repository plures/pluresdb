# Build Packages Script for PluresDB
# This script builds packages for all supported platforms and package managers

param(
    [string]$Version = "1.0.0",
    [string]$OutputDir = "dist",
    [switch]$SkipTests = $false,
    [switch]$SkipWebUI = $false
)

Write-Host "🚀 Building PluresDB Packages v$Version" -ForegroundColor Green

# Create output directory
if (Test-Path $OutputDir) {
    Remove-Item $OutputDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutputDir | Out-Null

# Function to run tests
function Test-Project {
    if (-not $SkipTests) {
        Write-Host "🧪 Running tests..." -ForegroundColor Yellow
        Set-Location "..\..\"
        deno test -A
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Tests failed!"
            exit 1
        }
        Set-Location "packaging\scripts"
    }
}

# Function to build web UI
function Build-WebUI {
    if (-not $SkipWebUI) {
        Write-Host "🎨 Building web UI..." -ForegroundColor Yellow
        Set-Location "..\..\web\svelte"
        npm install
        npm run build
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Web UI build failed!"
            exit 1
        }
        Set-Location "..\..\..\packaging\scripts"
    }
}

# Function to build Deno binary
function Build-DenoBinary {
    Write-Host "🔨 Building Deno binary..." -ForegroundColor Yellow
    Set-Location "..\..\"
    deno compile -A --output "packaging\scripts\$OutputDir\pluresdb.exe" src/main.ts
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Deno binary build failed!"
        exit 1
    }
    Set-Location "packaging\scripts"
}

# Function to create Windows ZIP package
function New-WindowsZip {
    Write-Host "📦 Creating Windows ZIP package..." -ForegroundColor Yellow
    
    $zipDir = "$OutputDir\windows-x64"
    New-Item -ItemType Directory -Path $zipDir | Out-Null
    
    # Copy binary
    Copy-Item "$OutputDir\pluresdb.exe" "$zipDir\"
    
    # Copy web UI
    Copy-Item "..\..\web\dist" "$zipDir\web" -Recurse
    
    # Copy config files
    Copy-Item "..\..\deno.json" "$zipDir\"
    Copy-Item "..\..\src\config.ts" "$zipDir\"
    
    # Copy README and LICENSE
    Copy-Item "..\..\README.md" "$zipDir\"
    Copy-Item "..\..\LICENSE" "$zipDir\"
    
    # Create installer script
    $installScript = @"
@echo off
echo Installing PluresDB...
echo.
echo PluresDB is a P2P Graph Database with SQLite Compatibility
echo.
echo Features:
echo - Local-first data storage
echo - P2P synchronization
echo - SQLite-compatible API
echo - Vector search and embeddings
echo - Encrypted data sharing
echo - Cross-device sync
echo - Comprehensive web UI
echo.
echo Starting PluresDB server...
echo.
echo Web UI will be available at: http://localhost:34568
echo API will be available at: http://localhost:34567
echo.
echo Press Ctrl+C to stop the server
echo.
pluresdb.exe serve --port 34567
"@
    $installScript | Out-File -FilePath "$zipDir\install.bat" -Encoding ASCII
    
    # Create ZIP
    Compress-Archive -Path "$zipDir\*" -DestinationPath "$OutputDir\pluresdb-windows-x64.zip" -Force
    Remove-Item $zipDir -Recurse -Force
}

# Function to create MSI installer
function New-MSIInstaller {
    Write-Host "📦 Creating MSI installer..." -ForegroundColor Yellow
    
    # Check if WiX is installed
    $wixPath = Get-Command "candle.exe" -ErrorAction SilentlyContinue
    if (-not $wixPath) {
        Write-Warning "WiX Toolset not found. Skipping MSI creation."
        Write-Host "To create MSI installers, install WiX Toolset from: https://wixtoolset.org/"
        return
    }
    
    # Prepare source directory for MSI
    $msiSourceDir = "$OutputDir\msi-source"
    New-Item -ItemType Directory -Path $msiSourceDir | Out-Null
    
    # Copy files
    Copy-Item "$OutputDir\pluresdb.exe" "$msiSourceDir\"
    Copy-Item "..\..\web\dist" "$msiSourceDir\web" -Recurse
    Copy-Item "..\..\deno.json" "$msiSourceDir\"
    Copy-Item "..\..\src\config.ts" "$msiSourceDir\"
    
    # Create assets directory
    New-Item -ItemType Directory -Path "$msiSourceDir\assets" | Out-Null
    
    # Compile WiX source
    & candle.exe "..\msi\pluresdb.wxs" -o "$OutputDir\pluresdb.wixobj" -dSourceDir="$msiSourceDir"
    if ($LASTEXITCODE -ne 0) {
        Write-Error "WiX compilation failed!"
        return
    }
    
    # Link MSI
    & light.exe "$OutputDir\pluresdb.wixobj" -o "$OutputDir\pluresdb.msi"
    if ($LASTEXITCODE -ne 0) {
        Write-Error "MSI linking failed!"
        return
    }
    
    # Cleanup
    Remove-Item "$OutputDir\pluresdb.wixobj"
    Remove-Item $msiSourceDir -Recurse -Force
}

# Function to create Deno package
function New-DenoPackage {
    Write-Host "📦 Creating Deno package..." -ForegroundColor Yellow
    
    $denoDir = "$OutputDir\deno"
    New-Item -ItemType Directory -Path $denoDir | Out-Null
    
    # Copy source files
    Copy-Item "..\..\src" "$denoDir\" -Recurse
    Copy-Item "..\..\examples" "$denoDir\" -Recurse
    Copy-Item "..\..\README.md" "$denoDir\"
    Copy-Item "..\..\LICENSE" "$denoDir\"
    Copy-Item "..\deno\deno.json" "$denoDir\"
    
    # Create ZIP
    Compress-Archive -Path "$denoDir\*" -DestinationPath "$OutputDir\pluresdb-deno.zip" -Force
    Remove-Item $denoDir -Recurse -Force
}

# Function to create NixOS package
function New-NixOSPackage {
    Write-Host "📦 Creating NixOS package..." -ForegroundColor Yellow
    
    $nixDir = "$OutputDir\nixos"
    New-Item -ItemType Directory -Path $nixDir | Out-Null
    
    # Copy Nix files
    Copy-Item "..\nixos\*" "$nixDir\"
    
    # Create ZIP
    Compress-Archive -Path "$nixDir\*" -DestinationPath "$OutputDir\pluresdb-nixos.zip" -Force
    Remove-Item $nixDir -Recurse -Force
}

# Function to update winget manifest
function Update-WingetManifest {
    Write-Host "📦 Updating winget manifest..." -ForegroundColor Yellow
    
    $manifestPath = "..\winget\pluresdb.yaml"
    $manifest = Get-Content $manifestPath -Raw
    
    # Update version
    $manifest = $manifest -replace "PackageVersion: .*", "PackageVersion: $Version"
    
    # Update download URL
    $manifest = $manifest -replace "InstallerUrl: .*", "InstallerUrl: https://github.com/pluresdb/pluresdb/releases/download/v$Version/pluresdb-windows-x64.zip"
    
    # Calculate SHA256 (placeholder for now)
    $sha256 = "PLACEHOLDER_SHA256"
    $manifest = $manifest -replace "InstallerSha256: .*", "InstallerSha256: $sha256"
    
    $manifest | Out-File -FilePath $manifestPath -Encoding UTF8
}

# Main execution
try {
    Test-Project
    Build-WebUI
    Build-DenoBinary
    New-WindowsZip
    New-MSIInstaller
    New-DenoPackage
    New-NixOSPackage
    Update-WingetManifest
    
    Write-Host "✅ All packages built successfully!" -ForegroundColor Green
    Write-Host "📁 Output directory: $OutputDir" -ForegroundColor Cyan
    
    # List created files
    Write-Host "`n📋 Created packages:" -ForegroundColor Yellow
    Get-ChildItem $OutputDir -Name | ForEach-Object { Write-Host "  - $_" -ForegroundColor White }
    
    Write-Host "`n🚀 Next steps:" -ForegroundColor Green
    Write-Host "  1. Test the packages" -ForegroundColor White
    Write-Host "  2. Upload to GitHub Releases" -ForegroundColor White
    Write-Host "  3. Submit winget manifest to Microsoft" -ForegroundColor White
    Write-Host "  4. Submit NixOS package to nixpkgs" -ForegroundColor White
    
} catch {
    Write-Error "Build failed: $($_.Exception.Message)"
    exit 1
}
