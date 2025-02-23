#!/usr/bin/fish

# Default to non-verbose output
set -q VERBOSE; or set VERBOSE 0

# Set up logging functions
function log
    test "$VERBOSE" = "1"; and echo $argv
end

function error
    echo "Error: $argv" >&2
    exit 1
end

# Set up error handling
status --is-interactive; and exit 1

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
begin
    set output (fish -c "nonexistent_command" 2>&1)
    or true
end
if not string match -q "*fish: Unknown command: nonexistent_command*" -- "$output"
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
grep -q "eval (dela configure-shell | string collect)" ~/.config/fish/config.fish; or begin
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
dela list | grep -q "test-task"; or begin
    error "test-task not found in dela list"
    exit 1
end
dela list | grep -q "npm-test"; or begin
    error "npm-test not found in dela list"
    exit 1
end
dela list | grep -q "npm-build"; or begin
    error "npm-build not found in dela list"
    exit 1
end
if not dela list | grep -q "poetry-build"
    error "poetry-build not found in dela list"
    exit 1
end

log "Testing task shadowing detection..."

# Create a custom executable in PATH
log "Creating custom executable..."
mkdir -p ~/.local/bin
echo '#!/bin/sh' > ~/.local/bin/custom-exe
echo 'echo "Custom executable in PATH"' >> ~/.local/bin/custom-exe
chmod +x ~/.local/bin/custom-exe

# Test that dela list shows shadowing symbols
log "Testing shadow detection in dela list..."
set output (dela list)

# Check for shell builtin shadowing (cd)
if not string match -q "*cd (make) †*" "$output"
    error "Shell builtin shadowing symbol not found for 'cd' task"
    error "Got output: $output"
    exit 1
end

if not string match -q "*† task 'cd' shadowed by fish shell builtin*" "$output"
    error "Shell builtin shadow info not found for 'cd' task"
    error "Got output: $output"
    exit 1
end

# Check for PATH executable shadowing (custom-exe)
if not string match -q "*custom-exe (make) ‡*" "$output"
    error "PATH executable shadowing symbol not found for 'custom-exe' task"
    error "Got output: $output"
    exit 1
end

log "4. Testing allowlist functionality..."

# Ensure we're in non-interactive mode for allowlist testing
set -x DELA_NON_INTERACTIVE 1

# Test that task is initially blocked
log "Testing task is initially blocked..."
set output (fish -c "test-task" 2>&1); or true
if not string match -q "*requires approval*" -- "$output"
    error "Expected task to be blocked with approval prompt, but got: $output"
    exit 1
end

# Test interactive allow-command functionality
log "Testing interactive allow-command functionality..."
set -e DELA_NON_INTERACTIVE
printf "2\n" | dela allow-command test-task >/dev/null 2>&1; or error "Failed to allow test-task"

# Test allowed task execution
log "Testing allowed task execution..."
source ~/.config/fish/config.fish
eval (dela configure-shell | string collect)

# Create a temporary script to run the command
echo '#!/usr/bin/fish
dr test-task' > ~/run_test.fish
chmod +x ~/run_test.fish
set output (~/run_test.fish 2>&1)
rm ~/run_test.fish

if not string match -q "*Test task executed successfully*" -- "$output"
    error "Task execution failed after allowing. Got: $output"
    exit 1
end

# Test UV tasks with non-interactive mode
log "Testing UV tasks with non-interactive mode..."
dela allow-command uv-test --allow 2 >/dev/null 2>&1; or error "Failed to allow uv-test"
dela allow-command uv-build --allow 2 >/dev/null 2>&1; or error "Failed to allow uv-build"

# Create a temporary script for UV test
echo '#!/usr/bin/fish
dr uv-test' > ~/run_uv_test.fish
chmod +x ~/run_uv_test.fish
set output (~/run_uv_test.fish 2>&1)
rm ~/run_uv_test.fish

if not string match -q "*Test task executed successfully*" -- "$output"
    error "UV test task failed. Got: $output"
    exit 1
end

# Create a temporary script for UV build
echo '#!/usr/bin/fish
dr uv-build' > ~/run_uv_build.fish
chmod +x ~/run_uv_build.fish
set output (~/run_uv_build.fish 2>&1)
rm ~/run_uv_build.fish

if not string match -q "*Build task executed successfully*" -- "$output"
    error "UV build task failed. Got: $output"
    exit 1
end

# Test Poetry tasks
log "Testing Poetry tasks..."
dela allow-command poetry-test --allow 2 >/dev/null 2>&1; or error "Failed to allow poetry-test"
dela allow-command poetry-build --allow 2 >/dev/null 2>&1; or error "Failed to allow poetry-build"

# Create a temporary script for Poetry test
echo '#!/usr/bin/fish
dr poetry-test' > ~/run_poetry_test.fish
chmod +x ~/run_poetry_test.fish
set output (~/run_poetry_test.fish 2>&1)
rm ~/run_poetry_test.fish

if not string match -q "*Test task executed successfully*" -- "$output"
    error "Poetry test task failed. Got: $output"
    exit 1
end

# Create a temporary script for Poetry build
echo '#!/usr/bin/fish
dr poetry-build' > ~/run_poetry_build.fish
chmod +x ~/run_poetry_build.fish
set output (~/run_poetry_build.fish 2>&1)
rm ~/run_poetry_build.fish

if not string match -q "*Build task executed successfully*" -- "$output"
    error "Poetry build task failed. Got: $output"
    exit 1
end

# Verify command_not_found_handler was properly replaced
log "Testing final command_not_found_handler..."
set output (fish -c "nonexistent_command" 2>&1); or true
if not string match -q "*fish: Unknown command: nonexistent_command*" -- "$output"
    error "Command not found handler wasn't properly replaced."
    error "Got: '$output'"
    exit 1
end

log "=== All tests passed successfully! ==="
exit 0 