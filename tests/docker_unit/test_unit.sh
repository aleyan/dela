#!/bin/bash
set -e

echo "Running unit tests using cached dependencies..."
# Use the already built dependencies from the builder image
RUSTFLAGS="-C debuginfo=2" cargo test --all-features --quiet 