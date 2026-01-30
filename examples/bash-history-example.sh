#!/bin/bash
# Example: Setting up Bash History Integration with PluresDB
# This script demonstrates how to configure and use PluresDB command history

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

echo -e "${CYAN}=== PluresDB Bash History Integration Example ===${NC}\n"

# 1. Source the module
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODULE_PATH="$SCRIPT_DIR/../modules/bash/pluresdb.sh"

if [ ! -f "$MODULE_PATH" ]; then
    echo -e "${RED}Error: Module not found at $MODULE_PATH${NC}"
    exit 1
fi

source "$MODULE_PATH"

# 2. Initialize the database
echo -e "${CYAN}=== Initializing PluresDB History ===${NC}"
pluresdb_init

# 3. Configure settings
echo -e "\n${CYAN}=== Configuring PluresDB ===${NC}"
export PLURESDB_CAPTURE_OUTPUT=false  # Don't capture output for performance
export PLURESDB_IGNORE_PATTERNS="ls,cd,pwd"  # Ignore common commands

# 4. Manually add some example commands (simulating history)
echo -e "\n${CYAN}=== Adding Example Commands ===${NC}"
pluresdb_add_command "ls -la" 0 50
pluresdb_add_command "git status" 0 45
pluresdb_add_command "git commit -m 'test'" 1 120 "" "nothing to commit"
pluresdb_add_command "npm install" 0 5400
pluresdb_add_command "cargo build" 0 3200
pluresdb_add_command "docker ps" 0 150

if pluresdb_is_nixos; then
    pluresdb_add_command "nixos-rebuild switch" 0 45000
    pluresdb_add_command "nix-env -iA nixpkgs.git" 0 8500
fi

if pluresdb_is_wsl; then
    pluresdb_add_command "powershell.exe Get-Process" 0 300
    pluresdb_add_command "cmd.exe /c dir" 0 100
fi

echo -e "${GREEN}✓ Example commands added${NC}"

# 5. Query the history
echo -e "\n${CYAN}=== Recent Command History ===${NC}"
pluresdb_history --last 10

# 6. Show command frequency
echo -e "\n${CYAN}=== Command Frequency ===${NC}"
pluresdb_frequency 10

# 7. Show failed commands
echo -e "\n${CYAN}=== Failed Commands ===${NC}"
pluresdb_failed 5

# 8. Query with filters
echo -e "\n${CYAN}=== Git Commands Only ===${NC}"
pluresdb_history --command-like "git" --last 10

# 9. Show unique commands
echo -e "\n${CYAN}=== Unique Commands ===${NC}"
pluresdb_history --unique --last 10

# 10. Show host summary
echo -e "\n${CYAN}=== Host Summary ===${NC}"
pluresdb_hosts

# 11. NixOS-specific features (if on NixOS)
if pluresdb_is_nixos; then
    echo -e "\n${CYAN}=== NixOS Commands ===${NC}"
    pluresdb_nixos_commands
    
    echo -e "\n${CYAN}=== NixOS Rebuild History ===${NC}"
    pluresdb_nixos_rebuilds
fi

# 12. WSL-specific features (if in WSL)
if pluresdb_is_wsl; then
    echo -e "\n${CYAN}=== WSL Commands ===${NC}"
    pluresdb_wsl_commands
    
    echo -e "\n${CYAN}=== WSL Windows Hostname ===${NC}"
    echo "Windows hostname: $(pluresdb_wsl_windows_hostname)"
fi

# 13. Advanced queries
echo -e "\n${CYAN}=== Advanced Query: Slowest Commands ===${NC}"
query="
SELECT command, avg_duration_ms, total_executions
FROM command_frequency
WHERE total_executions > 0
ORDER BY avg_duration_ms DESC
LIMIT 5
"

echo -e "${GRAY}Query:${NC}"
echo -e "${GRAY}$query${NC}"
echo -e "\n${GRAY}Results:${NC}"
pluresdb query --db "$PLURESDB_DB_PATH" "$query"

# 14. Show integration instructions
echo -e "\n${CYAN}=== Enabling Automatic History Integration ===${NC}"
echo -e "${YELLOW}To enable automatic history capture, run:${NC}"
echo -e "  ${GREEN}pluresdb_enable_integration${NC}"
echo -e "  ${GREEN}source ~/.bashrc  # Reload your profile${NC}"
echo ""
echo -e "${YELLOW}To disable automatic history capture, run:${NC}"
echo -e "  ${GREEN}pluresdb_disable_integration${NC}"

# 15. Show help
echo -e "\n${CYAN}=== Available Commands ===${NC}"
pluresdb_help

echo -e "\n${GREEN}=== Example Complete ===${NC}"
echo -e "${GRAY}Database location: $PLURESDB_DB_PATH${NC}"
echo ""
echo -e "${CYAN}Next steps:${NC}"
echo -e "${NC}1. Run 'pluresdb_enable_integration' to auto-capture history${NC}"
echo -e "${NC}2. Reload your profile: source ~/.bashrc${NC}"
echo -e "${NC}3. Use 'pluresdb_history' to query your command history${NC}"
echo -e "${NC}4. Use 'pluresdb_frequency' to see frequently used commands${NC}"
echo -e "${NC}5. Use 'pluresdb_help' to see all available commands${NC}"
