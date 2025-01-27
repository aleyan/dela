.PHONY: build test test_shells test_zsh test_bash test_fish test_pwsh run install

# Default to non-verbose output
VERBOSE ?= 0

build:
	@echo "Building dela..."
	cargo build

test:
	@echo "Running tests..."
	cargo test

# Individual shell test targets
test_zsh:
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running zsh integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh zsh; \
	else \
		echo "Running zsh integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh zsh; \
	fi

test_bash:
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running bash integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh bash; \
	else \
		echo "Running bash integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh bash; \
	fi

test_fish:
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running fish integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh fish; \
	else \
		echo "Running fish integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh fish; \
	fi

test_pwsh:
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running PowerShell integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh pwsh; \
	else \
		echo "Running PowerShell integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh pwsh; \
	fi

# Test all shells
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