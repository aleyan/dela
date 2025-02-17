# Architecture

This document describes the overall architecture of `dela`—a task runner that automatically delegates tasks to the appropriate definitions across different file types, such as Makefiles, shell scripts, package managers, and more.

## Overview

At a high level, `dela` operates by intercepting commands in your shell that would normally result in a "command not found" error. Instead, `dela` checks to see if the command corresponds to a task in the current directory, and then executes it accordingly. The following sections describe the foundational architecture enabling this functionality.

---

## Installation & Shell Setup

1. **Shell Integration**
   - `dela init` modifies the user’s shell configuration (e.g., `.zshrc`, `.bashrc`, etc.) by adding the logic to handle “command not found” events.
   - The logic typically appends or modifies the shell’s `command_not_found_handle` function (or equivalent) so that any unrecognized command is forwarded to the `dela` executable.
   -  The shell configuration will `eval(dela configure_shell)` to configure the shell. This is done so that new versions of dela can be installed without having to re-run the init command.

2. **User Home Directory**
   - When `dela init` is first run, it creates a `~/.dela` directory to store configuration details, including the allowlists.

3. **Cross-Shell Compatibility**
   - While `.zshrc` is our primary target, the installation logic may be extended to handle `~/.bashrc`, `~/.config/fish/config.fish`, or other shell init files if detected or requested.

---

## Task Discovery and Parsing

1. **Supported File Types**
   - `dela` recognizes tasks in Makefiles (`make` commands), shell scripts (direct executables), Python (`pyproject.toml` scripts), Node (`package.json` scripts), and more. It can be extended to handle further file types by implementing additional parsers.

2. **Search Strategy**
   - Upon detecting an unrecognized command, `dela` searches the current working directory (and potentially subdirectories) for known task definition files.
   - Each file type has a parser that extracts a list of defined tasks (e.g., target names from a Makefile, script entries from a package.json, etc.).

3. **Task Mapping**
   - The discovered tasks are stored in memory. If a requested task matches multiple definition files, the user is prompted to choose which file or which specific task definition to run.

---

## Execution Model

1. **Shell Invocation**
   - Once a matching task is found, `dela` instructs the shell to run the appropriate command. For example, if the task is in a Makefile, it runs `make <task>`. For a Node-based task, it might run `npm run <task>`, and so on.

2. **Bare vs. `dr`**
   - Users can invoke tasks by calling the bare command without a tool eg (`build`), which triggers the "command not found" handler. Or they can use `dr build`, which bypasses the shell's "command not found" mechanism and directly invokes the runner logic.

3. **Shell Execution Strategy**
   - Rather than executing commands directly from Rust, `dela` returns commands to the shell for execution.
   - This is implemented through shell function wrappers:
   ```zsh
   dela() {
       if [[ $1 == "run" ]]; then
           cmd=$(dela get_command "${@:2}")
           eval "$cmd"
       else
           command dela "$@"
       fi
   }
   ```
   - Benefits:
     - Commands execute in the actual shell environment
     - Shell builtins (cd, source, etc.) work correctly
     - Environment modifications persist
     - Shell aliases and functions are available

4. **Extensibility**
   - The architecture supports adding new task definition modules for various technologies. Each module implements a function to detect tasks and a strategy to execute them. These extensions are simply additional rust files in parsers directory.

---

## Allowlist Management & Security

1. **Allowlists**
   - `dela` tracks user-approved tasks or task definition files in `~/.dela/allowlists`. This ensures that tasks cannot execute code from an untrusted directory or file without explicit permission when a user typos a command.

2. **Prompting**
   - When a task is run for the first time from a new directory or new file, the user is prompted to allow or deny that command. This approach helps prevent accidental or malicious commands from executing automatically.

3. **Scoping**
   - The user can allow just a single run, allow all tasks from a file, or allow any command from an entire directory. Each choice is recorded in the respective allowlist configuration file for future sessions.

---

## Implementation Details

1. **Rust-Based CLI**
   - `dela` is written in Rust to maximize portability and performance. It uses libraries for command-line argument parsing and for orchestrating shell calls (e.g. `std::process::Command`).

2. **Storage & Configuration**
   - All user-specific data (allowlists, configuration, logs, etc.) are stored in the `~/.dela` folder by default.
   - A minimal in-memory store is built at runtime from these local files so that repeated tasks in the same session don’t require repeated file access.

3. **Error Handling & Logging**
   - If a task fails to execute or if a file is unreadable, `dela` provides user-friendly error messages.
   - Future improvements might include structured logging for better debugging and analytics.

---

## Future Enhancements

## Dockerized Testing for Shell Scripts and Shell Integration
While unit tests suffice for Rust logic, shell integration tests often require multiple real shells. A recommended approach is:
1. Create lightweight Docker images that contain different shells (e.g., zsh, bash, fish).
2. Copy (or mount) the `dela` binary and associated resources (like `zsh.sh`) into each container.
3. Run scenario-based tests that:
   - Initialize a fresh user environment (HOME, SHELL, etc.).
   - Execute `dela init`.
   - Source the updated shell configuration (e.g., `.zshrc`, `.bashrc`, or `config.fish`).
   - Confirm that tasks can be executed directly (bare command) and via `dela run <task>`.
4. Collect results for each shell to ensure cross-shell functionality remains consistent.

1. **Extending to More Task Runners**
   - Additional detection and execution for other popular build or scripting tools (Gradle, Maven, Rake, etc.).

2. **Plugin Architecture**
   - Third-party developers can create plugins for `dela` to support specialized or less common build tools. This might involve a well-defined interface for discovering tasks and executing them.

3. **Graphical Shell Completions**
   - Command auto-completion in Bash, Zsh, or Fish for discovered tasks, making it easier to see available tasks at a glance.

4. **Remote or Distributed Task Execution**
   - Potentially allow tasks to be executed in containers or remote servers for more advanced workflows.

---

## Conclusion

By combining shell integration, a modular parsing approach, and secure allowlists, `dela` provides a streamlined solution for discovering and running tasks in any directory. The Rust-based CLI foundation ensures easy installation, high performance, and the flexibility to expand into new ecosystems.