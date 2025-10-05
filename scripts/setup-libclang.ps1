<#
.SYNOPSIS
    Ensures libclang (LLVM) is installed and configures the LIBCLANG_PATH required by bindgen-based crates.

.DESCRIPTION
    This script automates the detection or installation of libclang on Windows machines.
    It will:
      1. Check existing environment variables and well-known locations for libclang.dll
      2. Optionally install LLVM via winget or Chocolatey if libclang is missing
      3. Configure the LIBCLANG_PATH environment variable (user scope) once libclang is found
      4. Optionally append the path to the current session PATH so cargo build/test immediately picks it up

    Run it from the repository root before building Rust components that depend on bindgen (e.g., zstd-sys).

.PARAMETER ForceInstall
    Always attempt to (re)install LLVM even if libclang is already detected.

.PARAMETER ConfigureCurrentProcess
    Also updates $env:LIBCLANG_PATH and $env:PATH for the current PowerShell session
    in addition to setting the persisted user-level environment variable.

.PARAMETER SkipInstall
    Do not attempt to install LLVM automatically. The script will fail if libclang is not found.

.EXAMPLE
    pwsh ./scripts/setup-libclang.ps1

.EXAMPLE
    pwsh ./scripts/setup-libclang.ps1 -ConfigureCurrentProcess

.NOTES
    - Requires either winget or Chocolatey for automated installation.
    - When installing via winget, administrative consent may be required for the first run.
    - After the user-level environment variable is written you should restart your shell (unless ConfigureCurrentProcess was supplied).
#>
[CmdletBinding()]
param(
    [switch]$ForceInstall,
    [switch]$ConfigureCurrentProcess,
    [switch]$SkipInstall
)

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO]  $Message" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Message)
    Write-Host "[ OK ]  $Message" -ForegroundColor Green
}

function Write-WarningMessage {
    param([string]$Message)
    Write-Host "[WARN]  $Message" -ForegroundColor Yellow
}

function Write-ErrorMessage {
    param([string]$Message)
    Write-Host "[FAIL]  $Message" -ForegroundColor Red
}

function Test-CommandExists {
    param([string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-EnvLibClangPath {
    $env:LIBCLANG_PATH
}

function Test-LibClangInPath {
    param([string]$Directory)
    if ([string]::IsNullOrWhiteSpace($Directory)) {
        return $null
    }
    $candidate = Join-Path -Path $Directory -ChildPath "libclang.dll"
    if (Test-Path $candidate) {
        return (Get-Item $candidate).Directory.FullName
    }
    return $null
}

function Get-KnownLibClangLocations {
    $paths = @()

    # Explicit environment variable first
    $envPath = Get-EnvLibClangPath
    if (-not [string]::IsNullOrWhiteSpace($envPath)) {
        $paths += $envPath
    }

    # LLVM default installation
    $paths += @(
        "$env:ProgramFiles\LLVM\bin",
        "$env:ProgramFiles\LLVM\lib",
        "$env:ProgramFiles\LLVM\lib64",
        "$env:ProgramFiles(x86)\LLVM\bin",
        "$env:ProgramFiles(x86)\LLVM\lib"
    )

    # Visual Studio (MSVC) bundled LLVM/Clang
    $vsRoot = Join-Path $env:ProgramFiles "Microsoft Visual Studio"
    if (Test-Path $vsRoot) {
        $paths += Get-ChildItem -Path $vsRoot -Directory -Recurse -Filter "libclang.dll" -ErrorAction SilentlyContinue |
            ForEach-Object { $_.Directory.FullName } |
            Select-Object -Unique
    }

    # Generic entries that appear in PATH (e.g., if user installed LLVM elsewhere)
    $paths += [Environment]::GetEnvironmentVariable("PATH", "Machine").Split(';')
    $paths += [Environment]::GetEnvironmentVariable("PATH", "User").Split(';')

    # Remove duplicates/empty
    return $paths | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique
}

function Find-LibClang {
    $locations = Get-KnownLibClangLocations
    foreach ($location in $locations) {
        $resolved = Test-LibClangInPath -Directory $location
        if ($resolved) {
            return $resolved
        }
    }
    return $null
}

function Install-LLVM {
    Write-Info "Attempting to install LLVM (includes libclang)..."

    if (Test-CommandExists -Name "winget") {
        try {
            winget install LLVM.LLVM --silent --accept-package-agreements --accept-source-agreements | Out-Null
            Write-Success "LLVM installed via winget"
            return $true
        }
        catch {
            Write-WarningMessage "winget install failed: $($_.Exception.Message)"
        }
    }
    elseif (Test-CommandExists -Name "choco") {
        try {
            choco install llvm -y --no-progress | Out-Null
            Write-Success "LLVM installed via Chocolatey"
            return $true
        }
        catch {
            Write-WarningMessage "Chocolatey install failed: $($_.Exception.Message)"
        }
    }
    else {
        Write-WarningMessage "Neither winget nor Chocolatey is available for automated installation."
    }

    return $false
}

function Set-LibClangEnvironment {
    param(
        [Parameter(Mandatory = $true)][string]$Directory,
        [switch]$ConfigureProcess
    )

    Write-Info "Setting LIBCLANG_PATH user environment variable to '$Directory'"
    [Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $Directory, "User")

    if ($ConfigureProcess) {
        $env:LIBCLANG_PATH = $Directory
        if ($env:PATH -notlike "*$Directory*") {
            $env:PATH = "$Directory;$env:PATH"
        }
        Write-Info "Updated current session PATH for immediate use"
    }

    Write-Success "LIBCLANG_PATH configured. Restart PowerShell or open a new terminal to pick up user-level changes."
}

try {
    if ($ForceInstall -and $SkipInstall) {
        throw "Cannot use -ForceInstall together with -SkipInstall."
    }

    $libclangDir = Find-LibClang

    if ($ForceInstall -or (-not $libclangDir)) {
        if ($SkipInstall) {
            throw "libclang was not found and installation is disabled (SkipInstall)."
        }
        $installed = Install-LLVM
        if (-not $installed) {
            throw "Automatic LLVM installation failed. Please install LLVM manually and rerun this script."
        }
        $libclangDir = Find-LibClang
    }

    if (-not $libclangDir) {
        throw "libclang.dll could not be located even after installation. Please install LLVM manually and rerun."
    }

    Write-Success "Found libclang at '$libclangDir'"
    Set-LibClangEnvironment -Directory $libclangDir -ConfigureProcess:$ConfigureCurrentProcess

    Write-Info "You can now run 'cargo build' / 'cargo test' successfully."
}
catch {
    Write-ErrorMessage $_
    exit 1
}
