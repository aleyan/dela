# Dela!

Dela is a lightweight task runner that provides discovery for task definitions in various formats, and lets you execute tasks without specifying the runner while delegating their execution to your existing tools like Make, npm, uv, and others.

## Installation

You can install `dela` from crates.io. The `dela init` command will add itself to your shell and create a `.dela` directory in your home directory.

```sh
$ cargo install dela
$ dela init
```

## Usage

### Discovering tasks
The `dela list` command will list all the tasks defined.

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

You can also call request dela explicitly with `dr <task>`.

```sh
$ dr build
```

If you don't have dela shell integration, you can use `dela run <task>` to run a task. This will execute the task in a subshell environment.

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

Currently, `dela` supports Make, npm, uv, poetry, Maven, Gradle, and Github Actions.

### Which platforms are supported?

Currently, `dela` supports macOS and Linux.

### Is dela production ready?

`dela` is not at 0.1 yet and its cli is subject to change.

## Development

To use a dev version of the rust binary locally, build and install it with the following command.

```sh
$ cargo install --path .
```

You can also source the shell integration directly from the `resources` directory.

```sh
$ source resources/zsh.sh
```

## Testing
Run integration tests with `dr test`, it requires `Make`, `cargo`, and `dela` to be installed.

```sh
$ tests
```

Run integrations test with `test_shells`, it requires `Make`, `Docker`, and `dela` to be installed.

```sh
$ tests_integration
```
