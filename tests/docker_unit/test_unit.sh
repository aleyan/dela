#!/bin/bash
set -e

echo "Running unit tests using cached dependencies..."

# Run tests directly with cargo test
# This will automatically use the cached dependencies in target directory
cargo test --all-features