#!/bin/bash
set -e

# Run all tests using the pre-compiled dependencies
echo "Running unit tests..."
cargo test --all-features --verbose
# Store the exit code
test_result=$?

# Only report success if tests actually passed
if [ $test_result -eq 0 ]; then
  echo "Unit tests completed successfully!"
  exit 0
else
  echo "Unit tests failed with exit code $test_result"
  exit $test_result
fi 