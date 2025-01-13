#!/bin/zsh

# Exit on any error
set -e
set -x  # Print each command for debugging

echo "=== Testing dela shell integration for zsh ==="

echo "1. Verifying test environment..."

# Verify dela binary is installed and accessible
which dela || (echo "dela not found in PATH" && exit 1)

# Verify .zshrc exists
test -f ~/.zshrc || (echo ".zshrc not found" && exit 1)

# Verify Makefile exists
test -f ~/Makefile || (echo "Makefile not found" && exit 1)

# Verify initial command_not_found_handler works
source ~/.zshrc
output=$(nonexistent_command 2>&1) || true
if ! echo "$output" | grep -q "Command not found: nonexistent_command"; then
    echo "Initial command_not_found_handler not working."
    echo "Expected: 'Command not found: nonexistent_command'"
    echo "Got: '$output'"
    exit 1
fi

echo "2. Testing dela initialization..."

# Initialize dela and verify directory creation
dela init
test -d ~/.dela || (echo "~/.dela directory not created" && exit 1)

# Verify shell integration was added
grep -q "eval \"\$(dela configure-shell)\"" ~/.zshrc || {
    echo "Shell integration not found in .zshrc"
    exit 1
}

echo "3. Testing dela shell integration..."

# Source updated zshrc and check for errors
source ~/.zshrc
if [ $? -ne 0 ]; then
    echo "Failed to source .zshrc"
    exit 1
fi

# Verify shell integration was loaded
output=$(dela configure-shell 2>&1)
if [ $? -ne 0 ]; then
    echo "dela configure-shell failed with output: $output"
    exit 1
fi

# Test dela list command
echo "Testing dela list command..."
dela list | grep "test-task" || (echo "test-task not found in dela list" && exit 1)

echo "4. Testing task execution..."

# Test dela run command
echo "Testing dela run command..."
output=$(dela run test-task)
echo "$output" | grep -q "Test task executed successfully" || {
    echo "dela run test-task failed. Got: $output"
    exit 1
}

# Test direct task invocation
echo "Testing direct task invocation..."
output=$(test-task)
echo "$output" | grep -q "Test task executed successfully" || {
    echo "Direct task invocation failed. Got: $output"
    exit 1
}

# Test another task
echo "Testing another task..."
output=$(another-task)
echo "$output" | grep -q "Another task executed successfully" || {
    echo "another-task failed. Got: $output"
    exit 1
}

# Verify command_not_found_handler was properly replaced
echo "Testing final command_not_found_handler..."
output=$(nonexistent_command 2>&1) || true
if echo "$output" | grep -q "Command not found: nonexistent_command"; then
    echo "Command not found handler wasn't properly replaced."
    echo "Got: '$output'"
    exit 1
fi

echo "=== All tests passed successfully! ===" 