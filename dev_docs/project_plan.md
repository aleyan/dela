# Project Plan

This plan outlines the major development phases and tasks for building `dela`, a Rust-based task runner that automatically delegates commands to recognized task definition files. Each phase includes checklists that can be marked as completed in Markdown.

---

## Phase 1: Task Discovery & Parsing & CLI

- [ ] **CLI Scaffolding**
  - [x] [DTKT-1] Set up CLI argument parsing (e.g., `clap`).
  - [ ] **Basic Commands Implementation**
    - [x] [DTKT-56] Implement `init` command structure and help text
    - [x] [DTKT-57] Implement `configure_shell` command structure and help text
    - [x] [DTKT-9] Implement `list` command structure and help text
    - [x] [DTKT-58] Implement `run` command structure and help text
    - [x] [DTKT-59] Implement `get_command` command structure and help text
    - [x] [DTKT-60] Add command-line options for verbosity and debug output
    - [x] [DTKT-61] Add `--help` text for each subcommand
    - [x] [DTKT-62] Add `--version` flag to show version information

- [ ] **Task Definition Parsers**
  - [x] [DTKT-3] Create `task_discovery.rs`.
  - [x] [DTKT-4] Implement Makefile parser (using `makefile-lossless` or similar).
  - [x] [DTKT-5] Implement parser for `package.json` scripts (`package-json` or `package_json_schema`).
  - [x] [DTKT-6] Implement parser for `pyproject.toml` scripts (`pyproject-toml`).
  - [x] [DTKT-106] For `package.json`, detect if there is a lock file `pnpm` or  `npm` or `yarn` or `bun` use that to run tasks.
  - [x] [DTKT-104] Update makefile-lossless to new version supporting trailing text.

- [ ] **Structs and Runners**
  - [x] [DTKT-7] Define `Task` and `TaskRunner` enums in `types.rs`.
  - [x] [DTKT-8] Associate discovered tasks with appropriate runner.

- [ ] **CLI Implementation for `list`**
  - [x] [DTKT-9] Ensure `dela list` shows tasks from recognized files.
  - [x] [DTKT-10] Print tasks with references to the source file.
  - [x] [DTKT-92] List which task runner will be used for each task.
  - [x] [DTKT-113] Indicate which commands have duplicate names.
  - [x] [DTKT-116] Indicate when a command is missing a runner.

**Deliverables**
- [x] Parsing logic for multiple file types.
- [x] Working `dela list` displaying discovered tasks.
- [ ] Documentation on adding new parser modules.
---

## Phase 2: Shell Integration and Basic CLI

- [x] **Shell Integration**
  - [x] [DTKT-11] Modify `.zshrc` to invoke `dela` manually.
  - [x] [DTKT-12] Append/update `command_not_found_handle` manually.
  - [x] [DTKT-13] Implement `dela configure-shell` command to return the command_not_found_handle.
  - [x] [DTKT-14] Implement `dela init` command to automate creation of `~/.dela` and `~/.dela/allowlist.toml`.
  - [x] [DTKT-15] Modify `dela init` command add eval of `dela configure_shell`.
  - [ ] [DTKT-93] Have `dela init` take options options (eg no method missing)
  - [ ] [DTKT-105] Update `dela init` to cleanup the output.

- [x] **Shell Execution Strategy**
  - [x] [DTKT-75] Implement shell function wrapper for `dela run` command
  - [x] [DTKT-76] Implement `get-command` to return shell-executable command string
  - [x] [DTKT-77] Ensure commands execute in actual shell environment
  - [x] [DTKT-78] Indicate when tasks are shadowed by shell builtins (cd, source, etc.)
  - [x] [DTKT-79] Ensure environment modifications persist
  - [x] [DTKT-80] Make shell aliases and functions available to tasks
  - [x] [DTKT-117] Pass arguments after the task name to the task for base execution
  - [x] [DTKT-118] Pass arguments after the task name to the task for `dr` execution

- [ ] **Repository Preparation**
  - [x] [DTKT-16] Confirm Rust workspace structure is stable.
  - [x] [DTKT-17] Ensure code compiles and installs via `cargo install dela`.

**Deliverables**
- [x] Shell fallback for unrecognized commands.
- [x] Working `dela init`.
- [x] Shell-aware task execution environment.

---

## Phase 3: Task Execution & Workflow

- [x] **Task Execution Logic**
  - [x] [DTKT-18] Implement `task_execution.rs` to invoke tasks (e.g., `make <target>`, `npm run <script>`).
  - [x] [DTKT-94] Pass arguments after the task name to the task
  - [x] [DTKT-19] Handle errors if required CLI tools are missing.
  - [x] [DTKT-20] Implement shell environment inheritance for task execution.
  - [x] [DTKT-21] Support both direct execution and subshell spawning based on task type.
  - [x] [DTKT-22] Ensure environment variables and working directory are properly propagated.
  - [x] [DTKT-87] Implement task runner installation detection
  - [x] [DTKT-88] Implement task runner disambiguation eg(npm vs yarn vs bun)
  - [ ] [DTKT-138] Support environments where Taskfile is invoked via `go-task` instead of `task`

- [ ] **`run` Command and Bare-Command Invocation**
  - [x] [DTKT-23] Complete `dela run <task>` for direct execution.
  - [x] [DTKT-24] Ensure bare commands invoke `dela` through the fallback.
  - [x] [DTKT-95] Provide `dr` shell function to run dela tasks with --allow flag.
  - [x] [DTKT-112] Remove `dela run` in favor of `dr` shell function.

- [ ] **Task Name Disambiguation**
  - [x] [DTKT-85] Design and implement disambiguation suffix generation
    - [x] Detect task name collisions across different runners
    - [x] Generate unique suffixes based on task runner initials (e.g., test-m, test-p)
    - [x] Handle multiple runners with same initial by adding more letters
  - [x] [DTKT-86] Update list command for disambiguated task display
    - [x] Mark ambiguous tasks with double vertical bar (‖)
    - [x] Show both original and suffixed task names
    - [x] Add a footnote section listing all duplicate task names
  - [ ] [DTKT-25] Implement TUI for ambiguous task selection
    - [ ] Create interactive menu when ambiguous task is executed without suffix
    - [ ] Allow selection between all matching tasks
  - [x] [DTKT-26] Support disambiguation in execution paths
    - [x] Handle suffixed tasks in bare execution mode
    - [x] Handle suffixed tasks in `dr` execution mode
    - [x] Pass arguments correctly to disambiguated tasks

**Deliverables**
- [x] Fully functional `dela run <task>`.
- [x] Automatic fallback from unrecognized commands.
- [x] Handling of multiple tasks with the same name.

---

## Phase 4: Security & Allowlist Management

- [x] **Allowlist Data Structures**
  - [x] [DTKT-28] Implement `allowlist.rs` to read/write `~/.dela/allowlist.toml`.
  - [x] [DTKT-29] Define `AllowlistEntry` with `file`/`directory` scopes.

- [x] **User Prompts**
  - [x] [DTKT-30] Prompt user on first invocation of task from new file/directory.
  - [x] [DTKT-31] Support "Allow once," "Allow this task," "Allow file," "Allow directory," and "Deny."
  - [x] [DTKT-32] Persist decisions in the allowlist.
  - [ ] [DTKT-33] Have `dela run` take an optional `--allow` flag to allow a task without prompting.
  - [ ] [DTKT-109] Implement `dela allow` command to add allowlist entries.
  - [ ] [DTKT-110] Implement `dela deny` command to add denylist entries.
  - [ ] [DTKT-111] Implement `dela run --allow-once` command to run a command once.

- [x] **Runtime Checks**
  - [x] [DTKT-96] Consult allowlist before executing tasks.
  - [x] [DTKT-34] If disallowed, prompt or block execution.
  - [x] [DTKT-97] Add native task execution when shell integration is not detected

**Deliverables**
- [ ] Secure allowlist solution.
- [x] Integrated prompting mechanism.
- [x] Repeated approvals handled gracefully.

---

## Phase 5: Expand shell capabilities to support bash, fish, and PowerShell

- [x] **Bash Support**
  - [x] [DTKT-35] Implement `dela configure_shell` for bash.
  - [x] [DTKT-36] Implement `dela init` for bash.

- [x] **Fish Support**
  - [x] [DTKT-37] Implement `dela configure_shell` for fish.
  - [x] [DTKT-38] Implement `dela init` for fish.

- [x] **PowerShell Support**
  - [x] [DTKT-89] Implement `dela configure_shell` for PowerShell.
  - [x] [DTKT-90] Implement `dela init` for PowerShell.
  - [x] [DTKT-91] Handle PowerShell-specific output formatting.

---

## Phase 6: Testing & Quality Assurance

- [ ] **Unit Tests**
  - [x] [DTKT-107] Run unit tests in CI
  - [x] [DTKT-39] Cover each module: `shell_integration`, `task_discovery`, `allowlist`, `task_execution`.
  - [x] [DTKT-124] When running attempting to run a bare task that doesn't exist. Don't print anything.
  - [x] [DTKT-125] When running a task via `dr`, do not attempt to execute errors.

- [ ] **Bug & Fixes**
  - [x] [DTKT-128] Don't list the same make task twice
  - [x] [DTKT-129] Make makefile-lossless work with ifneq endif and update it
  - [x] [DTKT-130] Github pages should not list out sub tasks.
  - [x] [DTKT-131] Command line arguments should be passed to tasks when passed 'bare'
  - [x] [DTKT-132] Command line arguments should be passed to tasks when passing via dr

### Dockerized Cross-Shell Testing
- [x] [DTKT-82] Build Docker images that contain multiple shells (zsh, bash, fish, PowerShell).
- [x] [DTKT-83] Automate a test workflow where each container:
  - Installs `dela`.
  - Runs `dela init`.
  - Sources the relevant shell configuration.
  - Validates that tasks can be run both via bare command (through command_not_found_handler) and with `dela run <task>`.
- [x] [DTKT-84] Integrate these Docker-based tests into CI to confirm cross-shell compatibility.

- [ ] **Integration Tests**
  - [x] [DTKT-40] Simulate user flows with different shells (Zsh, Bash, Fish).
  - [ ] [DTKT-41] Validate allowlist logic and parsing of different file types.
  - [x] [DTKT-108] Run integration tests in CI

- [ ] **Cross-Shell Checks**
  - [ ] [DTKT-42] Test on macOS and Linux to ensure consistent behavior.
  - [ ] [DTKT-43] Explore Windows/PowerShell feasibility.

**Deliverables**
- [ ] Comprehensive test coverage.
- [x] CI/CD pipeline to automate test runs.
- [x] Verified cross-shell compatibility.

---

## Phase 7: Documentation & Release

- [ ] **Documentation**
  - [ ] [DTKT-44] Update `README.md` with usage instructions and examples.
  - [ ] [DTKT-45] Provide short tutorials or usage demos.
  - [ ] [DTKT-46] Consider additional docs folder or GitHub Pages for extended guides.
  - [ ] [DTKT-114] Publish docs to read the docs.

- [ ] **Versioning and Release**
  - [ ] [DTKT-47] Bump version in `Cargo.toml`.
  - [ ] [DTKT-48] Publish to crates.io.
  - [ ] [DTKT-49] Tag a stable release in the repository.
  - [ ] [DTKT-115] Mark realeses on github.

- [ ] **Community Feedback**
  - [ ] [DTKT-50] Collect user feedback on command discovery and allowlist features.
  - [ ] [DTKT-51] Triage bug reports and feature requests.

**Deliverables**
- [ ] User-friendly documentation.
- [ ] Release package on crates.io.
- [ ] Announcement of new tool.

---

## Phase 8: Command-Line Experience

- [ ] **Command Output Formatting**
  - [x] [DTKT-63] Implement colored output for task status and errors
  - [ ] [DTKT-64] Add progress indicators for long-running tasks
  - [ ] [DTKT-65] Support machine-readable output format (e.g., JSON)
  - [ ] [DTKT-66] Add `--quiet` flag to suppress non-error output

- [ ] **Task Search and Discovery**
  - [ ] [DTKT-67] Add fuzzy matching for task names
  - [ ] [DTKT-68] Support searching tasks by description
  - [ ] [DTKT-69] Add `--filter` option to show only specific types of tasks
  - [ ] [DTKT-70] Support task name completion in shells
  - [ ] [DTKT-81] Implement zsh completion for dela commands and task names

- [ ] **Task Execution Control**
  - [ ] [DTKT-71] Add `--dry-run` flag to show what would be executed
  - [ ] [DTKT-72] Support task dependencies and ordering
  - [ ] [DTKT-73] Add timeout support for long-running tasks
  - [ ] [DTKT-74] Support cancellation of running tasks

**Deliverables**
- [ ] Rich command-line interface with colored output
- [ ] Task search and filtering capabilities
- [ ] Enhanced task execution control
- [ ] Shell completion support

## Phase 9: Increase Task Runner Coverage

- [ ] **Additional Task Runners support**
  - [x] [DTKT-119] Implement Maven `pom.xml` parser and task discovery
  - [x] [DTKT-120] Implement Gradle `build.gradle`/`build.gradle.kts` parser
  - [x] [DTKT-121] Parse GitHub Actions workflow files to expose jobs as tasks for `act`.
  - [x] [DTKT-126] Implement docker compose support

  - [x] [DTKT-134] Implement CMake `CMakeLists.txt` parser and task discovery
  - [x] [DTKT-135] Implement Travis CI `.travis.yml` parser and task discovery

## Icebox and Future Enhancements (Post-Launch)

- [ ] **Desirable**
  - [ ] [DTKT-131] Become an MCP server for the tasks
  - [ ] [DTKT-132] Shell completions

- [ ] **Task Runner Expansions**
  - [ ] [DTKT-127] Implement cargo build
  - [ ] [DTKT-133] Improve Gradle task disambiguation strategy (duplicates in same file)
  - [ ] [DTKT-136] Implement Turborepo support
  - [ ] [DTKT-136] Implement Rake support
  - [x] [DTKT-137] Implement Justfile parser and task discovery
  - [ ] [DTKT-122] Add Starlark parsing for Bazel
  - [ ] [DTKT-123] Implement Bazel task running.

- [ ] **Unlikely to Happen, deprioretized**
  - [ ] [DTKT-52] Provide a standardized interface for community-built task parsers.
  - [ ] [DTKT-53] Support containers or remote servers for distributed workloads.
  - [ ] [DTKT-54] Introduce optional `~/.dela/config.toml` for global settings.
  - [ ] [DTKT-55] Add more flexible user preferences.


## Timeline & Dependencies

- **Phase 1** (Task Discovery & Parsing & CLI) is foundational and should be completed first.
- **Phase 2** (Shell Integration and Basic CLI) can proceed in parallel with Phase 1.
- **Phase 3** (Task Execution & Workflow) depends on both Phase 1 and 2 being completed.
- **Phase 4** (Security & Allowlist Management) requires Phase 3's task execution to be functional.
- **Phase 5** (Expand shell capabilities) builds upon Phase 2's shell integration work.
- **Phase 6** (Testing & Quality Assurance) can begin after Phase 4, running in parallel with Phase 5.
- **Phase 7** (Documentation & Release) should commence after all other phases are substantially complete.

Mark these items `[x]` when completed to track progress. This checklist format facilitates easy status updates for individuals and teams working on different tasks.
