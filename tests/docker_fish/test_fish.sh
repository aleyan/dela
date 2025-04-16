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

log "4. Testing task disambiguation..."

# Get output from dela list
set output (dela list)

# Check if the ambiguous task marker is present
if not string match -q "*test.*‖*" "$output"
    error "Ambiguous task marker (‖) not found for 'test' task"
    error "Got output: $output"
    exit 1
end

# Check if the disambiguation section exists
if not string match -q "*Duplicate task names (‖)*" "$output"
    error "Disambiguation section not found in dela list output"
    error "Got output: $output"
    exit 1
end

# Extract disambiguated task names
set make_test ""
set npm_test ""
set uv_test ""

# Extract make variant
if string match -q "*'test-*' for make version*" "$output"
    set make_test (string match -r "'(test-[^']+)' for make version" "$output" | string replace -r ".*'(test-[^']+)'.*" '$1')
end

# Extract npm variant
if string match -q "*'test-*' for npm version*" "$output"
    set npm_test (string match -r "'(test-[^']+)' for npm version" "$output" | string replace -r ".*'(test-[^']+)'.*" '$1')
end

# Extract uv variant
if string match -q "*'test-*' for uv version*" "$output"
    set uv_test (string match -r "'(test-[^']+)' for uv version" "$output" | string replace -r ".*'(test-[^']+)'.*" '$1')
end

log "Detected disambiguated test tasks:"
log "- Make: $make_test"
log "- NPM: $npm_test"
log "- UV: $uv_test"

# Verify at least some disambiguated names were found
if test -z "$make_test"; and test -z "$npm_test"; and test -z "$uv_test"
    error "No disambiguated task names found in dela list output"
    error "Got output: $output"
    exit 1
end

# Allow disambiguated tasks
set -x DELA_NON_INTERACTIVE 1

if test -n "$make_test"
    log "Testing Make disambiguated task ($make_test)..."
    dela allow-command "$make_test" --allow 2 >/dev/null 2>&1; or error "Failed to allow $make_test"
    
    # Create a temporary script for make test
    echo '#!/usr/bin/fish
dr '$make_test > ~/run_make_test.fish
    chmod +x ~/run_make_test.fish
    set output (~/run_make_test.fish 2>&1)
    rm ~/run_make_test.fish

    if not string match -q "*Make test task executed successfully*" "$output"
        error "Make test task failed. Got: $output"
        exit 1
    end
end

if test -n "$npm_test"
    log "Testing NPM disambiguated task ($npm_test)..."
    dela allow-command "$npm_test" --allow 2 >/dev/null 2>&1; or error "Failed to allow $npm_test"
    
    # Create a temporary script for npm test
    echo '#!/usr/bin/fish
dr '$npm_test > ~/run_npm_test.fish
    chmod +x ~/run_npm_test.fish
    set output (~/run_npm_test.fish 2>&1)
    rm ~/run_npm_test.fish

    if not string match -q "*NPM test task executed successfully*" "$output"
        error "NPM test task failed. Got: $output"
        exit 1
    end
end

if test -n "$uv_test"
    log "Testing UV disambiguated task ($uv_test)..."
    dela allow-command "$uv_test" --allow 2 >/dev/null 2>&1; or error "Failed to allow $uv_test"
    
    # Create a temporary script for uv test
    echo '#!/usr/bin/fish
dr '$uv_test > ~/run_uv_test.fish
    chmod +x ~/run_uv_test.fish
    set output (~/run_uv_test.fish 2>&1)
    rm ~/run_uv_test.fish

    if not string match -q "*Test task executed successfully*" "$output"
        error "UV test task failed. Got: $output"
        exit 1
    end
end

log "5. Testing allowlist functionality..."

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
set -x DELA_NON_INTERACTIVE 1
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

# Test Poetry tasks with non-interactive mode
log "Testing Poetry tasks with non-interactive mode..."
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

# Test arguments are passed to tasks
log "Testing argument passing to tasks..."
# First allow the command - using --allow 2 to automatically approve it
set -x DELA_NON_INTERACTIVE 1
dela allow-command print-args --allow 2 >/dev/null 2>&1
set -e DELA_NON_INTERACTIVE
if test $status -ne 0
    error "Failed to allow print-args"
    exit 1
end

# Test argument passing via environment variable
log "Testing argument passing via environment variable..."
set -x ARGS "--arg1 --arg2 value"
set -l output (dr print-args 2>&1)
set -e ARGS

# Print the output for debugging
log "Command output: $output"

if not string match -q "*Arguments passed to print-args: --arg1 --arg2 value*" -- "$output"
    echo "Full output: $output"
    error "Arguments not passed correctly through dr command"
    error "Expected: Arguments passed to print-args: --arg1 --arg2 value"
    error "Got: $output"
    exit 1
end

log "=== All tests passed successfully! ==="
exit 0 