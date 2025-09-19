⸻

Note for humans. You can start the dev MCP server with Inspector like this (recommended: run the built binary directly to avoid `cargo run` arg parsing issues):
```sh
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
- MCP namespace uses a **separate read-only** file: `~/.dela/mcp_allowlist.toml`
- Human CLI continues to use `~/.dela/allowlist.toml`
- All execution tools (notably **task_start**) evaluate **only** the MCP allowlist with precedence:
  - **Deny > Directory > File > Task**
  - If no hit → **deny** with `NotAllowlisted`
- Maintenance occurs **outside** MCP (future `dela allow --mcp …` CLI).

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
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TaskDto {
  pub unique_name: String,      // e.g., "build-m"
  pub source_name: String,      // original name in file
  pub runner: String,           // short_name()
  pub command: String,          // fully-expanded shell command
  pub runner_available: bool,   // is the runner usable
  pub allowlisted: bool,        // allowlist decision (MCP namespace)
  pub file_path: String,        // absolute or repo-root relative string
  pub description: Option<String>,
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RunningTaskDto {
  pub pid: i32,
  pub unique_name: String,
  pub args: Vec<String>,
  pub started_at: String,       // RFC3339
  pub state: String,            // running|exited|stopped|failed (Phase 2 expands)
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct StartResultDto {
  pub state: String,            // exited|running|failed
  pub pid: Option<i32>,         // present if running
  pub exit_code: Option<i32>,   // present if exited/failed
  pub initial_output: String,   // combined stdout+stderr captured during first-second
}
```


⸻

## Tool Schemas (JSON)

### 1) list_tasks
**Args**
```json
{ "runner": null }
```
**Result**
```json
{ "tasks": [ TaskDto, ... ] }
```

### 2) status
**Args**
```json
{}
```
**Result**
```json
{ "running": [ RunningTaskDto, ... ] }
```

### 3) task_start
**Args**
```json
{
  "unique_name": "dev-n",
  "args": ["--port","5173"],
  "env": { "NODE_ENV":"development" },
  "cwd": null
}
```
**Result**
```json
{ "ok": true, "result": StartResultDto }
```
**Denied example**
```json
{ "ok": false, "code": "NotAllowlisted", "hint": "Ask a human to grant MCP access." }
```

### 4) task_status
**Args**
```json
{ "unique_name": "dev-n" }
```
**Result**
```json
{ "running": [ RunningTaskDto, ... ] }
```

### 5) task_output
**Args**
```json
{ "pid": 12345, "last": 200 }
```
**Result**
```json
{ "lines": ["...", "..."], "truncated": false }
```
(Phase 2 may add `"from": 4096` for byte-cursor paging.)

### 6) task_stop
**Args**
```json
{ "pid": 12345, "grace_ms": 5000 }
```
**Result**
```json
{ "ok": true }
```

⸻

## Resources (Phase 2 / Optional)
- We can keep a conservative tool-only design. If needed for streaming or snapshots later:
  - `job://<pid>` → JSON status snapshot
  - `joblog://<pid>?from=<u64>` → chunked logs paging

⸻

## Server layout (skeleton)

// src/mcp/server.rs
use rmcp::{tool, tool_router, ServerHandler, model::*, service::{RequestContext, RoleServer}};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{process::Command, io::{AsyncBufReadExt, BufReader}};
use tokio::sync::{Mutex, RwLock};
use crate::{task_discovery, runner, allowlist, types};

#[derive(Clone)]
pub struct DelaMcpServer {
  root: PathBuf,
  jobs: Arc<RwLock<HashMap<i32, Job>>>, // PID → Job { child_handle, ring_buffer, started_at, ... }
}

#[tool_router]
impl DelaMcpServer {
  #[tool(description="List tasks")]
  pub async fn list_tasks(&self, Parameters(ListArgs{ runner }): Parameters<ListArgs>)
    -> Result<CallToolResult, ErrorData> {
    let d = task_discovery::discover_tasks(&self.root);
    let mut tasks = d.tasks;
    if let Some(r) = runner { tasks.retain(|t| t.runner.short_name()==r); }
    // compute command, runner_available, allowlisted
    let dtos: Vec<TaskDto> = tasks.iter().map(TaskDto::from_task_enriched(/* runner_available, allowlisted */)).collect();
    Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({ "tasks": dtos }))]))
  }

  #[tool(description="List all running tasks with PIDs")]
  pub async fn status(&self) -> Result<CallToolResult, ErrorData> {
    let running = self.snapshot_running().await; // Vec<RunningTaskDto>
    Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({ "running": running }))]))
  }

  #[tool(description="Start a task (≤1s capture, then background)")]
  pub async fn task_start(&self, Parameters(StartArgs{ unique_name, args, env, cwd }): Parameters<StartArgs>)
    -> Result<CallToolResult, ErrorData> {
    self.ensure_allowlisted(&unique_name, &cwd, &args, &env)?; // NotAllowlisted error otherwise
    let result = self.start_with_first_second_capture(unique_name, args.unwrap_or_default(), env, cwd).await?;
    Ok(CallToolResult::success(vec![Content::json(&serde_json::to_value(result).unwrap())]))
  }

  #[tool(description="Status for a single unique_name (may have multiple PIDs)")]
  pub async fn task_status(&self, Parameters(TaskStatusArgs{ unique_name }): Parameters<TaskStatusArgs>)
    -> Result<CallToolResult, ErrorData> {
    let running = self.snapshot_running_by_unique_name(&unique_name).await;
    Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({ "running": running }))]))
  }

  #[tool(description="Tail last N lines for a PID")]
  pub async fn task_output(&self, Parameters(TaskOutputArgs{ pid, last }): Parameters<TaskOutputArgs>)
    -> Result<CallToolResult, ErrorData> {
    let (lines, truncated) = self.tail_output(pid, last.unwrap_or(200)).await?;
    Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({ "lines": lines, "truncated": truncated }))]))
  }

  #[tool(description="Stop a PID with graceful timeout")]
  pub async fn task_stop(&self, Parameters(TaskStopArgs{ pid, grace_ms }): Parameters<TaskStopArgs>)
    -> Result<CallToolResult, ErrorData> {
    self.stop_job(pid, grace_ms.unwrap_or(5000)).await?;
    Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({ "ok": true }))]))
  }
}

impl ServerHandler for DelaMcpServer {
  fn get_info(&self) -> ServerInfo {
    ServerInfo {
      protocol_version: ProtocolVersion::V_2024_11_05,
      capabilities: ServerCapabilities::builder().enable_tools().enable_logging().build(),
      server_info: Implementation { name: "dela-mcp".into(), version: env!("CARGO_PKG_VERSION").into() },
      instructions: Some("List tasks, start them (≤1s capture then background), and manage running tasks via PID; all execution gated by an MCP allowlist.".into()),
    }
  }
  // (Phase 2) implement read_resource for job:// and joblog:// if we enable resources
}

## Job runner internals
- Spawn via `tokio::process::Command` with stdin closed; capture stdout/stderr.
- First-second capture using `tokio::time::timeout` around a pump loop.
- Maintain per-PID ring buffer (VecDeque) (Phase 2) and minimal metadata (started_at, command, unique_name, args).
- Update state on child exit via join handle; GC old entries (Phase 2).

⸻

## Security & Limits
- Deny by default if not explicitly allowlisted in the MCP file.
- Respect `.dela` path only under real user `$HOME`.
- Output limits: ring buffer size cap; per-message chunk max (e.g., 8 KB). (Phase 2)
- Concurrency: cap max running PIDs (config), reject beyond limit. (Phase 2)
- Path policy: Tasks execute with `cwd` under server root (no upward traversal).

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
