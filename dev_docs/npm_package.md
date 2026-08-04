# NPM Package Distribution

Goal: make `dela` easy to install for users who already have Node.js.

```sh
npm install -g @aleyan/dela
dela init
```

The npm package should install the same Rust binary that is published through
GitHub releases and crates.io. npm is only a distribution channel; it should not
change how `dela` runs.

## Approach

Use the standard native-binary npm layout:

- `@aleyan/dela` is the package users install. It contains a small JS launcher.
- Platform packages contain the prebuilt `dela` binary for one OS and CPU.
- The main package lists platform packages as `optionalDependencies`.
- npm installs only the platform package matching the user's machine.

This is the same pattern used by tools such as esbuild, swc, biome, and
tailwind. It gives users a one-command install without compiling Rust locally.

## Packages

| Package | Platform | npm fields |
|---|---|---|
| `@aleyan/dela` | wrapper package | none |
| `@aleyan/dela-darwin-amd64` | macOS Intel | `os: ["darwin"]`, `cpu: ["x64"]` |
| `@aleyan/dela-darwin-arm64` | macOS Apple Silicon | `os: ["darwin"]`, `cpu: ["arm64"]` |
| `@aleyan/dela-linux-amd64` | Linux x86_64 | `os: ["linux"]`, `cpu: ["x64"]` |
| `@aleyan/dela-linux-arm64` | Linux ARM64 | `os: ["linux"]`, `cpu: ["arm64"]` |

Package names use `amd64` and `arm64` to match the GitHub release archive names.
npm's `cpu` values must still use Node's names, so amd64 is `x64`.

## Repository Layout

```text
npm/
  dela/
    package.json
    bin/dela
  dela-darwin-amd64/package.json
  dela-darwin-arm64/package.json
  dela-linux-amd64/package.json
  dela-linux-arm64/package.json
```

The platform package directories only need `package.json` in source control.
CI copies the compiled binary into each directory before publishing.

Add this to `.gitignore` so release artifacts are not committed:

```gitignore
npm/*/dela
```

## Package Requirements

The main package should:

- expose `dela` through the `bin` field
- include only the launcher and package metadata in the published package
- have no regular runtime dependencies
- declare all platform packages as exact-version `optionalDependencies`
- use the same version as `Cargo.toml`

Each platform package should:

- use the same version as `Cargo.toml`
- set `os` and `cpu` so npm can skip non-matching packages
- include only `package.json` and the `dela` binary in the published package
- contain one executable file named `dela` at publish time

All npm packages must be published together with the same version.

## Launcher Requirements

`npm/dela/bin/dela` should be a Node.js script that:

- maps `process.platform` and `process.arch` to the correct platform package
- finds that package with `require.resolve`
- executes the native binary with inherited stdio
- forwards arguments and exit status unchanged
- prints a clear unsupported-platform or reinstall message when the binary is missing

Use `child_process.spawnSync` or `execFileSync` with `stdio: "inherit"` so
interactive prompts from `dela init` and allowlist confirmation continue to work.

## Release Flow

Add a `publish-npm` job to the release workflow after the crate publish succeeds.
It should:

1. Download the existing GitHub release build artifacts.
2. Set all npm package versions from the release version.
3. Copy each extracted `dela` binary into its platform package directory.
4. Verify every platform package contains an executable `dela` file.
5. Publish platform packages first.
6. Publish `@aleyan/dela` last.

Publishing the wrapper last matters because it references the platform packages
through `optionalDependencies`.
The publish step should skip package versions that are already on npm so a
failed release can be rerun after a partial publish.

The workflow publishes with npm trusted publishing (OIDC), not a long-lived npm
token. The `publish-npm` job must run on a GitHub-hosted runner, use Node.js
22.14.0 or newer, and grant `id-token: write`. npm then exchanges the GitHub
identity for a short-lived credential during `npm publish`.

Trusted publisher settings are per package. Configure all five packages on
npmjs.com for GitHub Actions using organization/user `aleyan`, repository
`dela`, workflow filename `release.yml`, no environment, and the `npm publish`
allowed action. After one successful OIDC release, revoke the old automation
token and remove the unused `NPM_TOKEN` GitHub secret.

The GitHub dry run validates package contents but cannot validate the npm-side
trusted publisher configuration without publishing. A real tag release is the
end-to-end check. Reruns remain safe because `make npm_publish` skips versions
that are already public. Non-dry-run `make npm_publish` is restricted to GitHub
Actions with an OIDC identity; use `make npm_verify_local` for local checks.

## Local Verification

Before publishing, verify the launcher and package contents:

```sh
cargo build --release
cp target/release/dela npm/dela-darwin-arm64/dela # use the local platform dir
node npm/dela/bin/dela --version
npm pack ./npm/dela --dry-run
npm pack ./npm/dela-darwin-arm64 --dry-run
```

For the first full end-to-end install test, use a temporary registry such as
Verdaccio. For a local no-registry smoke test, install the wrapper tarball with
the matching platform tarball in an isolated prefix:

```sh
npm install -g \
  --prefix target/npm-install-prefix \
  --omit=optional \
  ./aleyan-dela-darwin-arm64-0.0.6.tgz \
  ./aleyan-dela-0.0.6.tgz
target/npm-install-prefix/bin/dela --version
```

Use the platform tarball that matches the test machine.

## User Documentation

After npm publishing is implemented and verified, make npm the first install
option in `README.md` and `INSTALL.md`:

```sh
npm install -g @aleyan/dela
dela init
```

Keep `cargo install dela` and direct GitHub binaries as alternate install paths.

## Implementation Checklist

- [x] Create the `npm/` package directories.
- [x] Add the JS launcher.
- [x] Add `npm/*/dela` to `.gitignore`.
- [x] Add npm publishing to `.github/workflows/release.yml`.
- [ ] Configure trusted publishing for all five npm packages.
- [x] Verify local `npm pack` output.
- [x] Test a global install from the published wrapper package.
- [x] Publish platform packages and then the wrapper package.
- [x] Update `README.md`, `INSTALL.md`, and release docs with npm install instructions.
