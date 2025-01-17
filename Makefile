.PHONY: build test test_shells run install

# Default to non-verbose output
VERBOSE ?= 0

build:
	@echo "Building dela..."
	cargo build

test:
	@echo "Running tests..."
	cargo test

test_shells:
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running shell integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh; \
	else \
		echo "Running shell integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh; \
	fi

install:
	@echo "Installing dela locally..."
	cargo install --path .

run:
	@echo "Running dela..."
	cargo run 