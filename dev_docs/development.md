## Development

For development you need to have rustup toolchain, npm, uv, make, and docker.

To get started with development run this in project root:

```sh
$ cargo install --path .
$ source resources/zsh.sh  # or equivalent for your shell
```

### Testing MCP with Inspector

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
