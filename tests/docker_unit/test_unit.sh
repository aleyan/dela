#!/bin/bash
set -e

# Run all tests using the pre-compiled dependencies
echo "Running unit tests..."
cargo test --all-features --verbose
echo "Unit tests completed successfully!" 