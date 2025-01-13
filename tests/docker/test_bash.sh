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

# Enable command printing only in verbose mode
if [ "$VERBOSE" = "1" ]; then
    set -x
fi

log "=== Testing dela shell integration for bash ==="

log "1. Verifying test environment..."

# Verify dela binary is installed and accessible
which dela || (error "dela not found in PATH" && exit 1)

# Verify .bashrc exists
test -f ~/.bashrc || (error ".bashrc not found" && exit 1)

# Verify Makefile exists
test -f ~/Makefile || (error "Makefile not found" && exit 1)

# Verify initial command_not_found_handle works
source ~/.bashrc
output=$(nonexistent_command 2>&1) || true
if ! echo "$output" | grep -q "bash: command not found: nonexistent_command"; then
    error "Initial command_not_found_handle not working."
    error "Expected: 'bash: command not found: nonexistent_command'"
    error "Got: '$output'"
    exit 1
fi

log "2. Testing dela initialization..."

# Initialize dela and verify directory creation
dela init
test -d ~/.dela || (error "~/.dela directory not created" && exit 1)

# Verify shell integration was added
grep -q "eval \"\$(dela configure-shell)\"" ~/.bashrc || {
    error "Shell integration not found in .bashrc"
    exit 1
}

log "3. Testing dela shell integration..."

# Source updated bashrc and check for errors
source ~/.bashrc
if [ $? -ne 0 ]; then
    error "Failed to source .bashrc"
    exit 1
fi

# Verify shell integration was loaded
output=$(dela configure-shell 2>&1)
if [ $? -ne 0 ]; then
    error "dela configure-shell failed with output: $output"
    exit 1
fi

# Test dela list command
log "Testing dela list command..."
dela list | grep "test-task" || (error "test-task not found in dela list" && exit 1)

log "4. Testing task execution..."

# Test dela run command
log "Testing dela run command..."
output=$(dela run test-task)
echo "$output" | grep -q "Test task executed successfully" || {
    error "dela run test-task failed. Got: $output"
    exit 1
}

# Test direct task invocation
log "Testing direct task invocation..."
output=$(test-task)
echo "$output" | grep -q "Test task executed successfully" || {
    error "Direct task invocation failed. Got: $output"
    exit 1
}

# Test another task
log "Testing another task..."
output=$(another-task)
echo "$output" | grep -q "Another task executed successfully" || {
    error "another-task failed. Got: $output"
    exit 1
}

# Verify command_not_found_handle was properly replaced
log "Testing final command_not_found_handle..."
# Temporarily disable trace mode for this test
set +x
output=$(nonexistent_command 2>&1) || true
set -x
if ! echo "$output" | grep -q "bash: command not found: nonexistent_command"; then
    error "Command not found handler wasn't properly replaced."
    error "Got: '$output'"
    exit 1
fi

log "=== All tests passed successfully! ===" 