# Build Packages Script for PluresDB
# This script builds packages for all supported platforms and package managers

param(
    [string]$Version,
    [string]$OutputDir = "dist",
    [switch]$SkipTests = $false,
    [switch]$SkipWebUI = $false
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = (Resolve-Path (Join-Path $ScriptRoot "..\..")).Path
$OutputPath = Join-Path $ScriptRoot $OutputDir

if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargoPath = Join-Path $RepoRoot "Cargo.toml"
    if (-not (Test-Path $cargoPath)) {
        throw "Cargo.toml not found at $cargoPath. Provide -Version explicitly."
    }

    $cargoContent = Get-Content $cargoPath
    $packageVersion = $null
    $inPackageSection = $false

    foreach ($line in $cargoContent) {
        if ($line -match '^\s*\[.+\]') {
            $inPackageSection = $line -match '^\s*\[package\]'
            continue
        }

        if ($inPackageSection -and $line -match '^\s*version\s*=\s*"(?<ver>.+?)"') {
            $packageVersion = $matches['ver']
            break
        }
    }

    if (-not $packageVersion) {
        throw "Unable to determine version from Cargo.toml. Provide -Version explicitly."
    }

    $Version = $packageVersion
}

Write-Host "🚀 Building PluresDB Packages v$Version" -ForegroundColor Green

Push-Location $ScriptRoot

# Function to run tests
function Test-Project {
    if (-not $SkipTests) {
        Write-Host "🧪 Running tests..." -ForegroundColor Yellow
        Push-Location $RepoRoot
        try {
            deno test -A
        }
        finally {
            Pop-Location
        }
    }
}

# Function to build web UI
function Build-WebUI {
    if (-not $SkipWebUI) {
        Write-Host "🎨 Building web UI..." -ForegroundColor Yellow
        $webPath = Join-Path $RepoRoot "web\svelte"
        Push-Location $webPath
        try {
            npm install
            npm run build
        }
        finally {
            Pop-Location
        }
    }
}

# Function to build Deno binary
function Build-DenoBinary {
    Write-Host "🔨 Building Deno binary..." -ForegroundColor Yellow
    $outputBinary = Join-Path $OutputPath "pluresdb.exe"
    Push-Location $RepoRoot
    try {
        deno compile -A --no-lock --output $outputBinary src/main.ts
    }
    finally {
        Pop-Location
    }
}

# Function to create Windows ZIP package
function New-WindowsZip {
    Write-Host "📦 Creating Windows ZIP package..." -ForegroundColor Yellow
    
    $zipDir = Join-Path $OutputPath "windows-x64"
    New-Item -ItemType Directory -Path $zipDir | Out-Null
    
    # Copy binary
    Copy-Item (Join-Path $OutputPath "pluresdb.exe") "$zipDir\"
    
    # Copy web UI
    Copy-Item (Join-Path $RepoRoot "web\dist") "$zipDir\web" -Recurse
    
    # Copy config files
    Copy-Item (Join-Path $RepoRoot "deno.json") "$zipDir\"
    Copy-Item (Join-Path $RepoRoot "src\config.ts") "$zipDir\"
    
    # Copy README and LICENSE
    Copy-Item (Join-Path $RepoRoot "README.md") "$zipDir\"
    Copy-Item (Join-Path $RepoRoot "LICENSE") "$zipDir\"
    
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
    Compress-Archive -Path "$zipDir\*" -DestinationPath (Join-Path $OutputPath "pluresdb-windows-x64.zip") -Force
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
    
    # Derive MSI-safe version (WiX requires numeric Major.Minor.Build[.Revision])
    try {
        $msiVersion = ([Version]($Version.Split('-')[0])).ToString()
    } catch {
        throw "Version '$Version' is not a valid MSI product version."
    }

    # Discover hashed web asset filenames for WiX variables
    $webAssetsPath = Join-Path $RepoRoot "web\dist\assets"
    if (-not (Test-Path $webAssetsPath)) {
        throw "Web assets folder not found at $webAssetsPath. Build the web UI before creating the MSI."
    }

    $cssFile = Get-ChildItem -Path $webAssetsPath -Filter "index-*.css" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $jsFile = Get-ChildItem -Path $webAssetsPath -Filter "index-*.js" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

    if (-not $cssFile -or -not $jsFile) {
        throw "Unable to locate built web assets (index-*.css/js) in $webAssetsPath."
    }

    # Prepare source directory for MSI
    $msiSourceDir = Join-Path $OutputPath "msi-source"
    New-Item -ItemType Directory -Path $msiSourceDir | Out-Null
    
    # Copy files
    Copy-Item (Join-Path $OutputPath "pluresdb.exe") "$msiSourceDir\"

    $msiWebDistDir = Join-Path $msiSourceDir "web\dist"
    New-Item -ItemType Directory -Path $msiWebDistDir -Force | Out-Null
    Copy-Item (Join-Path $RepoRoot "web\dist\*") $msiWebDistDir -Recurse

    Copy-Item (Join-Path $RepoRoot "deno.json") "$msiSourceDir\"

    $msiSrcDir = Join-Path $msiSourceDir "src"
    New-Item -ItemType Directory -Path $msiSrcDir -Force | Out-Null
    Copy-Item (Join-Path $RepoRoot "src\config.ts") $msiSrcDir
    
    # Compile WiX source
    $candleArgs = @(
        (Join-Path $ScriptRoot "..\msi\pluresdb.wxs")
        "-o", (Join-Path $OutputPath "pluresdb.wixobj")
        "-dSourceDir=$msiSourceDir"
        "-dProductVersion=$msiVersion"
        "-dWebCssFile=$($cssFile.Name)"
        "-dWebJsFile=$($jsFile.Name)"
        "-ext", "WixUIExtension"
    )
    & candle.exe @candleArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "WiX compilation failed!"
        return
    }
    
    # Link MSI
    $lightArgs = @(
        (Join-Path $OutputPath "pluresdb.wixobj")
        "-o", (Join-Path $OutputPath "pluresdb.msi")
        "-ext", "WixUIExtension"
    )
    & light.exe @lightArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "MSI linking failed!"
        return
    }
    
    # Cleanup
    Remove-Item (Join-Path $OutputPath "pluresdb.wixobj")
    Remove-Item $msiSourceDir -Recurse -Force
}

# Function to create Deno package
function New-DenoPackage {
    Write-Host "📦 Creating Deno package..." -ForegroundColor Yellow
    
    $denoDir = Join-Path $OutputPath "deno"
    New-Item -ItemType Directory -Path $denoDir | Out-Null
    
    # Copy source files
    Copy-Item (Join-Path $RepoRoot "src") "$denoDir\" -Recurse
    Copy-Item (Join-Path $RepoRoot "examples") "$denoDir\" -Recurse
    Copy-Item (Join-Path $RepoRoot "README.md") "$denoDir\"
    Copy-Item (Join-Path $RepoRoot "LICENSE") "$denoDir\"
    Copy-Item (Join-Path $ScriptRoot "..\deno\deno.json") "$denoDir\"
    
    # Create ZIP
    Compress-Archive -Path "$denoDir\*" -DestinationPath (Join-Path $OutputPath "pluresdb-deno.zip") -Force
    Remove-Item $denoDir -Recurse -Force
}

# Function to create NixOS package
function New-NixOSPackage {
    Write-Host "📦 Creating NixOS package..." -ForegroundColor Yellow
    
    $nixDir = Join-Path $OutputPath "nixos"
    New-Item -ItemType Directory -Path $nixDir | Out-Null
    
    # Copy Nix files
    Copy-Item (Join-Path $ScriptRoot "..\nixos\*") "$nixDir\"
    
    # Create ZIP
    Compress-Archive -Path "$nixDir\*" -DestinationPath (Join-Path $OutputPath "pluresdb-nixos.zip") -Force
    Remove-Item $nixDir -Recurse -Force
}

# Function to update winget manifest
function Update-WingetManifest {
    Write-Host "📦 Updating winget manifest..." -ForegroundColor Yellow
    
    $manifestPath = Join-Path $ScriptRoot "..\winget\pluresdb.yaml"
    $manifest = Get-Content $manifestPath -Raw
    
    # Update version
    $manifest = $manifest -replace "PackageVersion: .*", "PackageVersion: $Version"
    
    # Update download URL
    $manifest = $manifest -replace "InstallerUrl: .*", "InstallerUrl: https://github.com/pluresdb/pluresdb/releases/download/v$Version/pluresdb-windows-x64.zip"

    # Calculate SHA256 of generated ZIP package
    $zipPath = Join-Path $OutputPath "pluresdb-windows-x64.zip"
    if (-not (Test-Path $zipPath)) {
        throw "Expected Windows ZIP package at $zipPath to compute InstallerSha256."
    }

    $zipHash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToUpper()
    $manifest = $manifest -replace "InstallerSha256: .*", "InstallerSha256: $zipHash"
    
    $manifest | Out-File -FilePath $manifestPath -Encoding UTF8
}

# Main execution
try {
    # Create output directory
    if (Test-Path $OutputPath) {
        Remove-Item $OutputPath -Recurse -Force
    }
    New-Item -ItemType Directory -Path $OutputPath | Out-Null

    Test-Project
    Build-WebUI
    Build-DenoBinary
    New-WindowsZip
    New-MSIInstaller
    New-DenoPackage
    New-NixOSPackage
    Update-WingetManifest
    
    Write-Host "✅ All packages built successfully!" -ForegroundColor Green
    Write-Host "📁 Output directory: $OutputPath" -ForegroundColor Cyan
    
    # List created files
    Write-Host "`n📋 Created packages:" -ForegroundColor Yellow
    Get-ChildItem $OutputPath -Name | ForEach-Object { Write-Host "  - $_" -ForegroundColor White }
    
    Write-Host "`n🚀 Next steps:" -ForegroundColor Green
    Write-Host "  1. Test the packages" -ForegroundColor White
    Write-Host "  2. Upload to GitHub Releases" -ForegroundColor White
    Write-Host "  3. Submit winget manifest to Microsoft" -ForegroundColor White
    Write-Host "  4. Submit NixOS package to nixpkgs" -ForegroundColor White
    
} catch {
    Write-Error "Build failed: $($_.Exception.Message)"
    exit 1
}
finally {
    Pop-Location
}
