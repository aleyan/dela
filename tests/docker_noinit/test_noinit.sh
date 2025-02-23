#!/bin/zsh
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

# Prevent broken pipe errors
exec 2>&1

echo "Starting non-initialized shell integration tests..."

# Test 1: Basic dela list without shell integration
echo "\nTest 1: Testing dela list without shell integration"
if dela list | grep -q "npm-test" && dela list | grep -q "npm-build"; then
    echo "${GREEN}✓ dela list shows npm tasks${NC}"
else
    echo "${RED}✗ dela list failed to show npm tasks${NC}"
    exit 1
fi

# Test 2: Test get-command functionality for npm
echo "\nTest 2: Testing get-command for npm"
output=$(dela get-command npm-test 2>&1)
if echo "$output" | grep -q "npm run npm-test"; then
    echo "${GREEN}✓ get-command returns correct npm command${NC}"
else
    echo "${RED}✗ get-command failed${NC}"
    echo "Full output: $output"
    exit 1
fi

# Test 3: Test allow-command interactive functionality for npm
echo "\nTest 3: Testing allow-command interactive functionality for npm"
echo "Initial allowlist contents:"
cat /home/testuser/.dela/allowlist.toml

# Test interactive allow-command with option 2 (Allow this task)
echo "\nTesting interactive allow-command with 'Allow this task' option:"
echo "2" | dela allow-command npm-build >/dev/null 2>&1

echo "\nAllowlist contents after allow-command:"
cat /home/testuser/.dela/allowlist.toml

# Verify the allowlist was updated with the specific task
if grep -q "npm-build" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ npm-build task was added to allowlist via interactive mode${NC}"
else
    echo "${RED}✗ npm-build task was not added to allowlist via interactive mode${NC}"
    exit 1
fi

# Test 4: Basic dela list for Taskfile tasks
echo "\nTest 4: Testing dela list for Taskfile tasks"
if dela list | grep -q "task-test" && dela list | grep -q "task-build" && dela list | grep -q "task-deps"; then
    echo "${GREEN}✓ dela list shows Taskfile tasks${NC}"
else
    echo "${RED}✗ dela list failed to show Taskfile tasks${NC}"
    exit 1
fi

# Test 5: Test get-command functionality for Taskfile
echo "\nTest 5: Testing get-command for Taskfile"
output=$(dela get-command task-test 2>&1)
if echo "$output" | grep -q "task task-test"; then
    echo "${GREEN}✓ get-command returns correct task command${NC}"
else
    echo "${RED}✗ get-command failed for Taskfile task${NC}"
    echo "Full output: $output"
    exit 1
fi

# Test 6: Test allow-command interactive functionality for Taskfile
echo "\nTest 6: Testing allow-command interactive functionality for Taskfile"
echo "Initial allowlist contents:"
cat /home/testuser/.dela/allowlist.toml

# Test interactive allow-command with option 2 (Allow this task)
echo "\nTesting interactive allow-command with 'Allow this task' option:"
echo "2" | dela allow-command task-build >/dev/null 2>&1 || {
    echo "${RED}✗ allow-command failed for Taskfile task${NC}"
    exit 1
}

echo "\nAllowlist contents after allow-command:"
cat /home/testuser/.dela/allowlist.toml

# Verify the allowlist was updated with the specific task
if grep -q "task-build" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ task-build task was added to allowlist via interactive mode${NC}"
else
    echo "${RED}✗ task-build task was not added to allowlist via interactive mode${NC}"
    exit 1
fi

# Verify the task was added with Task scope (not File or Directory)
if grep -q "scope = \"Task\"" /home/testuser/.dela/allowlist.toml; then
    echo "${GREEN}✓ Task scope was correctly set via interactive mode${NC}"
else
    echo "${RED}✗ Task scope was not correctly set via interactive mode${NC}"
    exit 1
fi

echo "\n${GREEN}All non-init tests completed successfully!${NC}"
