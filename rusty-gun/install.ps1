# Rusty Gun Installation Script for Windows
# This script installs Rusty Gun on Windows

param(
    [string]$Version = "1.0.0",
    [string]$InstallDir = "$env:USERPROFILE\.local\bin",
    [string]$ConfigDir = "$env:USERPROFILE\.config\rusty-gun",
    [string]$DataDir = "$env:USERPROFILE\.local\share\rusty-gun",
    [switch]$Help
)

# Show help
if ($Help) {
    Write-Host "Rusty Gun Installation Script for Windows" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Usage: .\install.ps1 [OPTIONS]" -ForegroundColor White
    Write-Host ""
    Write-Host "Options:" -ForegroundColor Yellow
    Write-Host "  -Version VERSION     Install specific version (default: $Version)" -ForegroundColor White
    Write-Host "  -InstallDir DIR      Installation directory (default: $InstallDir)" -ForegroundColor White
    Write-Host "  -ConfigDir DIR       Configuration directory (default: $ConfigDir)" -ForegroundColor White
    Write-Host "  -DataDir DIR         Data directory (default: $DataDir)" -ForegroundColor White
    Write-Host "  -Help                Show this help message" -ForegroundColor White
    Write-Host ""
    Write-Host "Examples:" -ForegroundColor Yellow
    Write-Host "  .\install.ps1                    # Install latest version" -ForegroundColor White
    Write-Host "  .\install.ps1 -Version 1.0.0     # Install specific version" -ForegroundColor White
    Write-Host "  .\install.ps1 -InstallDir C:\bin # Custom installation directory" -ForegroundColor White
    exit 0
}

# Function to print colored output
function Write-Info {
    param([string]$Message)
    Write-Host "ℹ️  $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "✅ $Message" -ForegroundColor Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "⚠️  $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "❌ $Message" -ForegroundColor Red
}

# Function to check if command exists
function Test-Command {
    param([string]$Command)
    $null = Get-Command $Command -ErrorAction SilentlyContinue
    return $?
}

# Function to install using winget
function Install-WithWinget {
    if (Test-Command "winget") {
        Write-Info "Installing via winget..."
        try {
            winget install rusty-gun.rusty-gun
            Write-Success "Installed via winget!"
            return $true
        }
        catch {
            Write-Warning "Winget installation failed: $($_.Exception.Message)"
            return $false
        }
    }
    return $false
}

# Function to install using Chocolatey
function Install-WithChocolatey {
    if (Test-Command "choco") {
        Write-Info "Installing via Chocolatey..."
        try {
            choco install rusty-gun
            Write-Success "Installed via Chocolatey!"
            return $true
        }
        catch {
            Write-Warning "Chocolatey installation failed: $($_.Exception.Message)"
            return $false
        }
    }
    return $false
}

# Function to install using Scoop
function Install-WithScoop {
    if (Test-Command "scoop") {
        Write-Info "Installing via Scoop..."
        try {
            scoop install rusty-gun
            Write-Success "Installed via Scoop!"
            return $true
        }
        catch {
            Write-Warning "Scoop installation failed: $($_.Exception.Message)"
            return $false
        }
    }
    return $false
}

# Function to download and install binary
function Install-Binary {
    param([string]$Version, [string]$InstallDir, [string]$ConfigDir, [string]$DataDir)
    
    $url = "https://github.com/rusty-gun/rusty-gun/releases/download/v$Version/rusty-gun-windows-x64.zip"
    
    Write-Info "Downloading Rusty Gun v$Version for Windows x64..."
    
    # Create temporary directory
    $tempDir = [System.IO.Path]::GetTempPath() + [System.Guid]::NewGuid().ToString()
    New-Item -ItemType Directory -Path $tempDir | Out-Null
    
    try {
        # Download file
        $zipPath = Join-Path $tempDir "rusty-gun.zip"
        Invoke-WebRequest -Uri $url -OutFile $zipPath
        
        # Extract zip
        Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force
        
        # Create directories
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
        New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
        
        # Install binary
        $binaryPath = Join-Path $tempDir "rusty-gun.exe"
        if (Test-Path $binaryPath) {
            Copy-Item $binaryPath $InstallDir
        }
        
        # Install web UI
        $webSource = Join-Path $tempDir "web"
        if (Test-Path $webSource) {
            Copy-Item $webSource $DataDir -Recurse -Force
        }
        
        # Install config files
        $configSource = Join-Path $tempDir "deno.json"
        if (Test-Path $configSource) {
            Copy-Item $configSource $ConfigDir
        }
        
        $configTsSource = Join-Path $tempDir "config.ts"
        if (Test-Path $configTsSource) {
            Copy-Item $configTsSource $ConfigDir
        }
        
        Write-Success "Rusty Gun installed successfully!"
    }
    finally {
        # Cleanup
        Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Function to add to PATH
function Add-ToPath {
    param([string]$InstallDir)
    
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    
    if ($currentPath -notlike "*$InstallDir*") {
        Write-Info "Adding $InstallDir to PATH..."
        $newPath = "$currentPath;$InstallDir"
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        Write-Success "Added to PATH. Please restart your terminal."
    }
}

# Function to create desktop shortcut
function New-DesktopShortcut {
    param([string]$InstallDir)
    
    $desktopPath = [Environment]::GetFolderPath("Desktop")
    $shortcutPath = Join-Path $desktopPath "Rusty Gun.lnk"
    $targetPath = Join-Path $InstallDir "rusty-gun.exe"
    
    if (Test-Path $targetPath) {
        $WshShell = New-Object -comObject WScript.Shell
        $Shortcut = $WshShell.CreateShortcut($shortcutPath)
        $Shortcut.TargetPath = $targetPath
        $Shortcut.Arguments = "serve"
        $Shortcut.WorkingDirectory = $InstallDir
        $Shortcut.Description = "Rusty Gun - P2P Graph Database"
        $Shortcut.Save()
        
        Write-Success "Desktop shortcut created!"
    }
}

# Main installation function
function Main {
    Write-Info "Installing Rusty Gun v$Version..."
    
    # Try package managers first
    if (Install-WithWinget) {
        return
    }
    
    if (Install-WithChocolatey) {
        return
    }
    
    if (Install-WithScoop) {
        return
    }
    
    # Fall back to binary installation
    Write-Info "Installing binary..."
    Install-Binary -Version $Version -InstallDir $InstallDir -ConfigDir $ConfigDir -DataDir $DataDir
    
    # Add to PATH
    Add-ToPath -InstallDir $InstallDir
    
    # Create desktop shortcut
    New-DesktopShortcut -InstallDir $InstallDir
    
    Write-Success "Installation complete!"
    Write-Info "Run 'rusty-gun serve' to start the server"
    Write-Info "Web UI will be available at: http://localhost:34568"
    Write-Info "API will be available at: http://localhost:34567"
}

# Run main function
try {
    Main
}
catch {
    Write-Error "Installation failed: $($_.Exception.Message)"
    exit 1
}
