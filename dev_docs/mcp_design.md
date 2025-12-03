⸻

Note for humans. You can start the dev MCP server with Inspector like this (recommended: run the built binary directly to avoid `cargo run` arg parsing issues):
```sh
# oen shot
MCPI_NO_COLOR=1 npx @modelcontextprotocol/inspector -- cargo -q run -- mcp


# Debug build (recommended while iterating)
cargo build --quiet
MCPI_NO_COLOR=1 RUST_LOG=warn npx @modelcontextprotocol/inspector ./target/debug/dela mcp

# Or release build
cargo build --release --quiet
MCPI_NO_COLOR=1 RUST_LOG=warn npx @modelcontextprotocol/inspector ./target/release/dela mcp
```

**Protocol hygiene:** never print to **stdout** before/while serving MCP over stdio.
All human/debug output must go to **stderr**. The server should enter
`DelaMcpServer::serve_stdio()` and block until shutdown so the Inspector can establish the SSE session.
⸻

# Dela MCP — Revised Design (First-Principles)

## Scope (tool surface)

This redesign narrows each tool to a single, clear responsibility and aligns with how editors/agents actually consume MCP. All names are stable and self-descriptive.

**Tools**
- **list_tasks** → Return a list of tasks by their **unique** names (e.g., `build-m`, `build-g`). For each task include:
  - unique_name name that will be used to refer to this task definition everywhere.
  - source_name name that used in the source file
  - command (fully expanded shell/language command that would be executed)
  - runner (short name)
  - runner_available (bool) — is the runner binary usable now?
  - allowlisted (bool) — based on MCP allowlist policy
  - source_path (string) - filepath to task definitions
  - description (optional)
- **status** → Return a list of **all running tasks** (across all names) with PIDs and minimal status.
- **task_start** → Start a task by **unique_name** with optional args/env/cwd. If it **finishes within 1s**, return its full output and exit status. If it **does not finish in 1s**, background it, return `running` with PID and any output captured during that first second.
- **task_status** → Return status for **running instances of a given unique_name**. There may be multiple PIDs if the same task was started with different arguments.
- **task_output** → Return the **last N lines** of output for a **PID** (with a default N). Supports simple paging via an optional `from` byte cursor (future).
- **task_stop** → Stop a running task by **PID** (TERM with grace, then KILL on timeout).
- 
⸻

Library & Transport
	•	Library: rmcp (stdio transport)
	•	Runtime: tokio multi-thread
	•	Capabilities: tools (+ logging later for streaming notifications)

Add (dev):

cargo add rmcp tokio --features tokio/full
cargo add serde serde_json --features derive
cargo add schemars
cargo add tracing tracing-subscriber

Libraries and their roles:
	•	rmcp - The core MCP protocol library. Handles stdio transport, tool routing, and logging notifications. We use it to define and expose our five tools via #[tool] attributes.
	•	tokio - Async runtime for Rust. Powers our job management with async process spawning, ring buffers, and graceful shutdown. The 'full' feature gives us process management and IO utilities.
	•	serde/serde_json - Serialization framework for Rust. Handles all JSON encoding/decoding of our DTOs and tool parameters. The 'derive' feature lets us auto-generate serializers.
	•	schemars - JSON Schema generation. Used with #[derive(JsonSchema)] to document our DTOs and tool parameters. Helps IDEs understand our protocol.
	•	tracing - Modern instrumentation framework. (Phase 2) Used to emit structured job output events that rmcp can surface as logging notifications.
	•	tracing-subscriber - (Phase 2) Formats events into rmcp-compatible notifications and handles log routing.

## Permissioning (Allowlist)
- MCP server uses the **same allowlist** as the human CLI: `~/.dela/allowlist.toml`
- All execution tools (notably **task_start**) evaluate the allowlist with precedence:
  - **Deny > Directory > File > Task**
  - If no hit → **deny** with `NotAllowlisted` error
- Maintenance occurs via the regular `dela allow` CLI commands
- Tasks must be explicitly allowlisted to be executed via MCP

⸻

## Process & Output Model

- **PID-centric**: Each started task is a real OS child process with a PID.
- **First-second capture**: `task_start` collects stdout/stderr for up to **1 second** (configurable later).
  - If the child exits in ≤1s → return `{ state: "exited", exit_code, output }`.
  - If still running after 1s → background it and return `{ state: "running", pid, initial_output }`.
- **Output ring buffer (Phase 2)**: Per-PID bounded buffer (e.g., 1–5 MB). `task_output` returns last N lines. Future paging via `from` byte cursor.
- **Lifecycle**: `task_stop` sends SIGTERM, waits grace (default 5s), then SIGKILL. Background jobs are GC'd after a TTL (configurable; Phase 2).

⸻

## Wire DTOs

We keep `types::Task` internal; map to stable DTOs that are editor-friendly.

```rust
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TaskDto {
  pub unique_name: String,      // e.g., "build-m", "test-n"
  pub source_name: String,      // original name in file
  pub runner: String,           // short_name() - "make", "npm", "gradle", etc.
  pub command: String,          // fully-expanded shell command
  pub runner_available: bool,   // is the runner usable on this system
  pub allowlisted: bool,        // allowlist decision (based on ~/.dela/allowlist.toml)
  pub file_path: String,        // absolute or repo-root relative string
  pub description: Option<String>, // task description if available
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ListTasksArgs {
  pub runner: Option<String>,   // optional filter by runner type
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TaskStartArgs {
  pub unique_name: String,      // the unique name of the task to start
  pub args: Option<Vec<String>>, // optional arguments to pass to the task
  pub env: Option<HashMap<String, String>>, // optional environment variables
  pub cwd: Option<String>,      // optional working directory
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct StartResultDto {
  pub state: String,            // "exited"|"running"|"failed"
  pub pid: Option<i32>,         // present if running
  pub exit_code: Option<i32>,   // present if exited/failed
  pub initial_output: String,   // combined stdout+stderr captured during first-second
}

// Note: RunningTaskDto, TaskStatusArgs, TaskOutputArgs, TaskStopArgs are Phase 2
// and not yet implemented in the current version.
```


⸻

## Tool Schemas (JSON)

### 1) list_tasks
**Args**
```json
{ "runner": "make" }
```
**Result**
```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"tasks\": [{\"unique_name\": \"build-m\", \"source_name\": \"build\", \"runner\": \"make\", \"command\": \"make build\", \"runner_available\": true, \"allowlisted\": true, \"file_path\": \"/project/Makefile\", \"description\": \"Build the project\"}]}"
    }
  ]
}
```

### 2) status
**Args**
```json
{}
```
**Result**
```json
{
  "content": [
    {
      "type": "text", 
      "text": "{\"running\": []}"
    }
  ]
}
```
*Note: Phase 10A returns empty array - background processes not yet supported*

### 3) task_start
**Args**
```json
{
  "unique_name": "test-task",
  "args": ["--verbose", "--debug"],
  "env": { "NODE_ENV": "development" },
  "cwd": "/path/to/project"
}
```
**Result (success)**
```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"state\": \"exited\", \"exit_code\": 0, \"initial_output\": \"Test task executed successfully\\n\"}"
    }
  ]
}
```
**Result (running)**
```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"state\": \"running\", \"pid\": 12345, \"initial_output\": \"Starting long-running task...\\n\"}"
    }
  ]
}
```
**Error (NotAllowlisted)**
```json
{
  "error": {
    "code": -32010,
    "message": "Task 'custom-exe' is not allowlisted for MCP execution",
    "data": "Ask a human to grant MCP access to this task"
  }
}
```
**Error (TaskNotFound)**
```json
{
  "error": {
    "code": -32012,
    "message": "Task 'nonexistent-task' not found",
    "data": "Use 'list_tasks' to see available tasks"
  }
}
```
**Error (RunnerUnavailable)**
```json
{
  "error": {
    "code": -32011,
    "message": "Runner 'make' is not available for task 'build'",
    "data": "Install make: brew install make (macOS) or apt-get install make (Ubuntu)"
  }
}
```

## Error Taxonomy

The MCP server uses a structured error system with specific error codes and helpful messages:

### Error Codes
- **-32010** `NOT_ALLOWLISTED` - Task is not allowlisted for MCP execution
- **-32011** `RUNNER_UNAVAILABLE` - Required task runner is not available on the system  
- **-32012** `TASK_NOT_FOUND` - Task with the given name was not found
- **-32603** `INTERNAL_ERROR` - Generic internal server error

### Error Structure
All errors follow the JSON-RPC 2.0 error format:
```json
{
  "error": {
    "code": -32010,
    "message": "Task 'custom-exe' is not allowlisted for MCP execution", 
    "data": "Ask a human to grant MCP access to this task"
  }
}
```

### Error Examples

**NotAllowlisted Error**
```json
{
  "error": {
    "code": -32010,
    "message": "Task 'deploy' is not allowlisted for MCP execution",
    "data": "Ask a human to grant MCP access to this task"
  }
}
```

**RunnerUnavailable Error**
```json
{
  "error": {
    "code": -32011,
    "message": "Runner 'make' is not available for task 'build'",
    "data": "Install make: brew install make (macOS) or apt-get install make (Ubuntu)"
  }
}
```

**TaskNotFound Error**
```json
{
  "error": {
    "code": -32012,
    "message": "Task 'nonexistent-task' not found",
    "data": "Use 'list_tasks' to see available tasks"
  }
}
```

**Internal Error**
```json
{
  "error": {
    "code": -32603,
    "message": "Failed to start process: No such file or directory",
    "data": "Check if the command and arguments are valid"
  }
}
```

### 4) task_status
**Args**
```json
{ "unique_name": "build-m" }
```
**Result**
```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"jobs\": [{\"pid\": 12345, \"unique_name\": \"build-m\", \"state\": \"running\", \"started_at\": 120}]}"
    }
  ]
}
```

### 5) task_output
**Args**
```json
{ "pid": 12345, "lines": 100, "show_truncation": true }
```
**Result**
```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"pid\": 12345, \"lines\": [\"Building...\", \"Compiling...\"], \"total_lines\": 2, \"total_bytes\": 25, \"truncated\": false, \"buffer_full\": false}"
    }
  ]
}
```

### 6) task_stop
**Args**
```json
{ "pid": 12345, "grace_period": 5 }
```
**Result**
```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"pid\": 12345, \"status\": \"graceful\", \"message\": \"Process stopped gracefully with exit code 0\", \"grace_period_used\": 5}"
    }
  ]
}
```

⸻

## Resources (Optional / Future)
- We can keep a conservative tool-only design. If needed for streaming or snapshots later:
  - `job://<pid>` → JSON status snapshot
  - `joblog://<pid>?from=<u64>` → chunked logs paging

⸻

## Server Implementation

The MCP server is implemented in `src/mcp/server.rs` with the following key components:

### Core Structure
```rust
pub struct DelaMcpServer {
    root: PathBuf,  // Working directory for task discovery
}

impl ServerHandler for DelaMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation { 
                name: "dela-mcp".into(), 
                version: env!("CARGO_PKG_VERSION").into() 
            },
            instructions: Some(
                "List tasks, start them (≤1s capture then background), and manage running tasks via PID; all execution gated by an MCP allowlist.".into()
            ),
        }
    }
}
```

### Implemented Tools

**list_tasks** - Lists all available tasks with enriched metadata
- Filters by runner type if specified
- Returns `TaskDto` objects with command, runner availability, and allowlist status
- Handles task disambiguation (e.g., "test-m", "test-n")

**status** - Returns all currently running background tasks
- Returns list of running jobs with PIDs, unique names, and metadata
- Updated in real-time as jobs start and complete

**task_start** - Starts a task with optional arguments, environment, and working directory
- Validates task exists and is allowlisted
- Checks runner availability
- Captures output for first second, then backgrounds if still running
- Returns `StartResultDto` with state, PID, exit code, and initial output

**task_status** - Returns status for running instances of a specific task
- Filters jobs by unique_name
- Returns all PIDs associated with that task name
- Includes state (running/exited/failed), started_at, command, and args

**task_output** - Returns the last N lines of output for a running task
- Default 200 lines, configurable via `lines` parameter
- Supports `show_truncation` flag to indicate if output was truncated
- Per-PID ring buffer (1000 lines, 5MB max)

**task_stop** - Stops a running task by PID
- Sends SIGTERM with configurable grace period (default 5s)
- Falls back to SIGKILL if process doesn't exit gracefully
- Returns stop status (graceful/killed/failed)

### Error Handling
- Uses structured error taxonomy with specific error codes
- Provides helpful error messages and resolution hints
- Follows JSON-RPC 2.0 error format

## Job Runner Internals
- Spawn via `tokio::process::Command` with stdin closed; capture stdout/stderr
- First-second capture using `tokio::time::timeout` around a pump loop
- Per-PID ring buffer (VecDeque) with configurable limits:
  - Max 1000 lines per job
  - Max 5MB output per job
- Job metadata includes: started_at, command, unique_name, source_name, args, cwd, file_path
- Background monitoring task updates job state on child exit
- Jobs transition through states: Running → Exited(exit_code) or Failed(reason)

⸻

## Security & Limits
- **Deny by default**: Tasks not explicitly allowlisted are rejected with `NotAllowlisted` error
- **Allowlist path**: Reads from `~/.dela/allowlist.toml` (same as CLI)
- **Output limits**:
  - Ring buffer: 1000 lines, 5MB max per job
  - Per-message chunk: 8KB max
- **Concurrency**: Max 50 concurrent jobs (configurable), rejects beyond limit
- **Path policy**: Tasks execute with `cwd` under server root (no upward traversal)

⸻

## Test checklist (AAA)
- **Arrange**: fixture repo with Makefile + package.json; include duplicate task names; simulate missing runner.
- **Act**: JSON calls to `list_tasks`; `task_start` fast-exit task (≤1s); `task_start` long runner (>1s) and then `status`/`task_status`/`task_output`; `task_stop`.
- **Assert**: correct allowlist denials; enriched TaskDto fields; ≤1s capture returns exit + output; long runner returns PID + initial output; output tailing works; graceful stop works.

⸻

## CLI entry

```
dela mcp [--cwd <dir>]
```
Starts stdio server in `<dir>` (default `.`).

⸻

This keeps the surface area tiny, denies write power to MCP, and provides a pragmatic PID-based control plane that works for both quick tasks and long-running workloads.
