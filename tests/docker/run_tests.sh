#!/bin/bash

# Exit on any error
set -e

# Default to non-verbose output
VERBOSE=${VERBOSE:-0}

# Set up logging functions
log() {
    if [ "$VERBOSE" = "1" ]; then
        echo "$@"
    fi
}

error() {
    echo "Error: $@" >&2
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Only show docker build output in verbose mode
log "Building zsh test image..."
if [ "$VERBOSE" = "1" ]; then
    docker build \
        --platform linux/amd64 \
        -t dela-test-zsh \
        -f "${SCRIPT_DIR}/Dockerfile.zsh" \
        "${PROJECT_ROOT}"
else
    docker build \
        --platform linux/amd64 \
        -t dela-test-zsh \
        -f "${SCRIPT_DIR}/Dockerfile.zsh" \
        "${PROJECT_ROOT}" >/dev/null 2>&1
fi

# Run the tests
log "Running zsh tests..."
if [ "$VERBOSE" = "1" ]; then
    docker run --rm \
        --platform linux/amd64 \
        -v "${SCRIPT_DIR}/test_zsh.sh:/home/testuser/test_script.sh:ro" \
        -e VERBOSE=1 \
        dela-test-zsh
else
    docker run --rm \
        --platform linux/amd64 \
        -v "${SCRIPT_DIR}/test_zsh.sh:/home/testuser/test_script.sh:ro" \
        -e VERBOSE=0 \
        dela-test-zsh >/dev/null 2>&1
fi

# Only show success message if tests pass
echo "All shell integration tests passed successfully!" 