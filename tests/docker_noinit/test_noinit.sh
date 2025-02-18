#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo "Starting non-initialized shell integration tests..."

# Test 1: Basic dela list without shell integration
echo -e "\nTest 1: Testing dela list without shell integration"
# Capture output to a file to avoid broken pipe
output_file=$(mktemp)
dela list > "$output_file" 2>&1
if grep -q "npm-test" "$output_file" && grep -q "npm-build" "$output_file"; then
    echo -e "${GREEN}✓ dela list shows tasks from package.json${NC}"
else
    echo -e "${RED}✗ dela list failed to show tasks${NC}"
    echo "Full output:"
    cat "$output_file"
    rm "$output_file"
    exit 1
fi
rm "$output_file"

# Test 2: Test get-command functionality
echo -e "\nTest 2: Testing get-command"
output_file=$(mktemp)
dela get-command npm-test > "$output_file" 2>&1
if grep -q "npm run npm-test" "$output_file"; then
    echo -e "${GREEN}✓ get-command returns correct npm command${NC}"
else
    echo -e "${RED}✗ get-command failed${NC}"
    echo "Full output:"
    cat "$output_file"
    rm "$output_file"
    exit 1
fi
rm "$output_file"

echo -e "\n${GREEN}✓ All noinit tests passed${NC}"
