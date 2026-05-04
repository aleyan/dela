# Release Procedure

Use the `Makefile` targets. They are the source of truth for the local release steps.

The release automation is in [.github/workflows/release.yml](/Users/aleyan/Projects/dela/.github/workflows/release.yml:1):

- manual runs are dry-run only
- real releases happen only from a pushed `v*` tag
- the tag must match `Cargo.toml`, for example `0.0.7` -> `v0.0.7`

## Normal Flow

1. Update `Cargo.toml` and `CHANGELOG.md` in the release commit.
2. Make sure that commit is on `main`.
3. Run:

```sh
make release_verify
```

4. Run the GitHub Actions dry run:
   - open `Actions`
   - open `Release`
   - click `Run workflow`
   - leave `dry_run=true`
   - run it on `main`
5. Confirm the dry run passed and inspect the uploaded artifacts.
6. Run:

```sh
make release_publish
```

7. Open `Actions` and watch the tag-triggered `Release` workflow finish.
8. Verify the release on:
   - GitHub Releases
   - crates.io

## What The Make Targets Do

`make release_verify` checks:

- version format in `Cargo.toml`
- matching version entry in `CHANGELOG.md`
- changelog date format
- changelog entry is not `Unreleased`
- the tag does not already exist locally
- the tag does not already exist on `origin`
- the version is not already on crates.io
- lint, tests, integration tests, and `cargo publish --dry-run --locked`

The release workflow also calls `make release_verify` for the shared metadata checks.

`make release_publish`:

- reruns `make release_verify`
- requires an interactive terminal
- requires `main`
- requires a clean working tree
- asks you to type the exact tag name
- creates and pushes the annotated tag

## GitHub UI Notes

Use GitHub UI for running the dry run and for monitoring the release.

Do not create the actual release from the GitHub Releases page. GitHub does allow creating a tag from `Draft a new release`, but this repo is designed for the tag push to trigger the workflow, create the draft release, upload assets, publish to crates.io, and then publish the GitHub release.

## Quick Checklist

- [ ] `Cargo.toml` version is correct
- [ ] `CHANGELOG.md` has the exact version and date
- [ ] release commit is on `main`
- [ ] `make release_verify` passed
- [ ] GitHub dry run passed
- [ ] dry-run artifacts look correct
- [ ] `make release_publish` completed
- [ ] real `Release` workflow passed
- [ ] GitHub Releases shows the release
- [ ] crates.io shows the version
