#!/bin/bash

# Exit on any error
set -e

# Default to non-verbose output
VERBOSE=${VERBOSE:-0}
# Default platform to arm64 (can be overridden by CI)
DOCKER_PLATFORM=${DOCKER_PLATFORM:-linux/arm64}
# Default to empty builder image (will be set by CI if needed)
BUILDER_IMAGE=${BUILDER_IMAGE:-}

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
    echo "  DOCKER_PLATFORM: Platform for Docker builds (default: linux/arm64)"
    echo "  BUILDER_IMAGE: Full path to builder image (default: uses local dela-builder)"
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

    # If we have a builder image, update the Dockerfile
    if [ -n "$BUILDER_IMAGE" ]; then
        log "Using builder image: $BUILDER_IMAGE"
        sed -i.bak "s|FROM dela-builder|FROM ${BUILDER_IMAGE}|" "${SCRIPT_DIR}/docker_${shell}/${dockerfile}"
    fi

    # Build the Docker image
    log "Building ${shell} test image..."
    if [ "$VERBOSE" = "1" ]; then
        docker build \
            --platform "$DOCKER_PLATFORM" \
            -t "${image_name}" \
            -f "${SCRIPT_DIR}/docker_${shell}/${dockerfile}" \
            "${PROJECT_ROOT}"
    else
        docker build \
            --platform "$DOCKER_PLATFORM" \
            -t "${image_name}" \
            -f "${SCRIPT_DIR}/docker_${shell}/${dockerfile}" \
            "${PROJECT_ROOT}" >/dev/null 2>&1
    fi

    # Restore the original Dockerfile if we modified it
    if [ -n "$BUILDER_IMAGE" ]; then
        mv "${SCRIPT_DIR}/docker_${shell}/${dockerfile}.bak" "${SCRIPT_DIR}/docker_${shell}/${dockerfile}"
    fi

    # Run the tests
    log "Running ${shell} tests..."
    if [ "$VERBOSE" = "1" ]; then
        docker run --rm \
            --platform "$DOCKER_PLATFORM" \
            -v "${SCRIPT_DIR}/docker_${shell}/${test_script}:/home/testuser/${container_script}:ro" \
            -e VERBOSE=1 \
            "${image_name}"
    else
        # Run tests in non-verbose mode and capture output
        output=$(docker run --rm \
            --platform "$DOCKER_PLATFORM" \
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