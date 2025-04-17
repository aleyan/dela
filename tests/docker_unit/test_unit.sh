#!/bin/bash
set -e

# Run all tests without excessive output
cargo test --all-features --quiet 