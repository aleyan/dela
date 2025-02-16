# Dela!

Dela is a lightweight task runner that automatically discovers tasks in your current directory, let's you execute a task by its name without specifying the runner, and delegates their execution to tools like Make, npm, uv, and others.

## Installation

You can install `dela` from crates.io. The `dela init` command will add itself to your shell and create a `.dela` directory in your home directory.

```sh
$ cargo install dela
$ dela init
```

## Usage

### Discovering tasks
The `dela` command will list all the tasks defined.

```sh
$ dela list
```

### Running tasks
You can invoke a task just by its name. For example here `build` task is defined in `Makefile` and is invoked directly.

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

You can also call dela run directly with the task name, in which case it will execute the task directly, unless it is specified in multiple files.

```sh
$ dr build
```

## Frequently Asked Questions

### How does dela work?

`dela` uses your shell's command_not_found_handler to detect when you are trying to run a command that doesn't exist. It then scans the current working directory for task definition files and executes the appropriate task runner.

### What happens if a task shares the same name with a command?

Then the bare command will be executed instead of the task. To execute the task, you can use `dr <task_name>`.

### How do I add a new task?

You can add a new task by adding a new task definition file. The task definition file can be a Makefile, a pyproject.toml, or a package.json.

### What shell environment are tasks executed in?

Tasks are executed in the same shell environment as the command you are running.

### Which shell integrations are supported?

Currently, `dela` supports zsh, bash, fish, and PowerShell.

### Which task runners are supported?

Currently, `dela` supports Make, npm, uv, and poetry.

## Development

To use a dev version of the rust binary locally, build and install it with the following command.

```sh
cargo install --path .
```

You can also source the shell integration directly from the `resources` directory.

```sh
source resources/zsh.sh
```
