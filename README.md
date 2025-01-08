# Dela!

A task runner that delegates the work to others. For your current working directory, it will scan the task definition files for tasks and allow you to invoke them directly from your shell.

## Installation

You can install `dela` from crates.io. The `dela init` command will add itself to your shell and create a `.dela` directory in your home directory.

```sh
cargo install dela
dela init
```

## Usage

The `dela` command will list all the tasks defined.

```sh
dela list
```

You can invoke a task just by its name. For example here `build` task is defined in `Makefile` and is invoked.

```sh
build
```

If you are running `dela` in a directory for the first time, it will ask you to put the task or the task definition file  or the directory itself on the allowed list. This is because you might want to run `dela` in non fully trusted directories and cause inadvertent execution.

```sh
> build
Running build from ~/Projects/dela/Makefile for the first time. Allow?
0) Allow one time
1) Allow build from ~/Projects/dela/Makefile
2) Allow any command from ~/Projects/dela/Makefile
3) Allow any command from ~/Projects/dela
4) Deny
```
