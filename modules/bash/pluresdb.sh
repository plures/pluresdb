#!/bin/bash
# PluresDB Bash Module
# Command history integration and database utilities for Bash

# Module version
PLURESDB_MODULE_VERSION="1.0.0"

# Module configuration
PLURESDB_DB_PATH="${PLURESDB_DB_PATH:-$HOME/.pluresdb/history.db}"
PLURESDB_API_ENDPOINT="${PLURESDB_API_ENDPOINT:-http://localhost:34567}"
PLURESDB_HOSTNAME="${HOSTNAME:-$(hostname)}"
PLURESDB_SHELL_TYPE="bash"
PLURESDB_SHELL_VERSION="${BASH_VERSION}"
PLURESDB_USERNAME="${USER}"
PLURESDB_SESSION_ID="$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || echo "$RANDOM-$RANDOM-$RANDOM")"
PLURESDB_CAPTURE_OUTPUT="${PLURESDB_CAPTURE_OUTPUT:-false}"
PLURESDB_MAX_OUTPUT_SIZE="${PLURESDB_MAX_OUTPUT_SIZE:-10240}"
PLURESDB_IGNORE_PATTERNS="${PLURESDB_IGNORE_PATTERNS:-}"

#region Core Database Functions

# Initialize PluresDB command history database
pluresdb_init() {
    local db_path="${1:-$PLURESDB_DB_PATH}"
    
    # Ensure directory exists
    local db_dir="$(dirname "$db_path")"
    mkdir -p "$db_dir"
    
    # Read and execute schema
    local script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    local schema_path="$script_dir/../shared/schema.sql"
    
    if [ -f "$schema_path" ]; then
        pluresdb query --db "$db_path" < "$schema_path"
        echo "✅ PluresDB command history initialized at: $db_path"
    else
        echo "⚠️  Schema file not found at: $schema_path" >&2
        return 1
    fi
}

# Add a command to PluresDB history
pluresdb_add_command() {
    local command="$1"
    local exit_code="${2:-0}"
    local duration="${3:-0}"
    local output="${4:-}"
    local error_output="${5:-}"
    local working_dir="${6:-$PWD}"
    
    # Check ignore patterns - escape regex special characters
    if [ -n "$PLURESDB_IGNORE_PATTERNS" ]; then
        IFS=',' read -ra patterns <<< "$PLURESDB_IGNORE_PATTERNS"
        for pattern in "${patterns[@]}"; do
            # Escape regex special characters in pattern for safety
            pattern_escaped=$(printf '%s\n' "$pattern" | sed 's/[.[\*^$()+?{|]/\\&/g')
            if [[ "$command" =~ ^$pattern_escaped ]]; then
                return 0
            fi
        done
    fi
    
    # Get timestamp in milliseconds
    local timestamp=$(($(date +%s) * 1000))
    
    # Truncate output if needed
    if [ "$PLURESDB_CAPTURE_OUTPUT" = "true" ] && [ ${#output} -gt $PLURESDB_MAX_OUTPUT_SIZE ]; then
        output="${output:0:$PLURESDB_MAX_OUTPUT_SIZE}... (truncated)"
    fi
    
    # Escape single quotes in strings for SQL
    command="${command//\'/\'\'}"
    output="${output//\'/\'\'}"
    error_output="${error_output//\'/\'\'}"
    working_dir="${working_dir//\'/\'\'}"
    
    # Build and execute insert query
    local query="INSERT INTO command_history 
        (command, hostname, shell_type, shell_version, username, working_directory, 
         timestamp, duration_ms, exit_code, output, error_output, session_id)
    VALUES 
        ('$command', '$PLURESDB_HOSTNAME', '$PLURESDB_SHELL_TYPE', '$PLURESDB_SHELL_VERSION', 
         '$PLURESDB_USERNAME', '$working_dir', $timestamp, $duration, $exit_code, 
         '${output}', '${error_output}', '$PLURESDB_SESSION_ID')"
    
    echo "$query" | pluresdb query --db "$PLURESDB_DB_PATH" - 2>/dev/null || true
}

# Query command history from PluresDB
pluresdb_history() {
    local command_like=""
    local hostname="$PLURESDB_HOSTNAME"
    local shell_type=""
    local success_only=false
    local failed_only=false
    local last=100
    local unique=false
    
    # Parse options
    while [[ $# -gt 0 ]]; do
        case $1 in
            --command-like)
                command_like="$2"
                shift 2
                ;;
            --hostname)
                hostname="$2"
                shift 2
                ;;
            --shell-type)
                shell_type="$2"
                shift 2
                ;;
            --success-only)
                success_only=true
                shift
                ;;
            --failed-only)
                failed_only=true
                shift
                ;;
            --last)
                # Validate that last is a positive integer
                if [[ "$2" =~ ^[0-9]+$ ]] && [ "$2" -gt 0 ]; then
                    last="$2"
                else
                    echo "Error: --last must be a positive integer" >&2
                    return 1
                fi
                shift 2
                ;;
            --unique)
                unique=true
                shift
                ;;
            *)
                echo "Unknown option: $1" >&2
                return 1
                ;;
        esac
    done
    
    # Build WHERE clause - use parameter substitution for safety
    local conditions=()
    
    if [ -n "$command_like" ]; then
        # Escape single quotes by doubling them for SQL
        command_like="${command_like//\'/\'\'}"
        conditions+=("command LIKE '%$command_like%'")
    fi
    
    if [ -n "$hostname" ]; then
        # Escape single quotes by doubling them for SQL
        hostname="${hostname//\'/\'\'}"
        conditions+=("hostname = '$hostname'")
    fi
    
    if [ -n "$shell_type" ]; then
        # Escape single quotes by doubling them for SQL
        shell_type="${shell_type//\'/\'\'}"
        conditions+=("shell_type = '$shell_type'")
    fi
    
    if [ "$success_only" = true ]; then
        conditions+=("is_success = 1")
    fi
    
    if [ "$failed_only" = true ]; then
        conditions+=("is_success = 0")
    fi
    
    local where_clause=""
    if [ ${#conditions[@]} -gt 0 ]; then
        where_clause="WHERE $(IFS=' AND '; echo "${conditions[*]}")"
    fi
    
    local table="command_history"
    if [ "$unique" = true ]; then
        table="command_history_unique"
    fi
    
    local query="SELECT * FROM $table $where_clause ORDER BY timestamp DESC LIMIT $last"
    
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

# Get command frequency statistics
pluresdb_frequency() {
    local top="${1:-10}"
    
    local query="SELECT * FROM command_frequency LIMIT $top"
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

# Get failed commands for troubleshooting
pluresdb_failed() {
    local last="${1:-10}"
    
    local query="SELECT * FROM failed_commands LIMIT $last"
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

# Get session history statistics
pluresdb_sessions() {
    local query="SELECT * FROM session_history"
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

# Get host summary statistics
pluresdb_hosts() {
    local query="SELECT * FROM host_summary"
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

# Clear command history
pluresdb_clear() {
    local older_than_days="${1:-90}"
    
    local cutoff_timestamp=$(( ($(date +%s) - (older_than_days * 86400)) * 1000 ))
    
    read -p "Delete command history older than $older_than_days days? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        local query="DELETE FROM command_history WHERE timestamp < $cutoff_timestamp"
        pluresdb query --db "$PLURESDB_DB_PATH" "$query"
        echo "✅ Cleared command history older than $older_than_days days"
    fi
}

#endregion

#region History Integration

# Enable automatic command history capture
pluresdb_enable_integration() {
    local bash_profile="$HOME/.bashrc"
    
    # Detect profile file
    if [ -f "$HOME/.bash_profile" ]; then
        bash_profile="$HOME/.bash_profile"
    fi
    
    local integration_code='
# PluresDB History Integration
export PLURESDB_HISTORY_ENABLED=1

# Source PluresDB module
PLURESDB_MODULE_PATH="'"$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"'/pluresdb.sh"
if [ -f "$PLURESDB_MODULE_PATH" ]; then
    source "$PLURESDB_MODULE_PATH"
fi

# Track command execution time
_pluresdb_command_start_time=0
_pluresdb_last_command=""

# Function to capture command before execution
_pluresdb_preexec() {
    # Get timestamp with fallback for systems without nanosecond support
    if date +%s%3N &>/dev/null; then
        _pluresdb_command_start_time=$(date +%s%3N)
    else
        # Fallback for macOS/BSD
        _pluresdb_command_start_time=$(($(date +%s) * 1000))
    fi
    _pluresdb_last_command="$1"
}

# Function to capture command after execution
_pluresdb_precmd() {
    local exit_code=$?
    local end_time
    
    # Get timestamp with fallback for systems without nanosecond support
    if date +%s%3N &>/dev/null; then
        end_time=$(date +%s%3N)
    else
        # Fallback for macOS/BSD - use milliseconds from epoch
        end_time=$(($(date +%s) * 1000))
    fi
    
    local duration=$(( end_time - _pluresdb_command_start_time ))
    
    if [ -n "$_pluresdb_last_command" ] && [ "$_pluresdb_last_command" != "_pluresdb_precmd" ]; then
        pluresdb_add_command "$_pluresdb_last_command" "$exit_code" "$duration"
    fi
    
    _pluresdb_last_command=""
}

# Hook into bash using DEBUG trap
trap '"'"'_pluresdb_preexec "$BASH_COMMAND"'"'"' DEBUG

# Hook into prompt command
if [[ ! "$PROMPT_COMMAND" =~ "_pluresdb_precmd" ]]; then
    # Safely append to PROMPT_COMMAND
    if [ -n "$PROMPT_COMMAND" ]; then
        PROMPT_COMMAND="_pluresdb_precmd;$PROMPT_COMMAND"
    else
        PROMPT_COMMAND="_pluresdb_precmd"
    fi
fi
'
    
    if ! grep -q "PluresDB History Integration" "$bash_profile" 2>/dev/null; then
        echo "$integration_code" >> "$bash_profile"
        echo "✅ PluresDB history integration enabled in: $bash_profile"
        echo "   Run: source $bash_profile"
    else
        echo "⚠️  PluresDB history integration already enabled"
    fi
}

# Disable automatic command history capture
pluresdb_disable_integration() {
    local bash_profile="$HOME/.bashrc"
    
    if [ -f "$HOME/.bash_profile" ]; then
        bash_profile="$HOME/.bash_profile"
    fi
    
    if [ -f "$bash_profile" ]; then
        # Remove integration code and clean up backup file
        sed -i.bak '/# PluresDB History Integration/,/^fi$/d' "$bash_profile"
        
        # Remove backup file if sed was successful
        if [ -f "${bash_profile}.bak" ]; then
            rm -f "${bash_profile}.bak"
        fi
        
        echo "✅ PluresDB history integration disabled"
        echo "   Run: source $bash_profile"
    fi
}

#endregion

#region NixOS Integration

# Check if running on NixOS
pluresdb_is_nixos() {
    [ -f /etc/NIXOS ] || grep -q "nixos" /etc/os-release 2>/dev/null
}

# Add NixOS-specific command tracking
pluresdb_nixos_commands() {
    if ! pluresdb_is_nixos; then
        echo "⚠️  Not running on NixOS" >&2
        return 1
    fi
    
    # Query for NixOS-specific commands
    local query="SELECT command, COUNT(*) as count, 
                        SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as successes,
                        SUM(CASE WHEN is_success THEN 0 ELSE 1 END) as failures
                 FROM command_history 
                 WHERE command LIKE 'nix%' 
                    OR command LIKE 'nixos-%' 
                    OR command LIKE 'nix-%'
                 GROUP BY command 
                 ORDER BY count DESC 
                 LIMIT 20"
    
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

# Track NixOS rebuild history
pluresdb_nixos_rebuilds() {
    if ! pluresdb_is_nixos; then
        echo "⚠️  Not running on NixOS" >&2
        return 1
    fi
    
    local query="SELECT timestamp, command, exit_code, duration_ms, working_directory
                 FROM command_history 
                 WHERE command LIKE 'nixos-rebuild%' 
                    OR command LIKE 'sudo nixos-rebuild%'
                 ORDER BY timestamp DESC 
                 LIMIT 50"
    
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

#endregion

#region WSL Integration

# Check if running in WSL
pluresdb_is_wsl() {
    grep -qi microsoft /proc/version 2>/dev/null
}

# Get Windows hostname for WSL
pluresdb_wsl_windows_hostname() {
    if pluresdb_is_wsl; then
        powershell.exe -Command '$env:COMPUTERNAME' 2>/dev/null | tr -d '\r\n'
    fi
}

# Track WSL-specific commands
pluresdb_wsl_commands() {
    if ! pluresdb_is_wsl; then
        echo "⚠️  Not running in WSL" >&2
        return 1
    fi
    
    # Query for WSL-specific commands
    local query="SELECT command, COUNT(*) as count,
                        SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as successes
                 FROM command_history 
                 WHERE command LIKE 'wsl%' 
                    OR command LIKE 'cmd.exe%' 
                    OR command LIKE 'powershell.exe%'
                    OR command LIKE '%/mnt/c/%'
                 GROUP BY command 
                 ORDER BY count DESC 
                 LIMIT 20"
    
    pluresdb query --db "$PLURESDB_DB_PATH" "$query"
}

#endregion

# Print help
pluresdb_help() {
    cat << 'EOF'
PluresDB Bash Module - Command History Integration

USAGE:
    pluresdb_init [db_path]                  Initialize database
    pluresdb_add_command <cmd> [exit_code]   Add command to history
    pluresdb_history [options]               Query command history
    pluresdb_frequency [top]                 Show command frequency
    pluresdb_failed [last]                   Show failed commands
    pluresdb_sessions                        Show session history
    pluresdb_hosts                           Show host summary
    pluresdb_clear [days]                    Clear old history
    pluresdb_enable_integration              Enable auto history capture
    pluresdb_disable_integration             Disable auto history capture

NIXOS:
    pluresdb_nixos_commands                  Show NixOS commands
    pluresdb_nixos_rebuilds                  Show nixos-rebuild history

WSL:
    pluresdb_wsl_commands                    Show WSL-specific commands

QUERY OPTIONS:
    --command-like <pattern>                 Filter by command pattern
    --hostname <host>                        Filter by hostname
    --shell-type <type>                      Filter by shell type
    --success-only                           Show only successful commands
    --failed-only                            Show only failed commands
    --last <n>                               Number of results (default: 100)
    --unique                                 Show deduplicated commands

EXAMPLES:
    pluresdb_history --last 10
    pluresdb_history --command-like "git" --success-only
    pluresdb_failed 5
    pluresdb_frequency 20
    pluresdb_nixos_rebuilds

EOF
}

# Export functions if in interactive shell
if [[ $- == *i* ]]; then
    export -f pluresdb_init
    export -f pluresdb_add_command
    export -f pluresdb_history
    export -f pluresdb_frequency
    export -f pluresdb_failed
    export -f pluresdb_sessions
    export -f pluresdb_hosts
    export -f pluresdb_clear
    export -f pluresdb_enable_integration
    export -f pluresdb_disable_integration
    export -f pluresdb_is_nixos
    export -f pluresdb_nixos_commands
    export -f pluresdb_nixos_rebuilds
    export -f pluresdb_is_wsl
    export -f pluresdb_wsl_windows_hostname
    export -f pluresdb_wsl_commands
    export -f pluresdb_help
fi
