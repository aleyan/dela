# Dela!

[![CI](https://github.com/alex/dela/actions/workflows/integration.yml/badge.svg)](https://github.com/alex/dela/actions/workflows/integration.yml)
[![Crates.io](https://img.shields.io/crates/v/dela)](https://crates.io/crates/dela)
[![Docs.rs](https://docs.rs/dela/badge.svg)](https://docs.rs/dela)

Dela is a lightweight task runner that provides discovery for task definitions in various formats, and lets you execute tasks without specifying the runner while delegating their execution to your existing tools like Make, npm, uv, and others.

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

## Frequently Asked Questions

### How does dela work?

`dela` uses your shell's command_not_found_handler to detect when you are trying to run a command that doesn't exist. It then scans the current working directory for task definition files and executes the appropriate task runner.

### What happens if a task shares the same name with a command?

Then the bare command will be executed instead of the task. To execute the task, you can use `dr <task_name>` to bypass the shadowed command but still make use of `dela`'s task runner disambiguation.

### How do I add a new task?

You can add a new task by adding a new task definition file. The task definition file can be a Makefile, a pyproject.toml, or a package.json.

### What shell environment are tasks executed in?

When executing bare tasks or via `dr`, tasks are executed in the current shell environment. When running tasks via `dela run`, tasks are executed in a subshell environment.

### Which shell integrations are supported?

Currently, `dela` supports zsh, bash, fish, and PowerShell.

### Which task runners are supported?

Currently, `dela` supports make, npm, uv, poetry, Maven, Gradle, and Github Actions.

### Which platforms are supported?

Currently, `dela` supports macOS and Linux.

### What is the purpose of allowlists?

Allowlist are a typo protection feature, and not for security. Since dela relies on
method missing functionality in your shell, typing a previously invalid command could
turn into executing something unintended, which is what allowlists mean to prevent.
When you download a repo from the internet and execute a task in it you need to be cognizant of its providence, just like you would with make or npm.

### Is dela production ready?

`dela` is not at 0.1 yet and its cli is subject to change.

## Development

For local development:

```sh
$ cargo install --path .
$ source resources/zsh.sh  # or equivalent for your shell
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
