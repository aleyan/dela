#!/bin/bash
set -e

# Run all tests using pre-compiled dependencies
cargo test --all-features --verbose 