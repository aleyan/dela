#!/bin/bash

# Exit on any error
set -e
set -x  # Print each command for debugging

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "Building zsh test image..."
docker build \
    --platform linux/amd64 \
    -t dela-test-zsh \
    -f "${SCRIPT_DIR}/Dockerfile.zsh" \
    "${PROJECT_ROOT}"

echo "Running zsh tests..."
docker run --rm \
    --platform linux/amd64 \
    -v "${SCRIPT_DIR}/test_zsh.sh:/home/testuser/test_script.sh:ro" \
    dela-test-zsh

echo "All Docker tests completed successfully!" 