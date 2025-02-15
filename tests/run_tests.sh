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

# Print usage information
usage() {
    echo "Usage: $0 [shell]"
    echo "  shell: Optional. One of: zsh, bash, fish, pwsh"
    echo "  If no shell is specified, tests all shells"
    echo ""
    echo "Environment variables:"
    echo "  VERBOSE=1: Enable verbose output"
    exit 1
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
log "SCRIPT_DIR: ${SCRIPT_DIR}"
log "PROJECT_ROOT: ${PROJECT_ROOT}"

# Function to run tests for a specific shell
run_shell_tests() {
    local shell=$1
    local image_name="dela-test-${shell}"
    local dockerfile="Dockerfile"
    local test_script="test_${shell}.sh"
    local container_script="test_script.sh"
    
    # PowerShell uses .ps1 extension
    if [ "$shell" = "pwsh" ]; then
        test_script="test_${shell}.ps1"
        container_script="test_script.ps1"
    fi

    # Build the Docker image
    log "Building ${shell} test image..."
    if [ "$VERBOSE" = "1" ]; then
        docker build \
            --platform linux/arm64 \
            -t "${image_name}" \
            -f "${SCRIPT_DIR}/docker_${shell}/${dockerfile}" \
            "${PROJECT_ROOT}"
    else
        docker build \
            --platform linux/arm64 \
            -t "${image_name}" \
            -f "${SCRIPT_DIR}/docker_${shell}/${dockerfile}" \
            "${PROJECT_ROOT}" >/dev/null 2>&1
    fi

    # Run the tests
    log "Running ${shell} tests..."
    if [ "$VERBOSE" = "1" ]; then
        docker run --rm \
            --platform linux/arm64 \
            -v "${SCRIPT_DIR}/docker_${shell}/${test_script}:/home/testuser/${container_script}:ro" \
            -e VERBOSE=1 \
            "${image_name}"
    else
        # Run tests in non-verbose mode and capture output
        output=$(docker run --rm \
            --platform linux/arm64 \
            -v "${SCRIPT_DIR}/docker_${shell}/${test_script}:/home/testuser/${container_script}:ro" \
            -e VERBOSE=0 \
            "${image_name}" 2>&1) || {
            echo "Test failed. Output:"
            echo "$output"
            return 1
        }
    fi

    echo "${shell} tests passed successfully!"
}

# Check if a specific shell was requested
if [ $# -eq 1 ]; then
    shell=$1
    # Validate shell argument
    case $shell in
        zsh|bash|fish|pwsh)
            log "Testing ${shell} shell integration..."
            run_shell_tests "${shell}"
            ;;
        *)
            error "Invalid shell: ${shell}"
            usage
            ;;
    esac
else
    # Test all shells
    for shell in zsh bash fish pwsh; do
        log "Testing ${shell} shell integration..."
        run_shell_tests "${shell}"
    done
fi 