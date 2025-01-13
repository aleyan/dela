#!/bin/zsh

# Exit on any error
set -e
set -x  # Print each command for debugging

echo "Testing dela shell integration for zsh..."

# Verify dela is installed and accessible
which dela || (echo "dela not found in PATH" && exit 1)

# Initialize dela
echo "Initializing dela..."
dela init

# Verify .zshrc exists and source it
echo "Sourcing .zshrc..."
test -f ~/.zshrc || (echo ".zshrc not found" && exit 1)
source ~/.zshrc

# Create a test Makefile
echo "Creating test Makefile..."
cat > Makefile << 'EOF'
test-task:
	@echo "Test task executed successfully"

another-task:
	@echo "Another task executed successfully"
EOF

# Test dela list command
echo "Testing dela list command..."
dela list | grep "test-task" || (echo "test-task not found in dela list" && exit 1)

# Try running the task directly (should work via command_not_found_handler)
echo "Testing direct task invocation..."
test-task

# Try running via dela run
echo "Testing dela run command..."
dela run test-task

# Try running another task
echo "Testing another task..."
another-task

# Verify .zshrc contains the shell integration
echo "Verifying shell integration in .zshrc..."
if grep -q "eval \"\$(dela configure-shell)\"" ~/.zshrc; then
    echo "Shell integration found in .zshrc"
else
    echo "Shell integration not found in .zshrc"
    exit 1
fi

# Verify ~/.dela directory exists and has expected contents
echo "Verifying ~/.dela directory..."
if [ ! -d ~/.dela ]; then
    echo "~/.dela directory not found"
    exit 1
fi

echo "All tests passed successfully!" 