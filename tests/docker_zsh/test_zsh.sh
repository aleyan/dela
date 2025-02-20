#!/bin/zsh

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

log "=== Testing dela shell integration for zsh ==="

log "1. Verifying test environment..."

# Verify dela binary is installed and accessible
which dela || (error "dela not found in PATH" && exit 1)

# Verify .zshrc exists
test -f ~/.zshrc || (error ".zshrc not found" && exit 1)

# Verify Makefile exists
test -f ~/Makefile || (error "Makefile not found" && exit 1)

# Verify initial command_not_found_handler works
source ~/.zshrc
output=$(nonexistent_command 2>&1) || true
if ! echo "$output" | grep -q "Command not found: nonexistent_command"; then
    error "Initial command_not_found_handler not working."
    error "Expected: 'Command not found: nonexistent_command'"
    error "Got: '$output'"
    exit 1
fi

log "2. Testing dela initialization..."

# Initialize dela and verify directory creation
dela init
test -d ~/.dela || (error "~/.dela directory not created" && exit 1)

# Verify shell integration was added
grep -q "eval \"\$(dela configure-shell)\"" ~/.zshrc || {
    error "Shell integration not found in .zshrc"
    exit 1
}

log "3. Testing dela shell integration..."

# Source updated zshrc and check for errors
source ~/.zshrc
if [ $? -ne 0 ]; then
    error "Failed to source .zshrc"
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
dela list | grep -q "test-task" || (error "test-task not found in dela list" && exit 1)
dela list | grep -q "npm-test" || (error "npm-test not found in dela list" && exit 1)
dela list | grep -q "npm-build" || (error "npm-build not found in dela list" && exit 1)
dela list | grep -q "uv-test" || (error "uv-test not found in dela list" && exit 1)
dela list | grep -q "uv-build" || (error "uv-build not found in dela list" && exit 1)
dela list | grep -q "poetry-test" || (error "poetry-test not found in dela list" && exit 1)
dela list | grep -q "poetry-build" || (error "poetry-build not found in dela list" && exit 1)

log "Testing task shadowing detection..."

# Create a custom executable in PATH
log "Creating custom executable..."
mkdir -p ~/.local/bin
cat > ~/.local/bin/custom-exe << 'EOF'
#!/bin/sh
echo "Custom executable in PATH"
EOF
chmod +x ~/.local/bin/custom-exe

# Test that dela list shows shadowing symbols
log "Testing shadow detection in dela list..."
output=$(dela list)

# Check for shell builtin shadowing (cd)
if ! echo "$output" | grep -q "cd (make) †"; then
    error "Shell builtin shadowing symbol not found for 'cd' task"
    error "Got output: $output"
    exit 1
fi

if ! echo "$output" | grep -q "† task 'cd' shadowed by zsh shell builtin"; then
    error "Shell builtin shadow info not found for 'cd' task"
    error "Got output: $output"
    exit 1
fi

# Check for PATH executable shadowing (custom-exe)
if ! echo "$output" | grep -q "custom-exe (make) ‡"; then
    error "PATH executable shadowing symbol not found for 'custom-exe' task"
    error "Got output: $output"
    exit 1
fi

if ! echo "$output" | grep -q "‡ task 'custom-exe' shadowed by executable at.*custom-exe"; then
    error "PATH executable shadow info not found for 'custom-exe' task"
    error "Got output: $output"
    exit 1
fi

log "4. Testing allowlist functionality..."

# Ensure we're in interactive mode for allowlist testing
unset DELA_NON_INTERACTIVE
unset DELA_AUTO_ALLOW

# Reload shell integration with new environment
source ~/.zshrc

# Test that task is initially not allowed
log "Testing task is initially blocked..."
output=$(test-task 2>&1) || true
if ! echo "$output" | grep -q "requires approval"; then
    error "Expected task to be blocked with approval prompt, but got: $output"
    exit 1
fi

# Allow the task using dela allow-command
log "Testing dela allow-command..."
export DELA_NON_INTERACTIVE=1
export DELA_AUTO_ALLOW=1
echo "2" | dela allow-command test-task || (error "Failed to allow test-task" && exit 1)
unset DELA_NON_INTERACTIVE
unset DELA_AUTO_ALLOW

# Reload shell integration again
source ~/.zshrc

# Verify task is now allowed and runs
log "Testing allowed task execution..."
output=$(test-task 2>&1)
if ! echo "$output" | grep -q "Test task executed successfully"; then
    error "Task execution failed. Got: $output"
    exit 1
fi

# Test UV tasks
log "Testing UV tasks..."
export DELA_NON_INTERACTIVE=1
echo "2" | dela allow-command uv-test || (error "Failed to allow uv-test" && exit 1)
echo "2" | dela allow-command uv-build || (error "Failed to allow uv-build" && exit 1)

output=$(dr uv-test 2>&1)
if ! echo "$output" | grep -q "Test task executed successfully"; then
    error "dr uv-test failed. Got: $output"
    exit 1
fi

output=$(dr uv-build 2>&1)
if ! echo "$output" | grep -q "Build task executed successfully"; then
    error "dr uv-build failed. Got: $output"
    exit 1
fi

# Allow and test Poetry tasks
log "Testing Poetry tasks..."
echo "2" | dela allow-command poetry-test || (error "Failed to allow poetry-test" && exit 1)
echo "2" | dela allow-command poetry-build || (error "Failed to allow poetry-build" && exit 1)

output=$(dr poetry-test 2>&1)
if ! echo "$output" | grep -q "Test task executed successfully"; then
    error "dr poetry-test failed. Got: $output"
    exit 1
fi

output=$(dr poetry-build 2>&1)
if ! echo "$output" | grep -q "Build task executed successfully"; then
    error "dr poetry-build failed. Got: $output"
    exit 1
fi
unset DELA_NON_INTERACTIVE

# Verify command_not_found_handler was properly replaced
log "Testing final command_not_found_handler..."
output=$(nonexistent_command 2>&1) || true
if echo "$output" | grep -q "Command not found: nonexistent_command"; then
    error "Command not found handler wasn't properly replaced."
    error "Got: '$output'"
    exit 1
fi

log "=== All tests passed successfully! ===" 