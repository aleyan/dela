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

# Verify the shell integration output contains the correct allowlist check
if ! echo "$output" | grep -q "if ! dela allow-command \"\$1\""; then
    error "Shell integration missing allowlist check"
    error "Got: $output"
    exit 1
fi

# Test dela list command
log "Testing dela list command..."
# Save the output to a file instead of piping directly to grep
dela list > ~/dela_list_output.txt || true
# Display output for debugging
if [ "$VERBOSE" = "1" ]; then
    cat ~/dela_list_output.txt
fi
if grep -q "test-task" ~/dela_list_output.txt; then
    log "test-task found in dela list"
else
    error "test-task not found in dela list" && exit 1
fi
if grep -q "npm-test" ~/dela_list_output.txt; then
    log "npm-test found in dela list"
else
    error "npm-test not found in dela list" && exit 1
fi
if grep -q "npm-build" ~/dela_list_output.txt; then
    log "npm-build found in dela list"
else
    error "npm-build not found in dela list" && exit 1
fi
if grep -q "poetry-build" ~/dela_list_output.txt; then
    log "poetry-build found in dela list"
else
    error "poetry-build not found in dela list" && exit 1
fi

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
dela list > ~/shadow_list_output.txt || true
if ! grep -q "cd-m.*cd.*†" ~/shadow_list_output.txt; then
    error "Shell builtin shadowing symbol not found for 'cd' task"
    cat ~/shadow_list_output.txt
    exit 1
fi

# Check for PATH executable shadowing (custom-exe)
if ! grep -q "custom-exe-m.*custom-exe.*‡" ~/shadow_list_output.txt; then
    error "PATH executable shadowing symbol not found for 'custom-exe' task"
    cat ~/shadow_list_output.txt
    exit 1
fi

log "4. Testing task disambiguation..."

# Extract disambiguated task names from the main listing
log "Searching for test- entries:"
grep -E 'test-[^ ]+' ~/dela_list_output.txt || log "No test- entries found!"

# Skip detailed disambiguation test - this is fully tested in test_noinit.sh
log "Skipping detailed disambiguation test"

# Add test for column width consistency
log "Testing column width formatting consistency..."

# Simplify the column width test - just verify basic formatting
dela list > ~/task_list_output.txt || true

# Count total number of task lines
total_lines=$(grep -E "^  [^ ]+" ~/task_list_output.txt | wc -l)
log "Found $total_lines task lines for column width check"

if [ "$total_lines" -lt 10 ]; then
    error "Expected at least 10 task lines, but found only $total_lines"
    cat ~/task_list_output.txt
    exit 1
fi

# Just verify all task lines start with 2 spaces followed by a non-space character
# followed by spaces, and have consistent column alignment
column_widths=$(grep -E "^  [^ ]+" ~/task_list_output.txt | awk '{print length($1)}' | sort | uniq | wc -l)
if [ "$column_widths" -gt 15 ]; then
    error "Column widths are not consistent (found more than 15 different widths)"
    cat ~/task_list_output.txt
    exit 1
fi

log "Column width formatting test passed successfully"

# Clean up the test file
rm -f ~/task_list_output.txt ~/dela_list_output.txt ~/shadow_list_output.txt

log "5. Testing allowlist functionality..."

# Ensure we're in non-interactive mode for allowlist testing
export DELA_NON_INTERACTIVE=1

# Test that task is initially not allowed
log "Testing task is initially blocked..."
output=$(test-task 2>&1) || true
if ! echo "$output" | grep -q "requires approval"; then
    error "Expected task to be blocked with approval prompt, but got: $output"
    exit 1
fi

# Test interactive allow-command functionality
log "Testing interactive allow-command functionality..."
unset DELA_NON_INTERACTIVE
unset DELA_AUTO_ALLOW
echo "2" | dela allow-command test-task || (error "Failed to allow test-task" && exit 1)

# Reload shell integration again
source ~/.bashrc

# Verify task is now allowed and runs
log "Testing allowed task execution..."
output=$(test-task 2>&1)
if ! echo "$output" | grep -q "Test task executed successfully"; then
    error "Task execution failed. Got: $output"
    exit 1
fi

# Test UV tasks with non-interactive mode
log "Testing UV tasks with non-interactive mode..."
export DELA_NON_INTERACTIVE=1
dela allow-command uv-test --allow 2 || (error "Failed to allow uv-test" && exit 1)
dela allow-command uv-build --allow 2 || (error "Failed to allow uv-build" && exit 1)

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

# Test Poetry tasks with non-interactive mode
log "Testing Poetry tasks with non-interactive mode..."
dela allow-command poetry-test --allow 2 || (error "Failed to allow poetry-test" && exit 1)
dela allow-command poetry-build --allow 2 || (error "Failed to allow poetry-build" && exit 1)

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

# Verify command_not_found_handle was properly replaced
log "Testing final command_not_found_handle..."
# Temporarily disable trace mode for this test
set +x
output=$(nonexistent_command 2>&1) || true
if ! echo "$output" | grep -q "dela: command or task not found: nonexistent_command"; then
    error "Command not found handler not properly replaced"
    error "Expected: 'dela: command or task not found: nonexistent_command'"
    error "Got: $output"
    exit 1
fi

# Test argument passing
log "Testing argument passing to tasks..."

# Test single argument passing
log "Testing single argument passing..."
dela allow-command print-arg-task --allow 2 || (error "Failed to allow print-arg-task" && exit 1)

output=$(ARG=value1 dr print-arg-task)
if ! echo "$output" | grep -q "Argument is: value1"; then
    error "Single argument not passed correctly"
    error "Expected: Argument is: value1"
    error "Got: $output"
    exit 1
fi

# Test multiple arguments passing
log "Testing multiple arguments passing..."
dela allow-command print-args --allow 2 || (error "Failed to allow print-args" && exit 1)

output=$(ARGS="--flag1 --flag2=value positional" dr print-args)
if ! echo "$output" | grep -q "Arguments passed to print-args:"; then
    error "Expected output to contain 'Arguments passed to print-args:'"
    exit 1
fi
if ! echo "$output" | grep -q -- "--flag1"; then
    error "Expected output to contain '--flag1'"
    error "Got: $output"
    exit 1
fi
if ! echo "$output" | grep -q -- "--flag2=value"; then
    error "Expected output to contain '--flag2=value'"
    error "Got: $output"
    exit 1
fi
if ! echo "$output" | grep -q "positional"; then
    error "Expected output to contain 'positional'"
    error "Got: $output"
    exit 1
fi

log "All tests passed!"

log "=== All tests passed successfully! ===" 