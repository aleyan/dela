.PHONY: release_set_versions release_verify _release_verify_github _release_verify_metadata _release_verify_tag_available _release_verify_crate_unpublished _release_verify_tokens _release_verify_cargo_token _release_verify_github_token _release_verify_tests _release_emit_github_outputs _release_guard_github_dry_run release_publish _release_notes

RELEASE_VERSION = $(shell grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f2)
RELEASE_TAG = v$(RELEASE_VERSION)
DELA_VERSION ?= $(RELEASE_VERSION)

# Set Cargo.toml, Cargo.lock, CHANGELOG.md, and npm package versions.
release_set_versions:
	node scripts/set_versions.js "$(DELA_VERSION)"

# Full local prerelease check for humans before pushing the release tag.
release_verify: _release_verify_metadata _release_verify_tag_available _release_verify_crate_unpublished _release_verify_tokens _release_verify_tests
	@echo "Release verification passed for $(RELEASE_TAG)."

# GitHub Actions entrypoint. Emits workflow outputs and validates repository
# secrets, but leaves lint/tests/package checks to dedicated jobs.
_release_verify_github: _release_guard_github_dry_run _release_emit_github_outputs _release_verify_metadata _release_verify_tokens _release_verify_github_token
	@echo "GitHub release verification passed for $(RELEASE_TAG)."

_release_verify_metadata:
	@set -euo pipefail; \
	echo "Verifying release metadata for $(RELEASE_TAG)..."; \
	if ! echo "$(RELEASE_VERSION)" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$$'; then \
		echo "Error: version $(RELEASE_VERSION) does not follow semantic versioning (X.Y.Z)."; \
		exit 1; \
	fi; \
	if ! grep -q "^## \[$(RELEASE_VERSION)\]" CHANGELOG.md; then \
		echo "Error: version $(RELEASE_VERSION) not found in CHANGELOG.md."; \
		exit 1; \
	fi; \
	if grep -q "^## \[$(RELEASE_VERSION)\] - Unreleased" CHANGELOG.md; then \
		echo "Error: version $(RELEASE_VERSION) is still marked as Unreleased in CHANGELOG.md."; \
		exit 1; \
	fi; \
	if ! grep -q "^## \[$(RELEASE_VERSION)\] - [0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}" CHANGELOG.md; then \
		echo "Error: version $(RELEASE_VERSION) in CHANGELOG.md does not have a YYYY-MM-DD date."; \
		exit 1; \
	fi; \
	if [ "$${GITHUB_REF_TYPE:-}" = "tag" ] && [ "$${GITHUB_REF_NAME:-}" != "$(RELEASE_TAG)" ]; then \
		echo "Error: GitHub tag $${GITHUB_REF_NAME:-<missing>} does not match $(RELEASE_TAG)."; \
		exit 1; \
	fi

_release_verify_tag_available:
	@set -euo pipefail; \
	if git rev-parse -q --verify "refs/tags/$(RELEASE_TAG)" >/dev/null; then \
		echo "Error: local tag $(RELEASE_TAG) already exists."; \
		exit 1; \
	fi; \
	if git ls-remote --exit-code --tags origin "refs/tags/$(RELEASE_TAG)" >/dev/null 2>&1; then \
		echo "Error: remote tag $(RELEASE_TAG) already exists on origin."; \
		exit 1; \
	fi

_release_verify_crate_unpublished:
	@set -euo pipefail; \
	if ! command -v jq >/dev/null 2>&1; then \
		echo "Error: jq is required for release_verify."; \
		exit 1; \
	fi; \
	RESPONSE=$$(curl --fail --silent --show-error --location https://crates.io/api/v1/crates/dela); \
	if echo "$$RESPONSE" | jq -e --arg version "$(RELEASE_VERSION)" '.versions[] | select(.num == $$version)' >/dev/null; then \
		echo "Error: version $(RELEASE_VERSION) already exists on crates.io."; \
		exit 1; \
	fi

_release_verify_tokens: _release_verify_cargo_token

_release_verify_cargo_token:
	@set -euo pipefail; \
	if [ -z "$${CARGO_REGISTRY_TOKEN:-}" ]; then \
		echo "Error: CARGO_REGISTRY_TOKEN is required for release verification."; \
		exit 1; \
	fi; \
	echo "Validating CARGO_REGISTRY_TOKEN..."; \
	if ! cargo owner --list dela --token "$$CARGO_REGISTRY_TOKEN" >/dev/null; then \
		echo "Error: CARGO_REGISTRY_TOKEN was rejected by crates.io."; \
		exit 1; \
	fi

_release_verify_github_token:
	@set -euo pipefail; \
	if ! command -v gh >/dev/null 2>&1; then \
		echo "Error: gh is required to validate GH_TOKEN."; \
		exit 1; \
	fi; \
	if [ -z "$${GH_TOKEN:-}" ]; then \
		echo "Error: GH_TOKEN is required for GitHub release verification."; \
		exit 1; \
	fi; \
	if [ -z "$${GITHUB_REPOSITORY:-}" ]; then \
		echo "Error: GITHUB_REPOSITORY is required for GitHub release verification."; \
		exit 1; \
	fi; \
	echo "Validating GH_TOKEN..."; \
	if ! gh api "repos/$${GITHUB_REPOSITORY}" --jq .full_name >/dev/null; then \
		echo "Error: GH_TOKEN could not access $${GITHUB_REPOSITORY}."; \
		exit 1; \
	fi

_release_verify_tests:
	@set -euo pipefail; \
	echo "Running lint..."; \
	$(MAKE) lint; \
	echo "Running unit tests..."; \
	$(MAKE) tests; \
	echo "Running integration tests..."; \
	$(MAKE) tests_integration; \
	echo "Running cargo publish dry run..."; \
	cargo publish --dry-run --locked

_release_emit_github_outputs:
	@set -euo pipefail; \
	if [ -n "$${GITHUB_OUTPUT:-}" ]; then \
		echo "version=$(RELEASE_VERSION)" >> "$$GITHUB_OUTPUT"; \
		echo "tag_name=$(RELEASE_TAG)" >> "$$GITHUB_OUTPUT"; \
	else \
		echo "version=$(RELEASE_VERSION)"; \
		echo "tag_name=$(RELEASE_TAG)"; \
	fi

_release_guard_github_dry_run:
	@set -euo pipefail; \
	if [ "$${GITHUB_EVENT_NAME:-}" = "workflow_dispatch" ] && [ "$${RELEASE_DRY_RUN:-true}" != "true" ]; then \
		echo "::error::Manual runs are dry-run only. Push a v* tag to publish."; \
		exit 1; \
	fi

# Extracts release notes for the current version from CHANGELOG.md into release_notes.md
_release_notes:
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
