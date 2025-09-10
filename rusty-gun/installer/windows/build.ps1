Param(
  [string]$Version = "0.1.0",
  [string]$Arch = "x64"
)

# Pre-req: compiled binary exists
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSCommandPath | Split-Path -Parent
$bin = Join-Path $root 'rusty-gun.exe'
if (-not (Test-Path $bin)) { throw "Binary not found: $bin. Run 'deno task compile' first." }

# Prepare staging directory
$stage = Join-Path $root "installer\windows\stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage | Out-Null

# Copy files
Copy-Item $bin (Join-Path $stage 'rusty-gun.exe')

# Optionally include web UI dist if present
$webDist = Join-Path $root 'web\dist'
if (Test-Path $webDist) {
  Copy-Item $webDist (Join-Path $stage 'web') -Recurse
}

# Create a simple ZIP as a placeholder deliverable (MSI can be added later)
$zipPath = Join-Path $root ("rusty-gun-$Version-$Arch.zip")
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zipPath
Write-Host "Created package: $zipPath"

# TODO: Replace with WiX/NSIS MSI build; script kept simple for dogfooding






