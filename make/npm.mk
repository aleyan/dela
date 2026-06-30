.PHONY: npm_release_prep npm_publish npm_verify_local

NPM_VERSION := $(shell grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f2)

npm_release_prep:
	@set -euo pipefail; \
	echo "Preparing NPM packages for version $(NPM_VERSION)..."; \
	mkdir -p npm/dela-darwin-amd64 npm/dela-darwin-arm64 npm/dela-linux-amd64 npm/dela-linux-arm64; \
	echo "Extracting platform binaries from dist/..."; \
	tar -xzf dist/dela-darwin-amd64.tar.gz -C npm/dela-darwin-amd64; \
	tar -xzf dist/dela-darwin-arm64.tar.gz -C npm/dela-darwin-arm64; \
	tar -xzf dist/dela-linux-amd64.tar.gz -C npm/dela-linux-amd64; \
	tar -xzf dist/dela-linux-arm64.tar.gz -C npm/dela-linux-arm64; \
	node -e " \
		const fs = require('fs'); \
		const version = '$(NPM_VERSION)'; \
		const packages = [ \
			'npm/dela', \
			'npm/dela-darwin-amd64', \
			'npm/dela-darwin-arm64', \
			'npm/dela-linux-amd64', \
			'npm/dela-linux-arm64' \
		]; \
		packages.forEach(dir => { \
			const file = dir + '/package.json'; \
			const pkg = JSON.parse(fs.readFileSync(file, 'utf8')); \
			pkg.version = version; \
			if (pkg.optionalDependencies) { \
				for (const dep of Object.keys(pkg.optionalDependencies)) { \
					pkg.optionalDependencies[dep] = version; \
				} \
			} \
			fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + '\n'); \
		}); \
	"; \
	for dir in npm/dela-darwin-amd64 npm/dela-darwin-arm64 npm/dela-linux-amd64 npm/dela-linux-arm64; do \
		if [ ! -x "$$dir/dela" ]; then \
			echo "Error: $$dir/dela does not exist or is not executable!"; \
			exit 1; \
		fi; \
	done; \
	echo "NPM packages prepared successfully."

npm_publish:
	@set -euo pipefail; \
	DRY_RUN_ARG=""; \
	if [ "$${NPM_PUBLISH_DRY_RUN:-0}" = "1" ]; then \
		DRY_RUN_ARG="--dry-run"; \
	fi; \
	for dir in npm/dela-darwin-amd64 npm/dela-darwin-arm64 npm/dela-linux-amd64 npm/dela-linux-arm64 npm/dela; do \
		echo "Publishing $$dir (dry-run: $${NPM_PUBLISH_DRY_RUN:-0})..."; \
		npm publish "$$dir" --access public $$DRY_RUN_ARG; \
	done

npm_verify_local:
	@set -euo pipefail; \
	echo "Locally verifying NPM packaging..."; \
	if [ ! -f "target/release/dela" ]; then \
		echo "dela binary not found in target/release/. Building release..."; \
		cargo build --release; \
	fi; \
	OS=$$(uname -s | tr '[:upper:]' '[:lower:]'); \
	ARCH=$$(uname -m); \
	if [ "$$ARCH" = "x86_64" ]; then ARCH="amd64"; fi; \
	if [ "$$ARCH" = "aarch64" ]; then ARCH="arm64"; fi; \
	PLATFORM_DIR="npm/dela-$$OS-$$ARCH"; \
	echo "Detected local platform directory: $$PLATFORM_DIR"; \
	if [ ! -d "$$PLATFORM_DIR" ]; then \
		echo "Error: Local platform directory $$PLATFORM_DIR does not exist!"; \
		exit 1; \
	fi; \
	cp target/release/dela "$$PLATFORM_DIR/dela"; \
	node -e " \
		const fs = require('fs'); \
		const version = '$(NPM_VERSION)'; \
		const packages = [ \
			'npm/dela', \
			'npm/dela-darwin-amd64', \
			'npm/dela-darwin-arm64', \
			'npm/dela-linux-amd64', \
			'npm/dela-linux-arm64' \
		]; \
		packages.forEach(dir => { \
			const file = dir + '/package.json'; \
			const pkg = JSON.parse(fs.readFileSync(file, 'utf8')); \
			pkg.version = version; \
			if (pkg.optionalDependencies) { \
				for (const dep of Object.keys(pkg.optionalDependencies)) { \
					pkg.optionalDependencies[dep] = version; \
				} \
			} \
			fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + '\n'); \
		}); \
	"; \
	echo "Verifying launcher execution..."; \
	node npm/dela/bin/dela --version; \
	echo "Verifying npm pack for wrapper package..."; \
	npm pack ./npm/dela --dry-run; \
	echo "Verifying npm pack for platform package..."; \
	npm pack "./$$PLATFORM_DIR" --dry-run; \
	echo "Local verification complete."
