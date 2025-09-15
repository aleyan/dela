#!/bin/bash
set -e

# This script will be mounted and executed by the Docker container
# It's a simple wrapper around run.sh

exec /home/testuser/mcp_tests/run.sh