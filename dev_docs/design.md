# Technical Design

This document provides a detailed technical design for `dela`, a task runner implemented in Rust. It builds on the concepts described in `architecture.md` and expands on how the codebase will be organized, the commands that will be supported, and how tasks will be discovered, executed, and secured through allowlists.

---

## Overview

`dela` is a single-binary Rust application that integrates with a user's shell to intercept "command not found" events. It searches for defined tasks in the current directory and delegates their execution to known task runner programs (e.g., `make`, `npm`, `python`, etc.). The tool also maintains allowlists to control which tasks/files/directories are permitted to run in the user's environment.

---

## Code Organization

The codebase follows a modular structure while avoiding excessive nesting. The main functionality is organized into logical modules with a focus on maintainability:

```
src/
├── main.rs                     // Entry point that parses CLI arguments and dispatches commands
├── commands/                   // Subcommands implementation
│   ├── allow_command.rs        // Command to manage allowlists
│   ├── configure_shell.rs      // Shell configuration command
│   ├── get_command.rs          // Command to retrieve task info
│   ├── init.rs                 // Command to initialize dela
│   ├── list.rs                 // Command to list tasks
│   └── mod.rs                  // Module exports
├── parsers/                    // Parsers for different task file formats
│   ├── parse_github_actions.rs // Parse GitHub Actions workflow files
│   ├── parse_gradle.rs         // Parse Gradle build files
│   ├── parse_makefile.rs       // Parse Makefiles
│   ├── parse_package_json.rs   // Parse package.json for npm scripts
│   ├── parse_pom_xml.rs        // Parse Maven pom.xml files
│   ├── parse_pyproject_toml.rs // Parse Python project files
│   ├── parse_taskfile.rs       // Parse Taskfile.yml
│   └── mod.rs                  // Module exports
├── runners/                    // Task runner implementations
│   ├── runners_package_json.rs // Runners for npm/yarn/etc.
│   ├── runners_pyproject_toml.rs // Runners for Python tools
│   └── mod.rs                  // Module exports
├── allowlist.rs                // Handling of allowlists, reading/writing from .dela/allowlist.toml
├── builtins.rs                 // Handling of shell builtin commands
├── prompt.rs                   // User interaction prompts
├── runner.rs                   // Core logic for running tasks
├── task_discovery.rs           // Logic for scanning directories, parsing tasks from recognized files
├── task_shadowing.rs           // Functions for detecting shadowed task names
└── types.rs                    // Common data structures (Task, TaskFile, etc.)
```

### `main.rs`
- The main entry point for the CLI application.
- Utilizes the `clap` crate for command-line argument parsing to decode user input into specific subcommands and options.
- Dispatches control to the corresponding logic in the commands modules.

### `commands/` directory
- Contains separate files for each subcommand implementation:
  1. **`init.rs`**: Configures the user's shell to call `dela` on "command not found".
  2. **`list.rs`**: Lists all tasks found in the current directory.
  3. **`get_command.rs`**: Retrieves information about a specific task for execution.
  4. **`allow_command.rs`**: Manages the allowlist entries.
  5. **`configure_shell.rs`**: Handles shell-specific configuration.

### `parsers/` directory
- Contains specialized parsers for different task file formats.
- Each parser is responsible for extracting tasks from a specific file format.
- Parsers implement fault tolerance where possible, with fallback mechanisms for handling non-standard formatting.

### `runners/` directory
- Contains implementations for different task runners.
- Handles the specific details of executing tasks with different tools (npm, yarn, Python, etc.).

### `allowlist.rs`
- Responsible for reading and writing user approval decisions to `~/.dela/allowlists`.
- Exposes APIs for checking and modifying task approvals.

### `task_discovery.rs`
- Scans the current directory for recognized task definition files.
- Coordinates the use of appropriate parsers for each file type.
- Aggregates discovered tasks and handles name collisions.

### `types.rs`
- Defines common structs, enums, or types used throughout the application.

## Core Types

The implementation uses the following core data structures:

```rust
pub enum TaskDefinitionType {
    Makefile,
    PackageJson,
    PyprojectToml,
    Taskfile,
    MavenPom,
    Gradle,
    GitHubActions,
    // ... potentially more types
}

pub enum TaskRunner {
    Make,
    NodeNpm,
    NodeYarn,
    NodePnpm,
    NodeBun,
    PythonPoetry,
    PythonUv,
    ShellScript,
    Maven,
    Gradle,
    GitHubActionsWorkflow,
    // ... potentially more runners
}

pub enum TaskFileStatus {
    Parsed,
    NotImplemented,
    ParseError(String),
    NotReadable(String),
    NotFound,
}

pub struct Task {
    pub name: String,
    pub file_path: PathBuf,
    pub definition_type: TaskDefinitionType,
    pub runner: TaskRunner,
    pub source_name: String,
    pub description: Option<String>,
    pub shadowed_by: Option<ShadowType>,
}

pub struct TaskDefinitionFile {
    pub path: PathBuf,
    pub definition_type: TaskDefinitionType,
    pub status: TaskFileStatus,
}

pub struct DiscoveredTasks {
    pub tasks: Vec<Task>,
    pub definitions: TaskDefinitions,
    pub errors: Vec<String>,
}

pub enum ShadowType {
    ShellBuiltin(String),
    PathExecutable(String),
    // ... potentially more types
}

pub enum AllowScope {
    Once,
    Task(String),
    File(PathBuf),
    Directory(PathBuf),
}
```

These types provide a comprehensive representation of tasks, their sources, and their execution requirements.

## Command-Line Interface

The following commands are planned for dela:
1) dela init
- Updates the user's shell configuration to set up dela as the fallback for "command not found".
- Creates ~/.dela directory (if missing) and an initial allowlist.toml.
2) dela list
- Prints out all discovered tasks in the current directory.
- Each task might be listed with its origin file.
- Example output:

```sh
$ dela list
build (make)
test (make)
start (npm)
```	

3) dela run <task>
- Bypasses the shell's fallback mechanism and directly invokes the discovered task.
- If multiple tasks with the same name exist in different files, prompts the user to disambiguate or choose from a menu.

4) dela allow <task>
- Manages allowlist entries for specific tasks.
- Used to pre-approve tasks or change existing approvals.

5) dela configure_shell
- Prints lines or shell functions that the user's rc file needs.
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
- The shell can't find a command named build, so it calls command_not_found_handle("build").
- dela finds build in the Makefile. If it's the first time, dela prompts about allowlisting. The user says "Allow any command from this Makefile."
- dela executes make build.

### Security & Allowlist Management
- Prompt: On the first run of any new command from a file or directory, dela asks if the user wants to allow just one-time execution, allow that specific task, allow all tasks from that file, allow all tasks in the directory, or deny.
- Storage: The user choice is written into a TOML-based allowlist in ~/.dela/allowlist.toml.
- Checks: Each time a task is run, dela consults the allowlist to ensure permission.
- Example: If the user chooses to allow a single run, the entry is ephemeral and not stored permanently. If the user chooses to "Allow any command from ~/Projects/dela/Makefile," a matching entry is created in the allowlist.

## Testing

The project has comprehensive test coverage through unit tests and integration tests.

### Unit Tests

Each module has its own unit tests to verify its functionality. For example:

1. **Parser Tests**:
   - Tests for parsing standard format files
   - Tests for handling malformed or non-standard files
   - Tests for fallback parsing mechanisms
   - Tests for edge cases (empty files, pattern rules, etc.)

2. **Command Tests**:
   - Verify each command produces the expected output
   - Test error handling and edge cases
   - Mock filesystem interactions where appropriate

3. **Allowlist Tests**:
   - Test allowlist file reading and writing
   - Verify allowlist validation logic
   - Test permission checking across different scopes

Each module has tests covering normal operation, edge cases, and error handling to ensure robust functionality.

### Integration Tests with Docker

The project requires testing complex interactions between shell integration and Rust code. We use Docker to provide isolated, reproducible test environments.

#### Current Implementation
1. **Test Infrastructure**
   - Docker-based test environment using Debian Bookworm
   - Multi-stage builds to optimize image size
   - Shell scripts embedded in binary at compile time
   - Makefile integration via `test_integration` target

2. **Test Coverage**
   - Environment verification
   - Shell integration installation
   - Command not found handler
   - Task discovery and execution
   - Direct task invocation

3. **Test Execution**
   - Quiet mode (default): `make test_integration`
   - Verbose mode: `VERBOSE=1 make test_integration`
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

This design outlines a flexible Rust-based task runner with a modular approach to shell integration, task discovery, and allowlist management. Key modules in the parsers, runners, and commands directories interoperate to seamlessly intercept unrecognized commands and map them to tasks. By prioritizing security, extensibility, and ease of configuration, dela aims to enhance developer productivity for a variety of project structures and ecosystems.