.PHONY: npm_release_prep npm_publish npm_verify_local

NPM_VERSION ?= $(shell grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f2)
NPM_CACHE_DIR ?= $(CURDIR)/target/npm-cache

NPM_PACKAGE_DIRS := npm/dela-darwin-amd64 npm/dela-darwin-arm64 npm/dela-linux-amd64 npm/dela-linux-arm64 npm/dela

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
	mkdir -p "$(NPM_CACHE_DIR)"; \
	for dir in $(NPM_PACKAGE_DIRS); do \
		name=$$(node -p "require('./$$dir/package.json').name"); \
		version=$$(node -p "require('./$$dir/package.json').version"); \
		if [ "$$dir" != "npm/dela" ] && [ ! -x "$$dir/dela" ]; then \
			echo "Error: $$dir/dela does not exist or is not executable. Run make npm_release_prep first."; \
			exit 1; \
		fi; \
		if [ "$${NPM_PUBLISH_DRY_RUN:-0}" = "1" ]; then \
			echo "Checking $$name@$$version from $$dir..."; \
			npm --cache "$(NPM_CACHE_DIR)" pack "./$$dir" --dry-run; \
			continue; \
		fi; \
		if npm --cache "$(NPM_CACHE_DIR)" view "$$name@$$version" version >/dev/null 2>&1; then \
			echo "$$name@$$version is already published; skipping."; \
			continue; \
		fi; \
		echo "Publishing $$name@$$version from $$dir..."; \
		npm --cache "$(NPM_CACHE_DIR)" publish "./$$dir" --access public; \
	done

npm_verify_local:
	@set -euo pipefail; \
	echo "Locally verifying NPM packaging..."; \
	mkdir -p "$(NPM_CACHE_DIR)"; \
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
	npm --cache "$(NPM_CACHE_DIR)" pack ./npm/dela --dry-run; \
	echo "Verifying npm pack for platform package..."; \
	npm --cache "$(NPM_CACHE_DIR)" pack "./$$PLATFORM_DIR" --dry-run; \
	echo "Local verification complete."
