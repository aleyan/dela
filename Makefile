.PHONY: build tests tests_integration test_unit test_noinit test_zsh test_bash test_fish test_pwsh run install builder publish

# Default to non-verbose output
VERBOSE ?= 0

# Load environment variables from .env file if it exists
-include .env

build:
	@echo "Building dela..."
	cargo build

tests:
	@echo "Running tests..."
	cargo test

# Build the base builder image
builder:
	@echo "Building base builder image..."
	docker build -t dela-builder -f tests/Dockerfile.builder .

# Individual shell test targets
test_unit: builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh unit;

test_noinit: builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh noinit;

test_zsh: builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh zsh;

test_bash: builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh bash;

test_fish: builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh fish;

test_pwsh: builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh pwsh;

# Run all shell tests
tests_integration: builder test_unit test_noinit test_zsh test_bash test_fish test_pwsh

install:
	@echo "Installing dela locally..."
	cargo install --path .

run:
	@echo "Running dela..."
	cargo run

# Publish to crates.io
publish: tests tests_integration
	@echo "Publishing dela to crates.io"
	@if [ -z "$(CARGO_REGISTRY_TOKEN)" ]; then \
		echo "Error: CARGO_REGISTRY_TOKEN is not set. Please add it to your .env file."; \
		exit 1; \
	fi
	@cargo publish

# Print git diff without pager
pdiff:
	@git --no-pager diff

format:
	cargo fmt --all
