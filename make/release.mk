.PHONY: release_verify release_publish release_notes

RELEASE_VERIFY_EXPECT_TAG ?=
RELEASE_VERIFY_SKIP_TAG_EXISTS ?= 0
RELEASE_VERIFY_SKIP_CRATES_CHECK ?= 0
RELEASE_VERIFY_SKIP_TESTS ?= 0

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

# Extracts release notes for the current version from CHANGELOG.md into release_notes.md
release_notes:
	@set -euo pipefail; \
	VERSION=$$(grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f2); \
	echo "Extracting release notes for $$VERSION into release_notes.md..."; \
	awk -v version="$$VERSION" ' \
		BEGIN { pattern = "^## \\[" version "\\]"; } \
		/^## \[/ { if (in_section) exit; if ($$0 ~ pattern) { in_section = 1; next; } } \
		in_section { print; } \
	' CHANGELOG.md > release_notes.md

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
	echo "Fetching origin/main to verify sync..."; \
	git fetch origin main --quiet; \
	REMOTE_COMMIT=$$(git rev-parse --verify origin/main); \
	if [ "$$(git rev-parse HEAD)" != "$$REMOTE_COMMIT" ]; then \
		echo "Error: local HEAD does not match origin/main. Please pull or push first."; \
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
