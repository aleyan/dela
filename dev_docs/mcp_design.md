Here’s a simplified MCP design summary for dela that sticks to your five tools, uses rmcp, and adds a clean way to handle long-running tasks.

⸻

Dela MCP (minimal) — Design Summary

Scope (only 5 tools)
	•	list_tasks(runner?) → returns tasks with uniqified names (e.g., build-m)
	•	get_task(name) → accepts uniqified names
	•	get_command(task, args[]) → accepts uniqified names
	•	run_task(op, …) → handles start / status / stop in one tool
	•	read_allowlist(namespace?) → read-only

No tool can modify permissions. The allowlist constrains MCP; humans manage it via CLI or editing files.

⸻

Library & Transport
	•	Library: rmcp (stdio transport)
	•	Runtime: tokio multi-thread
	•	Capabilities: tools + resources + logging (for streaming)

Add (dev):

cargo add rmcp tokio --features tokio/full
cargo add serde serde_json --features derive
cargo add schemars
cargo add tracing tracing-subscriber


⸻

Permissioning
	•	MCP namespace uses a separate read-only file: ~/.dela/mcp_allowlist.toml
	•	Human CLI continues to use ~/.dela/allowlist.toml
	•	run_task evaluates only the MCP allowlist:
	•	Deny > Directory > File > Task
	•	If no hit → deny with NotAllowlisted
	•	Maintenance is done outside MCP (e.g., a future dela allow --mcp … CLI).

⸻

Long-running tasks

Single tool run_task supports:
	•	op: "start" → spawns process, returns job_id
	•	op: "status" → returns state (queued|running|exited|failed|stopped) + exit code/time + bytes/logs cursor
	•	op: "stop" → sends signal (graceful, then kill after timeout)

Streaming:
	•	On start, if stream: true, server emits rmcp logging notifications with {job_id, chunk} as lines arrive.
	•	Also expose resources:
	•	job://<job_id> → JSON status snapshot
	•	joblog://<job_id>?from=<cursor> → returns next chunk + next cursor
	•	Output ring buffer (e.g., 1–5 MB) per job (configurable); older bytes are evicted.

Lifecycle:
	•	Orphan policy default detach with TTL (e.g., 30 min, configurable); server GC stops old jobs.
	•	stop sends SIGTERM, waits grace delay (e.g., 5s), then SIGKILL.

⸻

Data model (wire DTOs)

We keep types::Task internal; map to stable DTOs:

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct TaskDto {
  pub name: String,          // uniqified name (e.g., build-m)
  pub source_name: String,   // original name in file
  pub runner: String,        // short_name()
  pub file_path: String,
  pub description: Option<String>,
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct JobStatusDto {
  pub job_id: String,
  pub state: String,           // queued|running|exited|failed|stopped
  pub started_at: Option<String>,
  pub finished_at: Option<String>,
  pub exit_code: Option<i32>,
  pub bytes_emitted: u64,
  pub truncated: bool,
}


⸻

Tool schemas (JSON)

1) list_tasks

Args

{ "runner": null }

Result

{ "tasks": [ TaskDto, ... ] }  // Each task has uniqified name (e.g., build-m) and original name

2) get_task

Args

{ "name": "test-m" }

Result

{ "task": TaskDto }

3) get_command

Args

{ "task": "test-m", "args": ["--verbose"] }

Result (text)
make test --verbose

4) run_task

Args (start)

{
  "op": "start",
  "task": "dev-n",
  "args": ["--port","5173"],
  "stream": true,           // emit logging notifications
  "env": { "NODE_ENV":"development" },  // optional, default none
  "cwd": null,              // optional, default repo root
  "orphan_ttl_sec": 1800    // optional
}

Result

{ "ok": true, "job_id": "j_01HZ...", "state": "running" }

Notifications (if stream=true)

{ "level":"info", "data": { "job_id": "j_01HZ...", "chunk": "Vite dev server ready...\n" } }

Args (status)

{ "op": "status", "job_id": "j_01HZ..." }

Result

{ "ok": true, "status": JobStatusDto }

Args (stop)

{ "op": "stop", "job_id": "j_01HZ...", "grace_ms": 5000 }

Result

{ "ok": true, "status": JobStatusDto }

Denied example

{ "ok": false, "code":"NotAllowlisted", "hint":"Ask a human to grant MCP access." }

5) read_allowlist

Args

{ "namespace": "mcp" }  // default: "mcp" | "human"

Result
	•	Returns TOML as text/plain and a parsed JSON mirror for convenience.

⸻

Resources
	•	job://<job_id> → JobStatusDto (JSON)
	•	joblog://<job_id>?from=<u64> → { "from": N, "to": M, "data": "<chunk>", "eof": false }
	•	tasks://cwd → { tasks: [ TaskDto ] }

(No write resources.)

⸻

Server layout (skeleton)

// src/mcp/server.rs
use rmcp::{tool, tool_router, ServerHandler, model::*, service::{RequestContext, RoleServer}};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{process::Command, io::{AsyncBufReadExt, BufReader}};
use tokio::sync::{Mutex, RwLock};
use crate::{task_discovery, runner, allowlist, types};

#[derive(Clone)]
pub struct DelaMcpServer {
  root: PathBuf,
  jobs: Arc<RwLock<HashMap<String, Job>>>, // Job holds child, logs buffer, status
}

#[tool_router]
impl DelaMcpServer {
  #[tool(description="List tasks")]
  pub async fn list_tasks(&self, Parameters(ListArgs{runner,include_shadowing}): Parameters<ListArgs>)
    -> Result<CallToolResult, ErrorData> {
    let d = task_discovery::discover_tasks(&self.root);
    let mut tasks = d.tasks;
    if let Some(r) = runner { tasks.retain(|t| t.runner.short_name()==r); }
    let dtos: Vec<TaskDto> = tasks.iter().map(TaskDto::from_task).collect();
    Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({ "tasks": dtos }))]))
  }

  #[tool(description="Get task details")]
  pub async fn get_task(&self, Parameters(GetTaskArgs{name}): Parameters<GetTaskArgs>)
    -> Result<CallToolResult, ErrorData> {
    // resolve by disambiguated or original name; return error if ambiguous
    // ...
    Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({ "task": dto }))]))
  }

  #[tool(description="Get shell command (no exec)")]
  pub async fn get_command(&self, Parameters(GetCmdArgs{task,args}): Parameters<GetCmdArgs>)
    -> Result<CallToolResult, ErrorData> {
    // resolve, check runner availability
    Ok(CallToolResult::success(vec![Content::text(cmd)]))
  }

  #[tool(description="Start/stop/status for tasks")]
  pub async fn run_task(&self, Parameters(RunArgs{op, task, args, job_id, stream, env, cwd, grace_ms, orphan_ttl_sec}): Parameters<RunArgs>)
    -> Result<CallToolResult, ErrorData> {
    match op.as_str() {
      "start" => self.start_job(task.unwrap(), args.unwrap_or_default(), stream.unwrap_or(false), env, cwd, orphan_ttl_sec).await,
      "status" => self.status_job(job_id.unwrap()).await,
      "stop" => self.stop_job(job_id.unwrap(), grace_ms.unwrap_or(5000)).await,
      _ => Ok(CallToolResult::error(vec![Content::text("Invalid op")])),
    }
  }

  #[tool(description="Read allowlist (read-only)")]
  pub async fn read_allowlist(&self, Parameters(ReadAllowArgs{namespace}): Parameters<ReadAllowArgs>)
    -> Result<CallToolResult, ErrorData> {
    let ns = namespace.unwrap_or("mcp".into());
    let (toml_text, json) = /* load & parse */;
    Ok(CallToolResult::success(vec![
      Content::text(toml_text),
      Content::json(&json),
    ]))
  }
}

impl ServerHandler for DelaMcpServer {
  fn get_info(&self) -> ServerInfo {
    ServerInfo {
      protocol_version: ProtocolVersion::V_2024_11_05,
      capabilities: ServerCapabilities::builder().enable_tools().enable_resources().enable_logging().build(),
      server_info: Implementation { name: "dela-mcp".into(), version: env!("CARGO_PKG_VERSION").into() },
      instructions: Some("List and run tasks gated by an MCP allowlist; long-running tasks stream logs as notifications.".into()),
    }
  }
  // implement read_resource for job:// and joblog://
}

Job runner internals
	•	Spawn via tokio::process::Command with stdin closed, capture stdout/stderr
	•	Each line appended to a ring buffer (VecDeque) + notify via logging if stream=true
	•	Maintain JobStatus (state + times + exit code); update on child exit via await + join handle

⸻

Security & Limits
	•	Deny by default if not explicitly allowlisted in MCP file.
	•	Respect .dela path only under real user $HOME.
	•	Output limits: ring buffer size cap; per-message chunk max (e.g., 8 KB).
	•	Concurrency: max jobs N (config), queue or reject beyond limit.
	•	Path policy: Tasks always executed with cwd under repo root (no upward traversal).

⸻

Test checklist (AAA)
	•	Arrange: fixture repo with Makefile, package.json, tasks with same names.
	•	Act: JSON calls to list/get/get_command; start a long runner that prints every 100ms; poll status; stop.
	•	Assert: status transitions, logging notifications, allowlist denials, uniqified names.

⸻

CLI entry

dela mcp [--cwd <dir>]
Starts stdio server in <dir> (default .).

⸻

This keeps the surface area tiny, doesn’t grant MCP write power, and still gives great ergonomics for long-running workloads (jobs + notifications + resources).
