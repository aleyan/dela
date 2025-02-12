.PHONY: build test test_shells test_zsh test_bash test_fish test_pwsh run install builder

# Default to non-verbose output
VERBOSE ?= 0

build:
	@echo "Building dela..."
	cargo build

test:
	@echo "Running tests..."
	cargo test

# Build the base builder image
builder:
	@echo "Building base builder image..."
	docker build -t dela-builder -f tests/docker_common/Dockerfile.builder .

# Individual shell test targets
test_zsh: builder
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running zsh integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh zsh; \
	else \
		echo "Running zsh integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh zsh; \
	fi

test_bash: builder
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running bash integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh bash; \
	else \
		echo "Running bash integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh bash; \
	fi

test_fish: builder
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running fish integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh fish; \
	else \
		echo "Running fish integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh fish; \
	fi

test_pwsh: builder
	@if [ "$(VERBOSE)" = "1" ]; then \
		echo "Running PowerShell integration tests (verbose)..."; \
		VERBOSE=1 ./tests/run_tests.sh pwsh; \
	else \
		echo "Running PowerShell integration tests..."; \
		VERBOSE=0 ./tests/run_tests.sh pwsh; \
	fi

# Run all shell tests
test_shells: builder test_zsh test_bash test_fish test_pwsh

install:
	@echo "Installing dela locally..."
	cargo install --path .

run:
	@echo "Running dela..."
	cargo run 