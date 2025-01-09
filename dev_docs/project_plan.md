# Project Plan

This plan outlines the major development phases and tasks for building `dela`, a Rust-based task runner that automatically delegates commands to recognized task definition files. Each phase includes checklists that can be marked as completed in Markdown.

---

## Phase 1: Task Discovery & Parsing & CLI

- [ ] **CLI Scaffolding**
  - [x] [DTKT-1] Set up CLI argument parsing (e.g., `clap`).
  - [ ] [DTKT-2] Implement basic subcommands: `init`, `configure_shell`, `list`, `run`, `get_command`.

- [ ] **Task Definition Parsers**
  - [ ] [DTKT-3] Create `task_discovery.rs`.
  - [ ] [DTKT-4] Implement Makefile parser (using `makefile-lossless` or similar).
  - [ ] [DTKT-5] Implement parser for `package.json` scripts (`package-json` or `package_json_schema`).
  - [ ] [DTKT-6] Implement parser for `pyproject.toml` scripts (`pyproject-toml`).

- [ ] **Structs and Runners**
  - [ ] [DTKT-7] Define `Task` and `TaskRunner` enums in `types.rs`.
  - [ ] [DTKT-8] Associate discovered tasks with appropriate runner.

- [ ] **CLI Implementation for `list`**
  - [ ] [DTKT-9] Ensure `dela list` shows tasks from recognized files.
  - [ ] [DTKT-10] Print tasks with references to the source file.

**Deliverables**
- [ ] Parsing logic for multiple file types.
- [ ] Working `dela list` displaying discovered tasks.
- [ ] Documentation on adding new parser modules.
---

## Phase 2: Shell Integration and Basic CLI

- [ ] **Shell Integration**
  - [ ] [DTKT-11] Modify `.zshrc` to invoke `dela` manually.
  - [ ] [DTKT-12] Append/update `command_not_found_handle` manually.
  - [ ] [DTKT-13] Implement `dela configure_shell` command to return the command_not_found_handle.
  - [ ] [DTKT-14] Implement `dela init` command to automate creation of `~/.dela`.
  - [ ] [DTKT-15] Modify `dela init` command add eval of command_not_found_handle.

- [ ] **Repository Preparation**
  - [ ] [DTKT-16] Confirm Rust workspace structure is stable.
  - [ ] [DTKT-17] Ensure code compiles and installs via `cargo install dela`.

**Deliverables**
- [ ] Shell fallback for unrecognized commands.
- [ ] Working `dela init`.
- [ ] Placeholder implementations of `dela list` and `dela run`.

---

## Phase 3: Task Execution & Workflow

- [ ] **Task Execution Logic**
  - [ ] [DTKT-18] Implement `task_execution.rs` to invoke tasks (e.g., `make <target>`, `npm run <script>`).
  - [ ] [DTKT-19] Handle errors if required CLI tools are missing.
  - [ ] [DTKT-20] Implement shell environment inheritance for task execution.
  - [ ] [DTKT-21] Support both direct execution and subshell spawning based on task type.
  - [ ] [DTKT-22] Ensure environment variables and working directory are properly propagated.

- [ ] **`run` Command and Bare-Command Invocation**
  - [ ] [DTKT-23] Complete `dela run <task>` for direct execution.
  - [ ] [DTKT-24] Ensure bare commands invoke `dela` through the fallback.
  - [ ] [DTKT-25] Prompt user if multiple matching tasks exist.

- [ ] **Disambiguation**
  - [ ] [DTKT-26] Implement logic to handle multiple tasks with the same name.
  - [ ] [DTKT-27] Store or remember user's choice, if desired.

**Deliverables**
- [ ] Fully functional `dela run <task>`.
- [ ] Automatic fallback from unrecognized commands.
- [ ] Handling of multiple tasks with the same name.

---

## Phase 4: Security & Allowlist Management

- [ ] **Allowlist Data Structures**
  - [ ] [DTKT-28] Implement `allowlist.rs` to read/write `~/.dela/allowlist.toml`.
  - [ ] [DTKT-29] Define `AllowlistEntry` with `file`/`directory` scopes.

- [ ] **User Prompts**
  - [ ] [DTKT-30] Prompt user on first invocation of task from new file/directory.
  - [ ] [DTKT-31] Support "Allow once," "Allow this task," "Allow file," "Allow directory," and "Deny."
  - [ ] [DTKT-32] Persist decisions in the allowlist.

- [ ] **Runtime Checks**
  - [ ] [DTKT-33] Consult allowlist before executing tasks.
  - [ ] [DTKT-34] If disallowed, prompt or block execution.

**Deliverables**
- [ ] Secure allowlist solution.
- [ ] Integrated prompting mechanism.
- [ ] Repeated approvals handled gracefully.

---

## Phase 5: Expand shell capabilities to support bash and fish

- [ ] **Bash Support**
  - [ ] [DTKT-35] Implement `dela configure_shell` for bash.
  - [ ] [DTKT-36] Implement `dela init` for bash.

- [ ] **Fish Support**
  - [ ] [DTKT-37] Implement `dela configure_shell` for fish.
  - [ ] [DTKT-38] Implement `dela init` for fish.

---

## Phase 6: Testing & Quality Assurance

- [ ] **Unit Tests**
  - [ ] [DTKT-39] Cover each module: `shell_integration`, `task_discovery`, `allowlist`, `task_execution`.

- [ ] **Integration Tests**
  - [ ] [DTKT-40] Simulate user flows with different shells (Zsh, Bash, Fish).
  - [ ] [DTKT-41] Validate allowlist logic and parsing of different file types.

- [ ] **Cross-Shell Checks**
  - [ ] [DTKT-42] Test on macOS and Linux to ensure consistent behavior.
  - [ ] [DTKT-43] Explore Windows/PowerShell feasibility.

**Deliverables**
- [ ] Comprehensive test coverage.
- [ ] CI/CD pipeline to automate test runs.
- [ ] Verified cross-shell compatibility.

---

## Phase 7: Documentation & Release

- [ ] **Documentation**
  - [ ] [DTKT-44] Update `README.md` with usage instructions and examples.
  - [ ] [DTKT-45] Provide short tutorials or usage demos.
  - [ ] [DTKT-46] Consider additional docs folder or GitHub Pages for extended guides.

- [ ] **Versioning and Release**
  - [ ] [DTKT-47] Bump version in `Cargo.toml`.
  - [ ] [DTKT-48] Publish to crates.io.
  - [ ] [DTKT-49] Tag a stable release in the repository.

- [ ] **Community Feedback**
  - [ ] [DTKT-50] Collect user feedback on command discovery and allowlist features.
  - [ ] [DTKT-51] Triage bug reports and feature requests.

**Deliverables**
- [ ] User-friendly documentation.
- [ ] Release package on crates.io.
- [ ] Announcement of new tool.

---

## Future Enhancements (Post-Launch)

- [ ] **Plugin Architecture**
  - [ ] [DTKT-52] Provide a standardized interface for community-built task parsers.

- [ ] **Remote Execution**
  - [ ] [DTKT-53] Support containers or remote servers for distributed workloads.

- [ ] **Advanced Configuration**
  - [ ] [DTKT-54] Introduce optional `~/.dela/config.toml` for global settings.
  - [ ] [DTKT-55] Add more flexible user preferences.

---

## Timeline & Dependencies

- **Phase 1** (Task Discovery & Parsing & CLI) is foundational and should be completed first.
- **Phase 2** (Shell Integration and Basic CLI) can proceed in parallel with Phase 1.
- **Phase 3** (Task Execution & Workflow) depends on both Phase 1 and 2 being completed.
- **Phase 4** (Security & Allowlist Management) requires Phase 3's task execution to be functional.
- **Phase 5** (Expand shell capabilities) builds upon Phase 2's shell integration work.
- **Phase 6** (Testing & Quality Assurance) can begin after Phase 4, running in parallel with Phase 5.
- **Phase 7** (Documentation & Release) should commence after all other phases are substantially complete.

Mark these items `[x]` when completed to track progress. This checklist format facilitates easy status updates for individuals and teams working on different tasks.