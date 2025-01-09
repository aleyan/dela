.PHONY: build test run

build:
	@echo "Building dela..."
	cargo build

test:
	@echo "Running tests..."
	cargo test

run:
	@echo "Running dela..."
	cargo run 