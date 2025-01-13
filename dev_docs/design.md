# Technical Design

This document provides a detailed technical design for `dela`, a task runner implemented in Rust. It builds on the concepts described in `architecture.md` and expands on how the codebase will be organized, the commands that will be supported, and how tasks will be discovered, executed, and secured through allowlists.

---

## Overview

`dela` is a single-binary Rust application that integrates with a user's shell to intercept "command not found" events. It searches for defined tasks in the current directory and delegates their execution to known task runner programs (e.g., `make`, `npm`, `python`, etc.). The tool also maintains allowlists to control which tasks/files/directories are permitted to run in the user's environment.

---

## Code Organization

We will maintain a relatively simple, flat file structure in the `src/` directory, guided by Rust’s recommended layout:

```
src/
├── main.rs          // Entry point that parses CLI arguments and dispatches commands
├── commands.rs      // Contains definitions and implementations for all CLI subcommands
├── shell_integration.rs // Logic for modifying .zshrc or equivalents, configuring shell hooks
├── allowlist.rs     // Handling of allowlists, reading/writing from .dela/allowlist.toml
├── task_discovery.rs // Logic for scanning directories, parsing tasks from recognized files
├── task_execution.rs // Functions responsible for executing tasks once discovered
└── types.rs         // Common data structures (Task, TaskFile, etc.)
```

### `main.rs`
- The main entry point for the CLI application.
- Utilizes a command-line argument parser (e.g., `clap` or `structopt`) to decode user input into specific subcommands and options.
- Dispatches control to the corresponding logic in `commands.rs`.

### `commands.rs`
- Defines functions for each of the subcommands:
	1. **`dela init`**: Configures the user’s shell to call `dela` on "command not found". This subcommand:
		- Updates `~/.zshrc`, `~/.bashrc`, or `~/.config/fish/config.fish` (as appropriate) to source `command_not_found_handle`.
		- Optionally runs `dela configure_shell` logic so that updates propagate automatically.
	2. **`dela list`**: Lists all tasks found in the current directory.
	3. **`dela run <task>`**: Directly executes a given task without going through the “command not found” mechanism.
	4. **`dela configure_shell`**: Prints shell function definitions for the user’s environment to use `dela` as the fallback for unrecognized commands. Typically invoked and eval'ed by shell configuration scripts.
	5. **(Optional) Additional Commands**: Additional commands for debugging, verbose logging, or plugin management could be placed here.

### `shell_integration.rs`
- Provides the low-level logic for writing lines to the appropriate shell rc files (e.g., `.zshrc`).
- Contains any OS detection or shell type detection code.
- Implements fallback for shells that may not support a function-based “command not found” handler.

### `allowlist.rs`
- Responsible for reading and writing user approval decisions to `~/.dela/allowlists`.
- Exposes APIs such as:
	- `check_task_approval(...) -> bool`
	- `add_approval_for_file(...)`
	- `add_approval_for_directory(...)`
- The allowlist data will be stored in a TOML file at `~/.dela/allowlist.toml`. A possible structure:

	```toml
	# ~/.dela/allowlist.toml
	
	[[allowed_files]]
	path = "/Users/alex/Projects/dela/Makefile"
	scope = "file"             # or "directory"

	[[allowed_tasks]]
	task_name = "build"
	file_path = "/Users/alex/Projects/dela/Makefile"

### `task_discovery.rs`
- Scans the current directory (and possibly subdirectories) for recognized task definition files (Makefile, package.json, pyproject.toml, etc.).
- Parses these files to extract tasks. For example:
	- Makefile: Use a small parser or call make -pn to list target names.
	- package.json: Load JSON and look in the "scripts" section.
	- pyproject.toml: Possibly look for [tool.poetry.scripts] or other relevant sections.
- Returns a collection of Task objects, each describing a discovered task name and associated runner info.

### `task_execution.rs`
- Implements logic for executing a discovered task using the appropriate command line (e.g., make <task>, npm run <task>, etc.).
- Contains error handling to provide user-friendly messages if something fails (e.g., missing tool, unrecognized task, insufficient permissions).

### `types.rs`
- Defines common structs, enums, or types:

```rust
pub struct Task {
	pub name: String,
	pub file_path: String,
	pub runner: TaskRunner, // e.g., Make, Npm, Python, etc.
	pub permission: TaskPermission,
}

pub enum TaskPermission {
	AllowTask,
	AllowTaskFile,
	AllowTaskDirectory,
	Deny,
	Prompt,
}

pub enum TaskRunner {
	Make,
	Npm,
	Python,
	ShellScript,
	// ...
}

pub struct AllowlistEntry {
	pub path: String,   // file or directory
	pub scope: String,  // "file", "directory", etc.
}
```

These types are shared throughout the modules for consistent representation of tasks and allowlist entries.


## Command-Line Interface

The following commands are planned for dela:
1) dela init
- Updates the user’s shell configuration to set up dela as the fallback for “command not found”.
- Creates ~/.dela directory (if missing) and an initial allowlist.toml.
2) dela list
- Prints out all discovered tasks in the current directory.
- Each task might be listed with its origin file.
- Example output:

```sh
$ dela list
build (Makefile)
test (Makefile)
start (package.json)
```	

3) dela run <task>
- Bypasses the shell’s fallback mechanism and directly invokes the discovered task.
- If multiple tasks with the same name exist in different files, prompts the user to disambiguate or choose from a menu.

4) dela configure_shell (sub-invocation)
- If not allowed, prompts the user with allowlist options.

5) dela configure_shell (sub-invocation)
- Prints lines or shell functions that the user’s rc file needs.
- Called internally by dela init or manually by advanced users.

### Typical Workflow
1) User runs cargo install dela.
2) User calls dela init, which:
- Creates ~/.dela if necessary.
- Writes or updates lines in .zshrc (or other shell config).
- Installs a minimal function for command_not_found_handle that delegates to dela.
3) User opens or reloads their shell session.
4) In a project directory with a Makefile containing build and test:
- User types build.
- The shell can’t find a command named build, so it calls command_not_found_handle("build").
- dela finds build in the Makefile. If it’s the first time, dela prompts about allowlisting. The user says “Allow any command from this Makefile.”
- dela executes make build.

### Security & Allowlist Management
- Prompt: On the first run of any new command from a file or directory, dela asks if the user wants to allow just one-time execution, allow that specific task, allow all tasks from that file, allow all tasks in the directory, or deny.
- Storage: The user choice is written into a TOML-based allowlist in ~/.dela/allowlist.toml.
- Checks: Each time a task is run, dela consults the allowlist to ensure permission.
- Example: If the user chooses to allow a single run, the entry is ephemeral and not stored permanently. If the user chooses to “Allow any command from ~/Projects/dela/Makefile,” a matching entry is created in the allowlist.

### Future Extensibility
- Plugin Architecture: Potential for adding new parsers or new ways to run tasks (Java-based, Docker-based, etc.).
- Cross-Platform: Although we primarily target macOS and Linux shells, we want minimal friction for Windows, possibly with PowerShell compatibility.
- Configuration Management: We might add a ~/.dela/config.toml for additional user settings like default scopes or plugin directories.
- Logging & Debugging: Provide verbose output with dela --verbose or RUST_LOG environment variable.

## Testing

Each rust module will have its own unit tests. Adding tests is a requirements for checking a DTKT as complete.

### Integration Tests with Docker
The project requires testing complex interactions between shell integration and Rust code. We use Docker to provide isolated, reproducible test environments.

#### Current Implementation
1. **Test Infrastructure**
   - Docker-based test environment using Debian Bookworm
   - Multi-stage builds to optimize image size
   - Shell scripts embedded in binary at compile time
   - Makefile integration via `test_shells` target

2. **Test Coverage**
   - Environment verification
   - Shell integration installation
   - Command not found handler
   - Task discovery and execution
   - Direct task invocation

3. **Test Execution**
   - Quiet mode (default): `make test_shells`
   - Verbose mode: `make test_shells VERBOSE=1`
   - Proper error reporting and test progress

#### Future Enhancements
1. **Additional Test Cases**
   - Task allowlist functionality
   - Multiple task definitions
   - Error cases and edge conditions
   - Shell environment persistence

2. **CI/CD Integration**
   - GitHub Actions workflow
   - Matrix testing across shell versions
   - Test result reporting
   - Coverage tracking

3. **Performance Optimization**
   - Parallel shell testing
   - Build caching improvements
   - Test suite organization

## Conclusion

This design outlines a flexible Rust-based task runner with a modular approach to shell integration, task discovery, and allowlist management. Key modules (shell_integration.rs, allowlist.rs, task_discovery.rs, task_execution.rs) interoperate to seamlessly intercept unrecognized commands and map them to tasks. By prioritizing security, extensibility, and ease of configuration, dela aims to enhance developer productivity for a variety of project structures and ecosystems.