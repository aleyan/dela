.PHONY: crap_check crap_update crap_run _crap_deps

_crap_deps:
	@echo "Checking dependencies (cargo-llvm-cov and cargo-crap)..."
	@cargo llvm-cov --version 2>/dev/null | grep -q "0.8.7" || cargo install cargo-llvm-cov --version 0.8.7 --force --locked
	@cargo crap --version 2>/dev/null | grep -q "0.2.0" || cargo install cargo-crap --version 0.2.0 --force --locked

crap_run: _crap_deps
	@echo "Running CRAP analysis..."
	cargo llvm-cov --lcov --output-path lcov.info
	cargo crap --lcov lcov.info --top 20

crap_check: _crap_deps
	@echo "Checking CRAP against baseline..."
	cargo llvm-cov --lcov --output-path lcov.info
	cargo crap --lcov lcov.info --baseline cargo_crap_baseline.json --fail-regression

crap_update: _crap_deps
	@echo "Updating CRAP baseline..."
	cargo llvm-cov --lcov --output-path lcov.info
	cargo crap --lcov lcov.info --format json --output cargo_crap_baseline.json
