# PluresDB Command Line Integration

This guide covers the PluresDB PowerShell and Bash modules for advanced command history tracking, database utilities, and system integration.

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [PowerShell Module](#powershell-module)
- [Bash Module](#bash-module)
- [NixOS Integration](#nixos-integration)
- [WSL Integration](#wsl-integration)
- [Command History Features](#command-history-features)
- [Cross-Device Sync](#cross-device-sync)
- [Examples](#examples)

## Overview

PluresDB provides shell modules for PowerShell and Bash that enable:

- **Automatic command history capture** - Track every command you run
- **Advanced querying** - Search, filter, and analyze your command history
- **Deduplication** - View unique commands and frequency statistics
- **Cross-platform sync** - Access history from multiple machines via P2P sync
- **Success/failure tracking** - Identify problematic commands
- **Performance metrics** - Track command execution duration
- **System integration** - NixOS and WSL-specific features

## Installation

### Prerequisites

1. Install PluresDB:
   ```bash
   # Windows
   winget install pluresdb.pluresdb
   
   # npm
   npm install -g pluresdb
   
   # Deno
   deno install -Agf pluresdb
   ```

2. Ensure PluresDB CLI is in your PATH:
   ```bash
   pluresdb --version
   ```

### PowerShell Module

1. Copy the module to your PowerShell modules directory:
   ```powershell
   # System-wide (requires admin)
   $modulePath = "$env:ProgramFiles\WindowsPowerShell\Modules\PluresDB"
   
   # User-specific
   $modulePath = "$HOME\Documents\PowerShell\Modules\PluresDB"
   
   # Create directory and copy files
   New-Item -ItemType Directory -Path $modulePath -Force
   Copy-Item "modules\powershell\*" $modulePath -Recurse
   ```

2. Import the module:
   ```powershell
   Import-Module PluresDB
   ```

3. Initialize the database:
   ```powershell
   Initialize-PluresDBHistory
   ```

4. Enable automatic history capture:
   ```powershell
   Enable-PluresDBHistoryIntegration
   ```

### Bash Module

1. Source the module in your shell profile:
   ```bash
   # Add to ~/.bashrc or ~/.bash_profile
   source /path/to/pluresdb/modules/bash/pluresdb.sh
   ```

2. Initialize the database:
   ```bash
   pluresdb_init
   ```

3. Enable automatic history capture:
   ```bash
   pluresdb_enable_integration
   ```

## PowerShell Module

### Core Functions

#### Initialize-PluresDBHistory
Initialize the command history database.

```powershell
Initialize-PluresDBHistory [-DBPath <path>]

# Examples
Initialize-PluresDBHistory
Initialize-PluresDBHistory -DBPath "D:\MyHistory\history.db"
```

#### Get-PluresDBHistory
Query command history with various filters.

```powershell
Get-PluresDBHistory [options]

# Examples
Get-PluresDBHistory -Last 10
Get-PluresDBHistory -CommandLike "git*" -SuccessOnly
Get-PluresDBHistory -FailedOnly -Last 5
Get-PluresDBHistory -Unique -Last 20
Get-PluresDBHistory -ShellType "powershell" -Hostname "DESKTOP-PC"
```

**Parameters:**
- `-CommandLike <pattern>` - Filter commands matching wildcard pattern
- `-Hostname <name>` - Filter by hostname (default: current host)
- `-ShellType <type>` - Filter by shell type (powershell, bash, etc.)
- `-SuccessOnly` - Show only successful commands
- `-FailedOnly` - Show only failed commands
- `-Last <n>` - Number of recent commands (default: 100)
- `-Unique` - Show deduplicated commands

#### Get-PluresDBCommandFrequency
Show most frequently used commands.

```powershell
Get-PluresDBCommandFrequency [-Top <n>]

# Examples
Get-PluresDBCommandFrequency -Top 20
```

#### Get-PluresDBFailedCommands
Show commands that failed execution.

```powershell
Get-PluresDBFailedCommands [-Last <n>]

# Examples
Get-PluresDBFailedCommands -Last 10
```

#### Get-PluresDBSessionHistory
Show command statistics grouped by shell session.

```powershell
Get-PluresDBSessionHistory

# Example output shows:
# - Session start/end times
# - Command count per session
# - Success/failure rates
```

#### Get-PluresDBHostSummary
Show command statistics per host.

```powershell
Get-PluresDBHostSummary

# Example output shows:
# - Total commands per host
# - Shell types used
# - Success/failure rates
# - Active days
```

#### Clear-PluresDBHistory
Remove old command history.

```powershell
Clear-PluresDBHistory [-OlderThanDays <days>]

# Examples
Clear-PluresDBHistory -OlderThanDays 90
Clear-PluresDBHistory -OlderThanDays 365 -Confirm:$false
```

#### Set-PluresDBConfig
Configure module settings.

```powershell
Set-PluresDBConfig [options]

# Examples
Set-PluresDBConfig -CaptureOutput $true -MaxOutputSize 20480
Set-PluresDBConfig -IgnorePatterns @("ls", "cd", "pwd")
```

**Parameters:**
- `-CaptureOutput <bool>` - Enable/disable command output capture
- `-MaxOutputSize <bytes>` - Maximum output size to capture
- `-IgnorePatterns <array>` - Commands to exclude from history

### History Integration

#### Enable-PluresDBHistoryIntegration
Enable automatic command history capture by modifying your PowerShell profile.

```powershell
Enable-PluresDBHistoryIntegration

# After running, restart PowerShell or reload profile:
. $PROFILE
```

#### Disable-PluresDBHistoryIntegration
Disable automatic command history capture.

```powershell
Disable-PluresDBHistoryIntegration
```

## Bash Module

### Core Functions

#### pluresdb_init
Initialize the command history database.

```bash
pluresdb_init [db_path]

# Examples
pluresdb_init
pluresdb_init ~/.local/share/pluresdb/history.db
```

#### pluresdb_history
Query command history with filters.

```bash
pluresdb_history [options]

# Examples
pluresdb_history --last 10
pluresdb_history --command-like "git" --success-only
pluresdb_history --failed-only --last 5
pluresdb_history --unique --last 20
pluresdb_history --shell-type "bash" --hostname "server01"
```

**Options:**
- `--command-like <pattern>` - Filter by command pattern
- `--hostname <name>` - Filter by hostname
- `--shell-type <type>` - Filter by shell type
- `--success-only` - Show only successful commands
- `--failed-only` - Show only failed commands
- `--last <n>` - Number of results (default: 100)
- `--unique` - Show deduplicated commands

#### pluresdb_frequency
Show command frequency statistics.

```bash
pluresdb_frequency [top]

# Examples
pluresdb_frequency 20
```

#### pluresdb_failed
Show failed commands.

```bash
pluresdb_failed [last]

# Examples
pluresdb_failed 10
```

#### pluresdb_sessions
Show session history.

```bash
pluresdb_sessions
```

#### pluresdb_hosts
Show host summary.

```bash
pluresdb_hosts
```

#### pluresdb_clear
Clear old command history.

```bash
pluresdb_clear [days]

# Examples
pluresdb_clear 90  # Clear history older than 90 days
```

### History Integration

#### pluresdb_enable_integration
Enable automatic command history capture.

```bash
pluresdb_enable_integration

# After running, reload profile:
source ~/.bashrc
```

#### pluresdb_disable_integration
Disable automatic command history capture.

```bash
pluresdb_disable_integration
```

## NixOS Integration

The Bash module includes NixOS-specific features for tracking system configuration changes.

### pluresdb_nixos_commands
Show NixOS-related commands and their success rates.

```bash
pluresdb_nixos_commands

# Shows statistics for commands like:
# - nix-env
# - nix-build
# - nixos-rebuild
# - nix-shell
```

### pluresdb_nixos_rebuilds
Track NixOS system rebuild history.

```bash
pluresdb_nixos_rebuilds

# Shows:
# - Rebuild timestamps
# - Commands used (switch, boot, test)
# - Success/failure status
# - Duration
# - Working directory (often indicates which configuration was used)
```

### Use Cases

1. **Track configuration changes:**
   ```bash
   pluresdb_nixos_rebuilds | jq '.[] | select(.exit_code == 0)'
   ```

2. **Identify failed rebuilds:**
   ```bash
   pluresdb_history --command-like "nixos-rebuild" --failed-only
   ```

3. **Find frequently used NixOS commands:**
   ```bash
   pluresdb_nixos_commands
   ```

## WSL Integration

The Bash module detects WSL environments and provides WSL-specific tracking.

### pluresdb_wsl_commands
Show WSL-specific commands.

```bash
pluresdb_wsl_commands

# Tracks commands like:
# - wsl.exe
# - cmd.exe
# - powershell.exe
# - Commands accessing Windows filesystem (/mnt/c/)
```

### Features

1. **Windows hostname tracking** - Automatically captures Windows hostname in WSL
2. **Cross-filesystem commands** - Track commands accessing Windows drives
3. **PowerShell integration** - Track PowerShell commands run from WSL

### PowerShell in WSL

When using PowerShell in WSL, you can track history across both environments:

```bash
# In WSL Bash
pluresdb_history --shell-type "powershell"

# Show commands run on Windows from WSL
pluresdb_wsl_commands
```

## Command History Features

### Deduplication

The modules automatically deduplicate commands using database views:

```sql
-- Unique commands view
SELECT * FROM command_history_unique;

-- Shows:
-- - Unique command text
-- - Last execution time
-- - Total execution count
-- - Success/failure counts
-- - Average duration
```

### Grouping and Sorting

Commands can be grouped and sorted by:
- Hostname
- Shell type
- Timestamp
- Success/failure status
- Execution frequency
- Duration

### Query Examples

#### Find commands that often fail
```sql
SELECT command, failure_count, success_count, 
       ROUND(100.0 * failure_count / (success_count + failure_count), 2) as failure_rate
FROM command_frequency
WHERE failure_count > 0
ORDER BY failure_rate DESC;
```

#### Find slowest commands
```sql
SELECT command, avg_duration_ms, total_executions
FROM command_frequency
ORDER BY avg_duration_ms DESC
LIMIT 10;
```

#### Commands used across multiple hosts
```sql
SELECT command, unique_hosts, unique_shells
FROM command_frequency
WHERE unique_hosts > 1
ORDER BY unique_hosts DESC;
```

## Cross-Device Sync

PluresDB supports P2P sync of command history across devices.

### Setup Sync

1. **Enable sync in configuration:**
   ```sql
   UPDATE command_history_config 
   SET value = 'true' 
   WHERE key = 'sync_enabled';
   ```

2. **Configure device ID:**
   ```bash
   # The device_id is automatically set when commands are recorded
   # You can verify it:
   SELECT DISTINCT device_id FROM command_history;
   ```

3. **Start PluresDB with sync enabled:**
   ```bash
   pluresdb serve --p2p --port 34567
   ```

### Access History from Multiple Machines

Once synced, you can query history from all devices:

```powershell
# PowerShell - Show all hosts
Get-PluresDBHostSummary

# Show commands from a specific host
Get-PluresDBHistory -Hostname "LAPTOP-01" -Last 20

# Show commands from all machines running PowerShell
Get-PluresDBHistory -ShellType "powershell" -Last 50
```

```bash
# Bash - Show all hosts
pluresdb_hosts

# Show commands from a specific host
pluresdb_history --hostname "server01" --last 20

# Show commands from all machines running bash
pluresdb_history --shell-type "bash" --last 50
```

## Examples

### PowerShell Examples

```powershell
# Initialize and enable history tracking
Initialize-PluresDBHistory
Enable-PluresDBHistoryIntegration

# View recent commands
Get-PluresDBHistory -Last 10

# Find git commands that succeeded
Get-PluresDBHistory -CommandLike "git*" -SuccessOnly -Last 20

# Show most frequently used commands
Get-PluresDBCommandFrequency -Top 15

# Troubleshoot recent failures
Get-PluresDBFailedCommands -Last 5

# View unique commands only
Get-PluresDBHistory -Unique -Last 30

# Configure to capture output
Set-PluresDBConfig -CaptureOutput $true -MaxOutputSize 10240

# Ignore common commands
Set-PluresDBConfig -IgnorePatterns @("ls", "cd", "dir", "pwd")

# Clean up old history
Clear-PluresDBHistory -OlderThanDays 180

# View statistics
Get-PluresDBHostSummary
Get-PluresDBSessionHistory
```

### Bash Examples

```bash
# Initialize and enable history tracking
pluresdb_init
pluresdb_enable_integration

# View recent commands
pluresdb_history --last 10

# Find git commands that succeeded
pluresdb_history --command-like "git" --success-only --last 20

# Show most frequently used commands
pluresdb_frequency 15

# Troubleshoot recent failures
pluresdb_failed 5

# View unique commands only
pluresdb_history --unique --last 30

# Ignore common commands
export PLURESDB_IGNORE_PATTERNS="ls,cd,pwd"

# Clean up old history
pluresdb_clear 180

# View statistics
pluresdb_hosts
pluresdb_sessions

# NixOS specific
pluresdb_nixos_commands
pluresdb_nixos_rebuilds

# WSL specific
pluresdb_wsl_commands
```

### Advanced Query Examples

#### PowerShell - Find longest running commands
```powershell
$query = @"
SELECT command, avg_duration_ms, total_executions
FROM command_frequency
ORDER BY avg_duration_ms DESC
LIMIT 10
"@

pluresdb query --db $env:USERPROFILE\.pluresdb\history.db $query | ConvertFrom-Json
```

#### Bash - Commands with high failure rate
```bash
query="
SELECT command, failure_count, success_count, 
       ROUND(100.0 * failure_count / (success_count + failure_count), 2) as failure_rate
FROM command_frequency
WHERE failure_count > 0
ORDER BY failure_rate DESC
LIMIT 10
"

pluresdb query --db ~/.pluresdb/history.db "$query"
```

#### Cross-shell history analysis
```bash
# Show which shells are most active
query="
SELECT shell_type, 
       COUNT(*) as total_commands,
       COUNT(DISTINCT hostname) as unique_hosts,
       SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as successes
FROM command_history
GROUP BY shell_type
ORDER BY total_commands DESC
"

pluresdb query --db ~/.pluresdb/history.db "$query"
```

## Configuration

### Environment Variables

**PowerShell:**
- Module configuration is managed via `Set-PluresDBConfig`

**Bash:**
- `PLURESDB_DB_PATH` - Database file path (default: `~/.pluresdb/history.db`)
- `PLURESDB_API_ENDPOINT` - API endpoint (default: `http://localhost:34567`)
- `PLURESDB_CAPTURE_OUTPUT` - Capture command output (default: `false`)
- `PLURESDB_MAX_OUTPUT_SIZE` - Max output size in bytes (default: `10240`)
- `PLURESDB_IGNORE_PATTERNS` - Comma-separated ignore patterns

### Database Configuration

Configuration is stored in the `command_history_config` table:

```sql
-- View configuration
SELECT * FROM command_history_config;

-- Update configuration
UPDATE command_history_config SET value = 'true' WHERE key = 'capture_output';
UPDATE command_history_config SET value = '20480' WHERE key = 'max_output_size';
UPDATE command_history_config SET value = 'ls,cd,pwd' WHERE key = 'ignore_patterns';
```

## Troubleshooting

### Commands not being recorded

1. **Verify integration is enabled:**
   ```powershell
   # PowerShell
   Get-Content $PROFILE | Select-String "PluresDB"
   ```
   ```bash
   # Bash
   grep "PluresDB" ~/.bashrc
   ```

2. **Check database exists:**
   ```powershell
   Test-Path "$env:USERPROFILE\.pluresdb\history.db"
   ```
   ```bash
   ls -la ~/.pluresdb/history.db
   ```

3. **Verify pluresdb CLI is available:**
   ```bash
   which pluresdb
   pluresdb --version
   ```

### Performance issues

1. **Reduce output capture:**
   ```powershell
   Set-PluresDBConfig -CaptureOutput $false
   ```
   ```bash
   export PLURESDB_CAPTURE_OUTPUT=false
   ```

2. **Clean old history:**
   ```powershell
   Clear-PluresDBHistory -OlderThanDays 30
   ```
   ```bash
   pluresdb_clear 30
   ```

3. **Add ignore patterns for frequent commands:**
   ```powershell
   Set-PluresDBConfig -IgnorePatterns @("ls", "cd", "pwd", "dir")
   ```
   ```bash
   export PLURESDB_IGNORE_PATTERNS="ls,cd,pwd"
   ```

## Security Considerations

### Sensitive Commands

**⚠️ IMPORTANT**: Command history tracking captures command-line arguments, which may include sensitive information such as:
- Passwords passed as command-line arguments
- API keys and tokens
- Personal information
- File paths containing sensitive data

### Recommendations

1. **Use ignore patterns** for commands that may contain sensitive data:
   ```powershell
   # PowerShell
   Set-PluresDBConfig -IgnorePatterns @("*password*", "*token*", "*secret*", "*api*key*")
   ```
   ```bash
   # Bash
   export PLURESDB_IGNORE_PATTERNS="*password*,*token*,*secret*,*api*key*"
   ```

2. **Never enable output capture** for sensitive operations:
   ```powershell
   Set-PluresDBConfig -CaptureOutput $false  # Keep this disabled
   ```

3. **Use environment variables** instead of command-line arguments for sensitive data

4. **Regularly clean history** to remove potentially sensitive commands:
   ```powershell
   Clear-PluresDBHistory -OlderThanDays 30
   ```

5. **Secure the database file** with appropriate file permissions:
   ```bash
   # Linux/macOS
   chmod 600 ~/.pluresdb/history.db
   ```
   ```powershell
   # PowerShell - Restrict to current user only
   $acl = Get-Acl "$env:USERPROFILE\.pluresdb\history.db"
   $acl.SetAccessRuleProtection($true, $false)
   $rule = New-Object System.Security.AccessControl.FileSystemAccessRule($env:USERNAME, "FullControl", "Allow")
   $acl.SetAccessRule($rule)
   Set-Acl "$env:USERPROFILE\.pluresdb\history.db" $acl
   ```

6. **Disable P2P sync** for sensitive environments unless using encrypted connections

7. **Be aware** that command history is stored in plain text in the database

## See Also

- [PluresDB Documentation](../README.md)
- [CLI Reference](./CLI_TOOL_COMPLETION.md)
- [P2P Sync](./P2P_API_IMPLEMENTATION.md)
