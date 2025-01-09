# Project Plan

This plan outlines the major development phases and tasks for building `dela`, a Rust-based task runner that automatically delegates commands to recognized task definition files. Each phase includes checklists that can be marked as completed in Markdown.

---

## Phase 1: Task Discovery & Parsing & CLI

- [ ] **CLI Scaffolding**
  - [ ] Set up CLI argument parsing (e.g., `clap`).
  - [ ] Implement basic subcommands: `init`, `configure_shell`, `list`, `run`.

- [ ] **Task Definition Parsers**
  - [ ] Create `task_discovery.rs`.
  - [ ] Implement Makefile parser (using `makefile-lossless` or similar).
  - [ ] Implement parser for `package.json` scripts (`package-json` or `package_json_schema`).
  - [ ] Implement parser for `pyproject.toml` scripts (`pyproject-toml`).

- [ ] **Structs and Runners**
  - [ ] Define `Task` and `TaskRunner` enums in `types.rs`.
  - [ ] Associate discovered tasks with appropriate runner.

- [ ] **CLI Implementation for `list`**
  - [ ] Ensure `dela list` shows tasks from recognized files.
  - [ ] Print tasks with references to the source file.

**Deliverables**
- [ ] Parsing logic for multiple file types.
- [ ] Working `dela list` displaying discovered tasks.
- [ ] Documentation on adding new parser modules.
---

## Phase 2: Shell Integration and Basic CLI

- [ ] **Shell Integration**
  - [ ] Modify `.zshrc` to invoke `dela` manually.
  - [ ] Append/update `command_not_found_handle` manually.
  - [ ] Implement `dela configure_shell` command to return the command_not_found_handle.
  - [ ] Implement `dela init` command to automate creation of `~/.dela`.
  - [ ] Modify `dela init` command add eval of command_not_found_handle.


- [ ] **Repository Preparation**
  - [ ] Confirm Rust workspace structure is stable.
  - [ ] Ensure code compiles and installs via `cargo install dela`.

**Deliverables**
- [ ] Shell fallback for unrecognized commands.
- [ ] Working `dela init`.
- [ ] Placeholder implementations of `dela list` and `dela run`.

---



---

## Phase 3: Task Execution & Workflow

- [ ] **Task Execution Logic**
  - [ ] Implement `task_execution.rs` to invoke tasks (e.g., `make <target>`, `npm run <script>`).
  - [ ] Handle errors if required CLI tools are missing.
  - [ ] Implement shell environment inheritance for task execution.
  - [ ] Support both direct execution and subshell spawning based on task type.
  - [ ] Ensure environment variables and working directory are properly propagated.

- [ ] **`run` Command and Bare-Command Invocation**
  - [ ] Complete `dela run <task>` for direct execution.
  - [ ] Ensure bare commands invoke `dela` through the fallback.
  - [ ] Prompt user if multiple matching tasks exist.

- [ ] **Disambiguation**
  - [ ] Implement logic to handle multiple tasks with the same name.
  - [ ] Store or remember user’s choice, if desired.

**Deliverables**
- [ ] Fully functional `dela run <task>`.
- [ ] Automatic fallback from unrecognized commands.
- [ ] Handling of multiple tasks with the same name.

---

## Phase 4: Security & Allowlist Management

- [ ] **Allowlist Data Structures**
  - [ ] Implement `allowlist.rs` to read/write `~/.dela/allowlist.toml`.
  - [ ] Define `AllowlistEntry` with `file`/`directory` scopes.

- [ ] **User Prompts**
  - [ ] Prompt user on first invocation of task from new file/directory.
  - [ ] Support “Allow once,” “Allow this task,” “Allow file,” “Allow directory,” and “Deny.”
  - [ ] Persist decisions in the allowlist.

- [ ] **Runtime Checks**
  - [ ] Consult allowlist before executing tasks.
  - [ ] If disallowed, prompt or block execution.

**Deliverables**
- [ ] Secure allowlist solution.
- [ ] Integrated prompting mechanism.
- [ ] Repeated approvals handled gracefully.

---

## Phase 5: Expand shell capabilities to support bash and fish

- [ ] **Bash Support**
  - [ ] Implement `dela configure_shell` for bash.
  - [ ] Implement `dela init` for bash.

- [ ] **Fish Support**
  - [ ] Implement `dela configure_shell` for fish.
  - [ ] Implement `dela init` for fish.

---

## Phase 6: Testing & Quality Assurance

- [ ] **Unit Tests**
  - [ ] Cover each module: `shell_integration`, `task_discovery`, `allowlist`, `task_execution`.

- [ ] **Integration Tests**
  - [ ] Simulate user flows with different shells (Zsh, Bash, Fish).
  - [ ] Validate allowlist logic and parsing of different file types.

- [ ] **Cross-Shell Checks**
  - [ ] Test on macOS and Linux to ensure consistent behavior.
  - [ ] Explore Windows/PowerShell feasibility.

**Deliverables**
- [ ] Comprehensive test coverage.
- [ ] CI/CD pipeline to automate test runs.
- [ ] Verified cross-shell compatibility.

---

## Phase 7: Documentation & Release

- [ ] **Documentation**
  - [ ] Update `README.md` with usage instructions and examples.
  - [ ] Provide short tutorials or usage demos.
  - [ ] Consider additional docs folder or GitHub Pages for extended guides.

- [ ] **Versioning and Release**
  - [ ] Bump version in `Cargo.toml`.
  - [ ] Publish to crates.io.
  - [ ] Tag a stable release in the repository.

- [ ] **Community Feedback**
  - [ ] Collect user feedback on command discovery and allowlist features.
  - [ ] Triage bug reports and feature requests.

**Deliverables**
- [ ] User-friendly documentation.
- [ ] Release package on crates.io.
- [ ] Announcement of new tool.

---

## Future Enhancements (Post-Launch)

- [ ] **Plugin Architecture**
  - [ ] Provide a standardized interface for community-built task parsers.

- [ ] **Remote Execution**
  - [ ] Support containers or remote servers for distributed workloads.

- [ ] **Advanced Configuration**
  - [ ] Introduce optional `~/.dela/config.toml` for global settings.
  - [ ] Add more flexible user preferences.

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