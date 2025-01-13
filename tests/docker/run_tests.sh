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

# Function to run tests for a specific shell
run_shell_tests() {
    local shell=$1
    local image_name="dela-test-${shell}"
    local dockerfile="Dockerfile.${shell}"
    local test_script="test_${shell}.sh"

    # Build the Docker image
    log "Building ${shell} test image..."
    if [ "$VERBOSE" = "1" ]; then
        docker build \
            --platform linux/amd64 \
            -t "${image_name}" \
            -f "${SCRIPT_DIR}/${dockerfile}" \
            "${PROJECT_ROOT}"
    else
        docker build \
            --platform linux/amd64 \
            -t "${image_name}" \
            -f "${SCRIPT_DIR}/${dockerfile}" \
            "${PROJECT_ROOT}" >/dev/null 2>&1
    fi

    # Run the tests
    log "Running ${shell} tests..."
    if [ "$VERBOSE" = "1" ]; then
        docker run --rm \
            --platform linux/amd64 \
            -v "${SCRIPT_DIR}/${test_script}:/home/testuser/test_script.sh:ro" \
            -e VERBOSE=1 \
            "${image_name}"
    else
        docker run --rm \
            --platform linux/amd64 \
            -v "${SCRIPT_DIR}/${test_script}:/home/testuser/test_script.sh:ro" \
            -e VERBOSE=0 \
            "${image_name}" >/dev/null 2>&1
    fi

    log "${shell} tests passed successfully!"
}

# Run tests for each shell
run_shell_tests "zsh"
run_shell_tests "bash"

# Only show success message if all tests pass
echo "All shell integration tests passed successfully!" 