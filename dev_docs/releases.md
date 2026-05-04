# Release Procedure

This document describes how to cut a release for `dela`.

The repository's release automation lives in [.github/workflows/release.yml](/Users/aleyan/Projects/dela/.github/workflows/release.yml:1). The important behavior is:

- Manual runs are dry-run only.
- Real releases happen only when a tag matching `v*` is pushed.
- The tag must match the version in `Cargo.toml`. Example: version `0.0.7` requires tag `v0.0.7`.
- The workflow runs validation, lint, unit tests, integration tests, `cargo publish --dry-run`, and artifact builds before publishing to crates.io.
- On a real tag push, the workflow creates a draft GitHub release, uploads release assets, publishes to crates.io, then publishes the GitHub release.

## Prerequisites

Before starting a release:

- Ensure the release commit is already on `main`.
- Ensure `Cargo.toml` has the version you want to release.
- Ensure `CHANGELOG.md` contains a section for that exact version in the form `## [X.Y.Z] - YYYY-MM-DD`.
- Ensure the changelog entry is not marked `Unreleased`.
- Ensure you have permission to push tags to the repository.
- Ensure the repo secrets used by the release workflow are configured in GitHub:
  - `CARGO_REGISTRY_TOKEN`
  - the default `GITHUB_TOKEN` is used automatically by GitHub Actions

## Preferred Release Flow

Use this order:

1. Prepare the release commit locally.
2. Run the GitHub Actions dry run manually.
3. Inspect the dry-run artifacts.
4. Push the real tag.
5. Watch the real release workflow complete.
6. Verify crates.io and the GitHub release page.

## Prepare The Release Commit

Update the version and changelog in the same commit.

Typical checks to run locally before opening the dry run:

```sh
make lint
make tests
make tests_integration
cargo publish --dry-run --locked
```

The integration tests are slower, but the release workflow runs them, so it is better to catch failures before pushing a tag.

## Run The Dry Run In GitHub

The manual workflow is the safe test path. It validates the exact release pipeline without publishing anything.

In GitHub UI:

1. Open the repository.
2. Click `Actions`.
3. In the left sidebar, click `Release`.
4. Click `Run workflow`.
5. Leave `dry_run` set to `true`.
6. Select the branch containing the release commit, normally `main`.
7. Click `Run workflow`.

Notes:

- GitHub documents that manually running a workflow requires `workflow_dispatch` and the workflow file to exist on the default branch.
- This repository intentionally rejects manual runs with `dry_run=false`.

## Inspect Dry-Run Results

After the dry run finishes:

1. Open the workflow run in `Actions`.
2. Confirm all jobs succeeded.
3. Scroll to the `Artifacts` section on the run summary page.
4. Download the built archives and inspect them if needed.

Expected artifacts:

- `dela-linux-amd64`
- `dela-linux-arm64`
- `dela-darwin-amd64`
- `dela-darwin-arm64`

If the dry run fails, fix the issue and rerun the dry run before creating a tag.

## Create The Real Release Tag

For this repository, the tag push is the release trigger. The preferred method is local git, not the GitHub release UI.

From your local checkout:

```sh
git checkout main
git pull --ff-only
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Replace `X.Y.Z` with the version from `Cargo.toml`.

Example:

```sh
git tag -a v0.0.7 -m "v0.0.7"
git push origin v0.0.7
```

Why local git is preferred:

- The workflow is triggered by the tag push itself.
- The workflow creates and publishes the GitHub release for you.
- Creating a release manually in the GitHub UI can conflict with this automation.

## About Tagging In GitHub UI

GitHub's documented web UI flow for creating a new tag is through the Releases page:

1. Open the repository.
2. Click `Releases`.
3. Click `Draft a new release`.
4. In `Choose a tag`, type a new tag such as `v0.0.7`.
5. Click `Create new tag`.
6. Choose the target branch.

However, do not use that flow for normal `dela` releases.

Reason:

- In GitHub's UI, creating a new release and creating a new tag are part of the same flow.
- This repository's automation expects the tag push to create the draft release and attach artifacts.
- Starting with `Draft a new release` in the UI can create a release object outside the intended automated path.

Use the GitHub UI flow only if you specifically need to understand where GitHub puts tag creation in the web interface. For actual releases in this repo, push the tag from local git.

## Monitor The Real Release

After pushing the tag:

1. Open `Actions`.
2. Open the `Release` workflow run triggered by the tag push.
3. Confirm these stages succeed:
   - release metadata verification
   - lint and unit tests
   - integration tests
   - `cargo publish --dry-run`
   - binary artifact builds
   - draft GitHub release creation
   - asset upload
   - crates.io publish
   - final GitHub release publish

When complete:

- crates.io should show the new version
- the GitHub `Releases` page should show the new published release
- the release should contain the built archives

## Verify After Release

Check:

- `Cargo.toml` version matches the released version
- the release appears at `https://github.com/aleyan/dela/releases`
- the crate appears at `https://crates.io/crates/dela`
- the release page contains the expected tarballs

## Failure Recovery

### Dry run failed

- Fix the code or metadata.
- Re-run the dry run.
- Do not push the release tag until the dry run passes.

### Real release failed before crates.io publish

- Fix the issue.
- Re-run the failed workflow if appropriate, or delete the bad tag and create a new release commit/tag.
- If you delete and recreate a tag, make sure you understand exactly which commit the new tag points to.

### Crates.io publish succeeded but GitHub release publish failed

This is the most important recovery case.

In this repository, re-running the workflow for the same tag should be the first recovery step because:

- the workflow checks whether the crates.io version already exists and skips `cargo publish` if it does
- the workflow upload step uses `--clobber` for release assets
- the workflow can publish the existing draft release after crates.io has already succeeded

Do not bump the crate version just because the GitHub release step failed after crates.io already accepted the version.

## Release Checklist

Use this checklist each time:

- [ ] `Cargo.toml` version is correct
- [ ] `CHANGELOG.md` has the exact version and date
- [ ] release commit is merged to `main`
- [ ] local checks are clean
- [ ] GitHub dry run passed
- [ ] dry-run artifacts look correct
- [ ] tag `vX.Y.Z` was created locally
- [ ] tag was pushed to `origin`
- [ ] real `Release` workflow passed
- [ ] crates.io shows the new version
- [ ] GitHub Releases page shows the new release with assets

## References

- GitHub Docs: manually running a workflow
  - https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow
- GitHub Docs: managing releases in a repository
  - https://docs.github.com/articles/creating-releases
- GitHub Docs: about releases
  - https://docs.github.com/repositories/releasing-projects-on-github/about-releases
- GitHub Docs: downloading workflow artifacts
  - https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts
