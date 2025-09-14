.PHONY: build tests tests_integration test_unit test_noinit test_zsh test_bash test_fish test_pwsh run install builder publish

# Default to non-verbose output
VERBOSE ?= 0

# Load environment variables from .env file if it exists
-include .env

build:
	@echo "Building dela..."
	cargo build

tests:
	@echo "Running unit tests."
	cargo test

# Build the base builder image
_builder:
	@echo "Building base builder image..."
	docker build -t dela-builder -f tests/Dockerfile.builder .

# Individual shell test targets
test_unit: _builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh unit;

test_noinit: _builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh noinit;

test_zsh: _builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh zsh;

test_bash: _builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh bash;

test_fish: _builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh fish;

test_pwsh: _builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh pwsh;

# Run all shell tests
tests_integration: _builder test_unit test_noinit test_zsh test_bash test_fish test_pwsh

install:
	@echo "Installing dela locally..."
	cargo install --path .

# Run dela with arguments: make run ARGS="list" or make run ARGS="help"
run:
	@echo "Build and run dela binary with args"
	cargo run $(ARGS)

inspect_mcp:
	@echo "Inspecting MCP server..."
	npx @modelcontextprotocol/inspector cargo run --quiet -- mcp

# Publish to crates.io
publish: tests tests_integration
	@echo "Publishing dela to crates.io"
	@if [ -z "$(CARGO_REGISTRY_TOKEN)" ]; then \
		echo "Error: CARGO_REGISTRY_TOKEN is not set. Please add it to your .env file."; \
		exit 1; \
	fi
	@cargo publish

format:
	cargo fmt --all
