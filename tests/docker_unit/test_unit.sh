#!/bin/bash
set -e

# Get the directory containing this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

# Build the Docker image
docker build -t dela-unit-tests -f "$SCRIPT_DIR/Dockerfile" "$PROJECT_ROOT"

# Run the tests in a container
docker run --rm dela-unit-tests 