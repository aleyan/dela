.PHONY: build test test_shells run

build:
	@echo "Building dela..."
	cargo build

test:
	@echo "Running tests..."
	cargo test

test_shells:
	@echo "Running shell integration tests..."
	./tests/docker/run_tests.sh

run:
	@echo "Running dela..."
	cargo run 