.PHONY: build tests tests_integration test_unit test_noinit test_mcp test_zsh test_bash test_fish test_pwsh run install builder publish release_verify release_publish

SHELL := /bin/bash

# Default to non-verbose output
VERBOSE ?= 0
RELEASE_VERIFY_EXPECT_TAG ?=
RELEASE_VERIFY_SKIP_TAG_EXISTS ?= 0
RELEASE_VERIFY_SKIP_CRATES_CHECK ?= 0
RELEASE_VERIFY_SKIP_TESTS ?= 0

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


# Publish to crates.io. Deprecated. Will be removed once 0.0.7 is released sucessfully using the new release process.
publish: tests tests_integration
	@echo "Publishing dela to crates.io"
	@if [ -z "$(CARGO_REGISTRY_TOKEN)" ]; then \
		echo "Error: CARGO_REGISTRY_TOKEN is not set. Please add it to your .env file."; \
		exit 1; \
	fi
	@cargo publish

# Validates release metadata. By default this also checks that the release tag
# does not already exist, that the version is not already on crates.io, and
# that lint/tests/package dry-run pass. CI can disable those stricter
# prerelease checks via RELEASE_VERIFY_SKIP_* variables.
release_verify:
	@set -euo pipefail; \
	VERSION=$$(grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f2); \
	TAG="v$$VERSION"; \
	echo "Verifying release conditions for $$TAG..."; \
	if ! echo "$$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$$'; then \
		echo "Error: version $$VERSION does not follow semantic versioning (X.Y.Z)."; \
		exit 1; \
	fi; \
	if ! grep -q "^## \[$$VERSION\]" CHANGELOG.md; then \
		echo "Error: version $$VERSION not found in CHANGELOG.md."; \
		exit 1; \
	fi; \
	if grep -q "^## \[$$VERSION\] - Unreleased" CHANGELOG.md; then \
		echo "Error: version $$VERSION is still marked as Unreleased in CHANGELOG.md."; \
		exit 1; \
	fi; \
	if ! grep -q "^## \[$$VERSION\] - [0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}" CHANGELOG.md; then \
		echo "Error: version $$VERSION in CHANGELOG.md does not have a YYYY-MM-DD date."; \
		exit 1; \
	fi; \
	if [ -n "$(RELEASE_VERIFY_EXPECT_TAG)" ] && [ "$(RELEASE_VERIFY_EXPECT_TAG)" != "$$TAG" ]; then \
		echo "Error: expected tag $(RELEASE_VERIFY_EXPECT_TAG) does not match version tag $$TAG."; \
		exit 1; \
	fi; \
	if [ "$(RELEASE_VERIFY_SKIP_TAG_EXISTS)" != "1" ]; then \
		if git rev-parse -q --verify "refs/tags/$$TAG" >/dev/null; then \
			echo "Error: local tag $$TAG already exists."; \
			exit 1; \
		fi; \
		if git ls-remote --exit-code --tags origin "refs/tags/$$TAG" >/dev/null 2>&1; then \
			echo "Error: remote tag $$TAG already exists on origin."; \
			exit 1; \
		fi; \
	fi; \
	if [ "$(RELEASE_VERIFY_SKIP_CRATES_CHECK)" != "1" ]; then \
		if ! command -v jq >/dev/null 2>&1; then \
			echo "Error: jq is required for release_verify."; \
			exit 1; \
		fi; \
		RESPONSE=$$(curl --fail --silent --show-error --location https://crates.io/api/v1/crates/dela); \
		if echo "$$RESPONSE" | jq -e --arg version "$$VERSION" '.versions[] | select(.num == $$version)' >/dev/null; then \
			echo "Error: version $$VERSION already exists on crates.io."; \
			exit 1; \
		fi; \
	fi; \
	if [ "$(RELEASE_VERIFY_SKIP_TESTS)" != "1" ]; then \
		echo "Running lint..."; \
		$(MAKE) lint; \
		echo "Running unit tests..."; \
		$(MAKE) tests; \
		echo "Running integration tests..."; \
		$(MAKE) tests_integration; \
		echo "Running cargo publish dry run..."; \
		cargo publish --dry-run --locked; \
	fi; \
	echo "Release verification passed for $$TAG."

# Trigger a release by pushing a new version tag to github
# verifies that a human is doing it via cli interaction
release_publish:
	@set -euo pipefail; \
	if [ ! -t 0 ]; then \
		echo "Error: release_publish must be run interactively from a terminal."; \
		exit 1; \
	fi; \
	BRANCH=$$(git symbolic-ref --quiet --short HEAD || true); \
	if [ "$$BRANCH" != "main" ]; then \
		echo "Error: release_publish must be run from the main branch. Current branch: $${BRANCH:-detached HEAD}."; \
		exit 1; \
	fi; \
	if [ -n "$$(git status --short)" ]; then \
		echo "Error: working tree is not clean."; \
		git status --short; \
		exit 1; \
	fi; \
	$(MAKE) release_verify; \
	VERSION=$$(grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f2); \
	TAG="v$$VERSION"; \
	COMMIT=$$(git rev-parse --short HEAD); \
	printf "About to create and push tag %s from commit %s.\n" "$$TAG" "$$COMMIT"; \
	printf "Type %s to continue: " "$$TAG"; \
	read -r CONFIRM; \
	if [ "$$CONFIRM" != "$$TAG" ]; then \
		echo "Aborted: confirmation did not match $$TAG."; \
		exit 1; \
	fi; \
	git tag -a "$$TAG" -m "$$TAG"; \
	git push origin "$$TAG"; \
	echo "Pushed $$TAG. The GitHub Release workflow should start automatically."

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
