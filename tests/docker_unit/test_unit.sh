#!/bin/bash
set -e

echo "Running unit tests using cached dependencies..."

# Look for the test binaries by listing them and testing for test binaries specifically
echo "Rust test binaries are at:"
find /home/testuser/target -type f -executable -name 'dela-*' | grep -v '\.d$' | grep -v '\.cargo' | while read binary; do
  # Try to see if this is a test binary by checking if it accepts --list
  if $binary --list > /dev/null 2>&1; then
    echo "$binary (test binary)"
  else
    echo "$binary (not a test binary)"
  fi
done

echo "Running tests from compiled binaries..."
find /home/testuser/target -type f -executable -name 'dela-*' | grep -v '\.d$' | grep -v '\.cargo' | while read binary; do
  # Only run binaries that accept --list parameter (test binaries)
  if $binary --list > /dev/null 2>&1; then
    echo "Running test: $binary"
    $binary
  fi
done