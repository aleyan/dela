.PHONY: _npm_release_prep npm_publish npm_verify_local

NPM_CACHE_DIR ?= $(CURDIR)/target/npm-cache

NPM_PLATFORM_DIRS := npm/dela-darwin-amd64 npm/dela-darwin-arm64 npm/dela-linux-amd64 npm/dela-linux-arm64
NPM_PACKAGE_DIRS := $(NPM_PLATFORM_DIRS) npm/dela

# Prepare npm package directories from already-built dist/*.tar.gz release archives.
_npm_release_prep: release_set_versions
	@set -euo pipefail; \
	echo "Preparing NPM packages for version $(DELA_VERSION)..."; \
	for dir in $(NPM_PLATFORM_DIRS); do \
		archive="dist/$$(basename $$dir).tar.gz"; \
		mkdir -p "$$dir"; \
		rm -f "$$dir/dela"; \
		echo "Extracting $$archive -> $$dir"; \
		tar -xzf "$$archive" -C "$$dir"; \
	done; \
	for dir in $(NPM_PLATFORM_DIRS); do \
		if [ ! -x "$$dir/dela" ]; then \
			echo "Error: $$dir/dela does not exist or is not executable!"; \
			exit 1; \
		fi; \
	done; \
	echo "NPM packages prepared successfully."

# Publish npm packages, or pack-check them when NPM_PUBLISH_DRY_RUN=1.
npm_publish:
	@set -euo pipefail; \
	mkdir -p "$(NPM_CACHE_DIR)"; \
	NPM_USERCONFIG=""; \
	if [ "$${NPM_PUBLISH_DRY_RUN:-0}" != "1" ]; then \
		if [ -z "$${NPM_TOKEN:-}" ]; then \
			echo "Error: NPM_TOKEN is required to publish npm packages."; \
			exit 1; \
		fi; \
		mkdir -p target/npm-auth; \
		NPM_USERCONFIG=$$(mktemp "$(CURDIR)/target/npm-auth/npmrc.XXXXXX"); \
		trap 'rm -f "$$NPM_USERCONFIG"' EXIT; \
		chmod 600 "$$NPM_USERCONFIG"; \
		printf '%s\n' "//registry.npmjs.org/:_authToken=$${NPM_TOKEN}" > "$$NPM_USERCONFIG"; \
	fi; \
	for dir in $(NPM_PACKAGE_DIRS); do \
		name=$$(node -p "require('./$$dir/package.json').name"); \
		version=$$(node -p "require('./$$dir/package.json').version"); \
		if [ "$$dir" != "npm/dela" ] && [ ! -x "$$dir/dela" ]; then \
			echo "Error: $$dir/dela does not exist or is not executable. Run make _npm_release_prep first."; \
			exit 1; \
		fi; \
		if [ "$${NPM_PUBLISH_DRY_RUN:-0}" = "1" ]; then \
			echo "Checking $$name@$$version from $$dir..."; \
			npm --cache "$(NPM_CACHE_DIR)" pack "./$$dir" --dry-run; \
			continue; \
		fi; \
		if npm --cache "$(NPM_CACHE_DIR)" view "$$name@$$version" version --registry=https://registry.npmjs.org >/dev/null 2>&1; then \
			echo "$$name@$$version is already published; skipping."; \
			continue; \
		fi; \
		echo "Publishing $$name@$$version from $$dir..."; \
		npm --userconfig "$$NPM_USERCONFIG" --cache "$(NPM_CACHE_DIR)" publish "./$$dir" --access public --registry=https://registry.npmjs.org; \
	done

# Build the local release binary if needed and verify the npm launcher/package shape.
npm_verify_local: release_set_versions
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
	echo "Verifying launcher execution..."; \
	node npm/dela/bin/dela --version; \
	echo "Verifying npm pack for wrapper package..."; \
	npm --cache "$(NPM_CACHE_DIR)" pack ./npm/dela --dry-run; \
	echo "Verifying npm pack for platform package..."; \
	npm --cache "$(NPM_CACHE_DIR)" pack "./$$PLATFORM_DIR" --dry-run; \
	echo "Local verification complete."
