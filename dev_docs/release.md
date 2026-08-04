# Release Procedure

Use the `Makefile` targets. They are the source of truth for the local release steps.

The release automation is in .github/workflows/release.yml:

- manual runs are dry-run only
- real releases happen only from a pushed `v*` tag
- the tag must match `Cargo.toml`, for example `0.0.7` -> `v0.0.7`

## Normal Flow

1. Pick the next version, for example `0.0.7`.
2. Update Cargo, npm package versions, and the changelog heading:

```sh
make release_set_versions DELA_VERSION=0.0.7
```

3. Fill in the new `CHANGELOG.md` entry with release notes.
4. Commit those changes and make sure that commit is on `main`.
5. Run the local prerelease checks:

```sh
make release_verify
```

`make release_verify` requires `CARGO_REGISTRY_TOKEN` in the environment and
validates it. npm publishing uses GitHub Actions trusted publishing (OIDC), so
there is no long-lived npm token to validate locally. The GitHub dry run
validates the crates.io and GitHub credentials and checks the npm package
contents without publishing them.

6. Run the GitHub dry run from the UI:
   - open `Actions`
   - open `Release`
   - click `Run workflow`
   - leave `dry_run=true`
   - run it on `main`
7. Confirm the dry run passed and inspect the uploaded artifacts.
8. Start the real release by pushing the `v*` tag. The local helper is:

```sh
make release_publish
```

`make release_publish` only guards and pushes the tag. GitHub Actions does the
actual publishing after the tag exists.

9. Open `Actions` and watch the tag-triggered `Release` workflow finish.
10. Verify the release on:
   - GitHub Releases
   - crates.io
   - npmjs.com (as `@aleyan/dela`)

## What The Make Targets Do

`make release_set_versions DELA_VERSION=x.y.z` updates `Cargo.toml`,
`Cargo.lock`, every npm `package.json`, and creates or updates the matching
`CHANGELOG.md` heading.

`make release_verify` checks:

- version format in `Cargo.toml`
- matching version entry in `CHANGELOG.md`
- changelog date format
- changelog entry is not `Unreleased`
- the tag does not already exist locally
- the tag does not already exist on `origin`
- the version is not already on crates.io
- `CARGO_REGISTRY_TOKEN` is accepted by crates.io
- lint, tests, integration tests, and `cargo publish --dry-run --locked`

The release workflow calls private target `make _release_verify_github`, which
emits workflow outputs and verifies release metadata plus the crates.io and
GitHub tokens.

## npm Trusted Publishing Setup

The `publish-npm` job publishes through npm trusted publishing. It runs on a
GitHub-hosted runner with Node.js 24 and requests a short-lived OIDC identity
token using the `id-token: write` job permission. Do not add `NPM_TOKEN` or
`NODE_AUTH_TOKEN` to this job.

Each of these packages must have its own trusted publisher configured on
npmjs.com because npm stores trusted publisher settings per package:

- `@aleyan/dela-darwin-amd64`
- `@aleyan/dela-darwin-arm64`
- `@aleyan/dela-linux-amd64`
- `@aleyan/dela-linux-arm64`
- `@aleyan/dela`

Use the following settings for every package:

- provider: GitHub Actions
- organization or user: `aleyan`
- repository: `dela`
- workflow filename: `release.yml`
- environment: leave unset
- allowed action: `npm publish`

Configure all five packages before removing the old `NPM_TOKEN` repository
secret. Once a tag release succeeds through OIDC, revoke the npm automation
token and delete the unused GitHub secret. If a release is interrupted, rerun
the workflow: already-published package versions are skipped, platform packages
are published first, and the wrapper package is published last.

`make release_publish`:

- reruns `make release_verify`
- requires an interactive terminal
- requires `main`
- requires a clean working tree
- requires local `main` to match `origin/main`
- asks you to type the exact tag name
- creates and pushes the annotated tag that triggers GitHub Actions

## GitHub UI Notes

Use GitHub UI for running the dry run and for monitoring the real tag-triggered
release.

Do not create the actual release from the GitHub Releases page. GitHub does allow creating a tag from `Draft a new release`, but this repo is designed for the tag push to trigger the workflow, create the draft release, upload assets, publish to crates.io, and then publish the GitHub release.

## Quick Checklist

- [ ] `Cargo.toml` version is correct
- [ ] npm package versions match `Cargo.toml`
- [ ] `CHANGELOG.md` has the exact version and date
- [ ] release commit is on `main`
- [ ] `make release_verify` passed
- [ ] GitHub dry run passed
- [ ] dry-run artifacts look correct
- [ ] trusted publishing is configured for all five npm packages
- [ ] `make release_publish` completed
- [ ] real `Release` workflow passed
- [ ] GitHub Releases shows the release
- [ ] crates.io shows the version
- [ ] npmjs.com shows the version
