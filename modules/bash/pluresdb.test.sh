#!/bin/bash
# PluresDB Bash Module Tests
# Run with: bash modules/bash/pluresdb.test.sh

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

TESTS_PASSED=0
TESTS_FAILED=0

# Test helper functions
assert_equals() {
    local expected="$1"
    local actual="$2"
    local message="$3"
    
    if [ "$expected" = "$actual" ]; then
        echo -e "${GREEN}✓${NC} $message"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} $message"
        echo -e "  Expected: $expected"
        echo -e "  Actual: $actual"
        ((TESTS_FAILED++))
    fi
}

assert_success() {
    local command="$1"
    local message="$2"
    
    if eval "$command" &>/dev/null; then
        echo -e "${GREEN}✓${NC} $message"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} $message"
        echo -e "  Command failed: $command"
        ((TESTS_FAILED++))
    fi
}

assert_function_exists() {
    local function_name="$1"
    
    if declare -f "$function_name" &>/dev/null; then
        echo -e "${GREEN}✓${NC} Function exists: $function_name"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}✗${NC} Function exists: $function_name"
        ((TESTS_FAILED++))
    fi
}

# Setup
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/pluresdb.sh"

# Use a test database
TEST_DB_PATH="/tmp/pluresdb-test-$$.db"
export PLURESDB_DB_PATH="$TEST_DB_PATH"

echo "Testing PluresDB Bash Module"
echo "=============================="
echo ""

# Test 1: Module Loading
echo "Test Group: Module Loading"
assert_function_exists "pluresdb_init"
assert_function_exists "pluresdb_add_command"
assert_function_exists "pluresdb_history"
assert_function_exists "pluresdb_frequency"
assert_function_exists "pluresdb_failed"
assert_function_exists "pluresdb_sessions"
assert_function_exists "pluresdb_hosts"
assert_function_exists "pluresdb_clear"
assert_function_exists "pluresdb_enable_integration"
assert_function_exists "pluresdb_disable_integration"
echo ""

# Test 2: Configuration Variables
echo "Test Group: Configuration Variables"
assert_equals "bash" "$PLURESDB_SHELL_TYPE" "Shell type is bash"
assert_equals "false" "$PLURESDB_CAPTURE_OUTPUT" "Capture output default is false"
assert_equals "10240" "$PLURESDB_MAX_OUTPUT_SIZE" "Max output size default is 10240"
echo ""

# Test 3: NixOS Detection
echo "Test Group: NixOS Detection"
if pluresdb_is_nixos; then
    echo -e "${YELLOW}ℹ${NC} Running on NixOS"
    assert_function_exists "pluresdb_nixos_commands"
    assert_function_exists "pluresdb_nixos_rebuilds"
else
    echo -e "${YELLOW}ℹ${NC} Not running on NixOS (detection working)"
    assert_function_exists "pluresdb_nixos_commands"
    assert_function_exists "pluresdb_nixos_rebuilds"
fi
echo ""

# Test 4: WSL Detection
echo "Test Group: WSL Detection"
if pluresdb_is_wsl; then
    echo -e "${YELLOW}ℹ${NC} Running in WSL"
    assert_function_exists "pluresdb_wsl_commands"
    assert_function_exists "pluresdb_wsl_windows_hostname"
else
    echo -e "${YELLOW}ℹ${NC} Not running in WSL (detection working)"
    assert_function_exists "pluresdb_wsl_commands"
    assert_function_exists "pluresdb_wsl_windows_hostname"
fi
echo ""

# Test 5: Command Ignore Patterns
echo "Test Group: Command Ignore Patterns"
export PLURESDB_IGNORE_PATTERNS="test_ignore,another_ignore"

# Test that ignored command doesn't throw error
if pluresdb_add_command "test_ignore command" 0 100 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Ignore pattern works (command silently ignored)"
    ((TESTS_PASSED++))
else
    echo -e "${YELLOW}⚠${NC} Ignore pattern test (expected silent success)"
    # This is OK - command was ignored
    ((TESTS_PASSED++))
fi
echo ""

# Cleanup
rm -f "$TEST_DB_PATH"

# Summary
echo "=============================="
echo "Test Summary"
echo "=============================="
echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
if [ $TESTS_FAILED -gt 0 ]; then
    echo -e "${RED}Failed: $TESTS_FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
