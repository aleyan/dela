# Development

For development you need to have rustup toolchain, npm, uv, make, and docker.

To get started with development run this in project root:

```sh
$ cargo install --path .
$ source resources/zsh.sh  # or equivalent for your shell
```

## Testing MCP with Inspector

To test the MCP server interactively with the [MCP Inspector](https://github.com/modelcontextprotocol/inspector):

```sh
# Build and run with Inspector
$ cargo build --quiet
$ RUST_LOG=warn npx @modelcontextprotocol/inspector ./target/debug/dela mcp
```

## Testing

Run all tests:
```sh
$ make tests_integration
```

Run integrations test with `test_shells`, it requires `Make`, `Docker`, and `dela` to be installed.

```sh
$ tests_integration
```

## Publishing Releases

See [release.md](release.md) for the full procedure. In brief:

1. Bump versions and update `CHANGELOG.md`.

```sh
make release_set_versions DELA_VERSION=0.0.7
```

2. Commit to `main`, run checks, and run the GitHub dry run:

```sh
make release_verify
```

GitHub: `Actions` -> `Release` -> `Run workflow` -> `main`, with
`dry_run=true`.

3. Start the real GitHub release by pushing the `v*` tag. The local helper is:

```sh
make release_publish
```

GitHub Actions does the publishing. `make release_publish` only guards and
pushes the tag; use it as the CLI escape hatch when you are not creating the tag
through GitHub. Do not publish from the GitHub Releases page manually.
