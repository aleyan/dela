.PHONY: build tests tests_integration test_unit test_noinit test_mcp test_zsh test_bash test_fish test_pwsh run install builder publish

# Default to non-verbose output
VERBOSE ?= 0

# Load environment variables from .env file if it exists
-include .env

build:
	@echo "Building dela..."
	cargo build

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

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

test_mcp: _builder
	VERBOSE=$(VERBOSE) ./tests/run_tests.sh mcp;

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
	@echo "Inspecting MCP server (debug build + stdio via Inspector)..."
	@cargo build --quiet
	# Avoid any stdout noise; Inspector expects clean MCP JSON-RPC on stdout.
	MCPI_NO_COLOR=1 RUST_LOG=warn RUSTFLAGS="-A warnings" npx @modelcontextprotocol/inspector ./target/debug/dela mcp --cwd $(PWD)


# Publish to crates.io
publish: tests tests_integration
	@echo "Publishing dela to crates.io"
	@if [ -z "$(CARGO_REGISTRY_TOKEN)" ]; then \
		echo "Error: CARGO_REGISTRY_TOKEN is not set. Please add it to your .env file."; \
		exit 1; \
	fi
	@cargo publish

# Checks that versions (cargo.toml and CHANGELOG.md) are consistent
# And no tag exists yet, and versions aren't published to crates.io
# And tests pass.
verify_prerelease:
	@echo "Verifying prerelease conditions..."

# Trigger a release by pushing a new version tag to github
# verifies that a human is doing it via cli interaction
tag_and_release: verify_prerelease
	@echo "Tagging and releasing..."

format:
	cargo fmt --all

# Long-running test task for MCP testing
test_long:
	@echo "Starting long-running test task..."
	@sleep 30
	@echo "Long-running test task completed!"

coverage_github: build
	@echo "Running parsing coverage over top github repos..."
	uv run --script scripts/dela_coverage_git_refs.py --dela-bin ./target/debug/dela
