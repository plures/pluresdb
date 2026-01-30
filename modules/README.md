# PluresDB Shell Modules

This directory contains PowerShell and Bash modules for PluresDB command line integration.

## Structure

```
modules/
├── powershell/          # PowerShell module
│   ├── PluresDB.psm1    # Module implementation
│   └── PluresDB.psd1    # Module manifest
├── bash/                # Bash module
│   └── pluresdb.sh      # Module implementation
└── shared/              # Shared resources
    └── schema.sql       # Database schema for command history
```

## Features

- **Command History Tracking**: Automatically capture and store command history
- **Advanced Querying**: Search, filter, and analyze command history
- **Cross-Platform Sync**: P2P synchronization of history across devices
- **Deduplication**: View unique commands and frequency statistics
- **Performance Metrics**: Track command execution duration and success rates
- **System Integration**: NixOS and WSL-specific features
- **Flexible Configuration**: Customize capture behavior and ignore patterns

## Quick Start

### PowerShell

```powershell
# 1. Install the module
$modulePath = "$HOME\Documents\PowerShell\Modules\PluresDB"
New-Item -ItemType Directory -Path $modulePath -Force
Copy-Item "modules\powershell\*" $modulePath -Recurse

# 2. Import and initialize
Import-Module PluresDB
Initialize-PluresDBHistory

# 3. Enable automatic history capture
Enable-PluresDBHistoryIntegration
. $PROFILE

# 4. Query your history
Get-PluresDBHistory -Last 10
Get-PluresDBCommandFrequency -Top 20
```

### Bash

```bash
# 1. Source the module
echo 'source /path/to/pluresdb/modules/bash/pluresdb.sh' >> ~/.bashrc

# 2. Initialize database
source ~/.bashrc
pluresdb_init

# 3. Enable automatic history capture
pluresdb_enable_integration
source ~/.bashrc

# 4. Query your history
pluresdb_history --last 10
pluresdb_frequency 20
```

## Documentation

See [Command Line Integration Guide](../docs/COMMAND_LINE_INTEGRATION.md) for comprehensive documentation.

## Examples

- [`examples/powershell-history-example.ps1`](../examples/powershell-history-example.ps1) - PowerShell usage examples
- [`examples/bash-history-example.sh`](../examples/bash-history-example.sh) - Bash usage examples

## Database Schema

The command history database includes:

- **command_history** - Main table storing all command executions
- **command_history_unique** - View showing deduplicated commands
- **command_frequency** - View showing most frequently used commands
- **failed_commands** - View showing commands that failed
- **session_history** - View grouping commands by shell session
- **host_summary** - View showing statistics per host

## Platform Support

### PowerShell
- Windows PowerShell 5.1+
- PowerShell Core 6.0+
- Windows, Linux, macOS

### Bash
- Bash 4.0+
- Linux, macOS, WSL
- NixOS-specific features when running on NixOS
- WSL-specific features when running in WSL

## Configuration

### PowerShell
```powershell
Set-PluresDBConfig -CaptureOutput $false
Set-PluresDBConfig -MaxOutputSize 10240
Set-PluresDBConfig -IgnorePatterns @("ls", "cd", "pwd")
```

### Bash
```bash
export PLURESDB_DB_PATH="$HOME/.pluresdb/history.db"
export PLURESDB_CAPTURE_OUTPUT=false
export PLURESDB_MAX_OUTPUT_SIZE=10240
export PLURESDB_IGNORE_PATTERNS="ls,cd,pwd"
```

## License

AGPL v3 - See [LICENSE](../LICENSE) for details.
