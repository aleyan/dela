# Dela!

[![CI](https://github.com/aleyan/dela/actions/workflows/integration.yml/badge.svg)](https://github.com/aleyan/dela/actions/workflows/integration.yml)
[![Crates.io](https://img.shields.io/crates/v/dela)](https://crates.io/crates/dela)
[![Docs.rs](https://docs.rs/dela/badge.svg)](https://docs.rs/dela)

![demo](https://github.com/user-attachments/assets/e0db6b85-f1b6-44b5-b6ab-291582b89f34)

Dela is a task runner that provides discovery for task definitions in various formats, and lets you execute tasks without specifying the runner while delegating their execution to your existing tools like make, npm, uv, and others.

## Installation

Install `dela` from crates.io and initialize it to set up shell integration:

```sh
$ cargo install dela
$ dela init
```

The `dela init` command will:
- Add shell integration to handle "command not found" events
- Create a `~/.dela` directory for configuration

## Usage

### Discovering tasks
List all available tasks in the current directory:

```sh
$ dela list
```

### Running tasks
You can invoke a task just by its name from the shell via `<task>`. For example here `build` task is defined in `Makefile` and is invoked directly.

```sh
$ build
```

If you are running `dela` in a directory for the first time, it will ask you to put the task or the task definition file  or the directory itself on the allowed list. This is because you might want to run `dela` in non fully trusted directories and cause inadvertent execution.

```sh
$ build
Running build from ~/Projects/dela/Makefile for the first time. Allow?
0) Allow one time
1) Allow build from ~/Projects/dela/Makefile
2) Allow any command from ~/Projects/dela/Makefile
3) Allow any command from ~/Projects/dela
4) Deny
```

You can also use `dr` to explicitly invoke `dela`:

```sh
$ dr build
```

Or use `dela run` for subshell execution:

```sh
$ dela run build
```

## MCP Server

Dela includes an [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that allows AI assistants and editors to discover and execute tasks programmatically.

### Setting Up MCP in Your Editor

```sh
$ dela mcp --init-cursor       # Cursor: .cursor/mcp.json
$ dela mcp --init-vscode       # VSCode: .vscode/mcp.json
$ dela mcp --init-codex        # OpenAI Codex: ~/.codex/config.toml
$ dela mcp --init-gemini       # Gemini CLI: ~/.gemini/settings.json
$ dela mcp --init-claude-code  # Claude Code: ~/.claude-code/settings.json
```

### Starting the MCP Server Manually

```sh
$ dela mcp [--cwd <directory>]
```

The server communicates over stdio using JSON-RPC 2.0 and streams task output via logging notifications.

### Available Tools

Tool names are stable, and `list_tasks` exposes a stable wire format (including `unique_name` with suffixes like `test-m`).

| Tool | Description |
|------|-------------|
| `list_tasks` | List all available tasks with metadata (runner, availability, allowlist status) |
| `status` | List all currently running background tasks |
| `task_start` | Start a task by unique name with optional args/env/cwd |
| `task_status` | Get status for running instances of a specific task |
| `task_output` | Get the last N lines of output for a running task (by PID) |
| `task_stop` | Stop a running task by PID (SIGTERM + grace period + SIGKILL) |

### Security

The MCP server uses the same allowlist as the CLI (`~/.dela/allowlist.toml`). Tasks must be explicitly allowlisted to be executed via MCP. Use the regular `dela` CLI commands to manage allowlists.

## Frequently Asked Questions

### How does dela work?

`dela` uses your shell's command_not_found_handler to detect when you are trying to run a command that doesn't exist. It then scans the current working directory for task definition files and executes the appropriate task runner.

### What happens if a task shares the same name with a command?

Then the bare command will be executed instead of the task. Tasks shadowed by shell builtins and conflicting with other tasks get a unique suffixed name (for example `test` from a Makefile becomes `test-m`), so you can run the task via its suffixed name; `dr <task_name>` also works.

### How do I add a new task?

You add tasks to your existing task definition files (like `Makefile`, `package.json`, or `pyproject.toml`), and `dela` will discover them automatically.

### What shell environment are tasks executed in?

When executing bare tasks or via `dr`, tasks are executed in the current shell environment. When running tasks via `dela run`, tasks are executed in a subshell environment.

### Which shell integrations are supported?

Currently, `dela` supports zsh, bash, fish, and PowerShell.

### Which task runners are supported?

Currently, `dela` supports make, npm, yarn, pnpm, bun, uv, poetry, poe (poethepoet), Maven, Gradle, GitHub Actions, Docker Compose, CMake, Travis CI, just and task.

### Which platforms are supported?

Currently, `dela` supports macOS and Linux. There is no Windows support, powershell is for Linux only.

### What is the purpose of allowlists?

Allowlists are a safety feature to prevent accidental execution (especially in untrusted directories). They’re not a sandbox, so treat tasks from downloaded repos with the same caution you would with `make` or `npm`.

### Is dela production ready?

`dela` is not at 0.1 yet and its cli is subject to change.

### What are the alternatives to dela?

Other task runners that handle multiple runners are [task-keeper](https://github.com/linux-china/task-keeper), [ds](https://github.com/metaist/ds), and [rt](https://github.com/unvalley/rt).

## Development

For local development:

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

Note: `dela` is not at 0.1 yet and its CLI is subject to change.
