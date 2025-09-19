#!/usr/bin/env bash
set -euo pipefail

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo "Starting MCP integration tests..."

# Ensure the dela binary is available
if [[ ! -x /usr/local/bin/dela ]]; then
  echo "${RED}Error: dela binary not found at /usr/local/bin/dela${NC}" >&2
  exit 1
fi

# Test that dela mcp command works
echo "Testing dela mcp command availability..."
if ! dela mcp --help >/dev/null 2>&1; then
  echo "${RED}Error: dela mcp command not working${NC}" >&2
  exit 1
fi

echo "${GREEN}✓ dela mcp command is available${NC}"

# Run MCP protocol tests
echo "Running MCP protocol integration tests..."
export MCPI_NO_COLOR=1
export RUST_LOG=warn

if python3 test_mcp.py; then
  echo "${GREEN}✓ MCP protocol integration tests passed!${NC}"
else
  echo "${RED}✗ MCP protocol integration tests failed${NC}" >&2
  exit 1
fi

echo "${GREEN}✓ All MCP integration tests passed!${NC}"
