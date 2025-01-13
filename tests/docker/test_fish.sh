#!/usr/bin/fish

# Exit on any error
status --is-interactive; and exit 1

# Default to non-verbose output
set -q VERBOSE; or set VERBOSE 0

# Set up logging functions
function log
    test "$VERBOSE" = "1"; and echo $argv
end

function error
    echo "Error: $argv" >&2
end

log "=== Testing dela shell integration for fish ==="

log "1. Verifying test environment..."

# Verify dela binary is installed and accessible
command -v dela >/dev/null; or begin
    error "dela not found in PATH"
    exit 1
end

# Verify config.fish exists
test -f ~/.config/fish/config.fish; or begin
    error "config.fish not found"
    exit 1
end

# Verify Makefile exists
test -f ~/Makefile; or begin
    error "Makefile not found"
    exit 1
end

# Verify initial command_not_found_handler works
set output (nonexistent_command 2>&1); or true
if not echo $output | grep -q "fish: Unknown command: nonexistent_command"
    error "Initial command_not_found_handler not working."
    error "Expected: 'fish: Unknown command: nonexistent_command'"
    error "Got: '$output'"
    exit 1
end

log "2. Testing dela initialization..."

# Initialize dela and verify directory creation
dela init
test -d ~/.dela; or begin
    error "~/.dela directory not created"
    exit 1
end

# Verify shell integration was added
grep -q "eval (dela configure-shell)" ~/.config/fish/config.fish; or begin
    error "Shell integration not found in config.fish"
    exit 1
end

log "3. Testing dela shell integration..."

# Source updated config.fish and check for errors
source ~/.config/fish/config.fish
if test $status -ne 0
    error "Failed to source config.fish"
    exit 1
end

# Verify shell integration was loaded
set output (dela configure-shell 2>&1)
if test $status -ne 0
    error "dela configure-shell failed with output: $output"
    exit 1
end

# Test dela list command
log "Testing dela list command..."
dela list | grep "test-task"; or begin
    error "test-task not found in dela list"
    exit 1
end

log "4. Testing task execution..."

# Test dela run command
log "Testing dela run command..."
set output (dela run test-task)
echo $output | grep -q "Test task executed successfully"; or begin
    error "dela run test-task failed. Got: $output"
    exit 1
end

# Test direct task invocation
log "Testing direct task invocation..."
set output (test-task)
echo $output | grep -q "Test task executed successfully"; or begin
    error "Direct task invocation failed. Got: $output"
    exit 1
end

# Test another task
log "Testing another task..."
set output (another-task)
echo $output | grep -q "Another task executed successfully"; or begin
    error "another-task failed. Got: $output"
    exit 1
end

# Verify command_not_found_handler was properly replaced
log "Testing final command_not_found_handler..."
set output (nonexistent_command 2>&1); or true
if echo $output | grep -q "fish: Unknown command: nonexistent_command"
    error "Command not found handler wasn't properly replaced."
    error "Got: '$output'"
    exit 1
end

log "=== All tests passed successfully! ===" 