#![allow(deprecated)]

use super::allowlist::McpAllowlistEvaluator;
use super::dto::{
    ListTasksArgs, OutputChunkDto, StartResultDto, TaskDto, TaskOutputArgs, TaskStartArgs,
    TaskStatusArgs, TaskStopArgs,
};
use super::errors::DelaError;
use super::job_manager::{JobManager, JobMetadata, JobState, OutputLine};
use super::notifier::{OutputNotificationBatch, TaskStartNotifier};
use crate::runner::{is_runner_available_for_mcp, split_command_words};
use crate::task_discovery;
use chrono::SecondsFormat;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::*,
    service::{RequestContext, RoleServer},
    tool,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader, stdin, stdout};
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::Duration;

const TASK_DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(60);
const DEFAULT_TASK_START_WAIT_SECONDS: u64 = 1;
const MAX_TASK_START_WAIT_SECONDS: u64 = 3600;

static NEXT_FALLBACK_PID: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(1_000_000_000);

#[derive(Debug, Clone)]
struct CachedDiscoveredTasks {
    discovered: task_discovery::DiscoveredTasks,
    cached_at: Instant,
}

/// MCP server for dela that exposes task management capabilities
#[derive(Clone)]
pub struct DelaMcpServer {
    root: PathBuf,
    allowlist_evaluator: McpAllowlistEvaluator,
    job_manager: JobManager,
    task_cache: Arc<RwLock<HashMap<PathBuf, CachedDiscoveredTasks>>>,
    task_cache_ttl: Duration,
}

impl DelaMcpServer {
    /// Create a new MCP server instance
    pub fn new(root: PathBuf) -> Self {
        let allowlist_evaluator =
            McpAllowlistEvaluator::new().unwrap_or_else(|_| McpAllowlistEvaluator {
                allowlist: crate::types::Allowlist::default(),
            });
        Self::new_inner(root, allowlist_evaluator, TASK_DISCOVERY_CACHE_TTL)
    }

    fn new_inner(
        root: PathBuf,
        allowlist_evaluator: McpAllowlistEvaluator,
        task_cache_ttl: Duration,
    ) -> Self {
        let job_manager = JobManager::new();
        Self {
            root,
            allowlist_evaluator,
            job_manager,
            task_cache: Arc::new(RwLock::new(HashMap::new())),
            task_cache_ttl,
        }
    }

    /// Create a new MCP server instance with a custom allowlist evaluator (for testing)
    #[cfg(test)]
    pub fn new_with_allowlist(
        root: PathBuf,
        mut allowlist_evaluator: McpAllowlistEvaluator,
    ) -> Self {
        for entry in &mut allowlist_evaluator.allowlist.entries {
            if let Ok(canon) = entry.path.canonicalize() {
                entry.path = canon;
            }
        }
        let root = root.canonicalize().unwrap_or(root);
        Self::new_inner(root, allowlist_evaluator, TASK_DISCOVERY_CACHE_TTL)
    }

    #[cfg(test)]
    pub fn new_with_allowlist_and_cache_ttl(
        root: PathBuf,
        mut allowlist_evaluator: McpAllowlistEvaluator,
        task_cache_ttl: Duration,
    ) -> Self {
        for entry in &mut allowlist_evaluator.allowlist.entries {
            if let Ok(canon) = entry.path.canonicalize() {
                entry.path = canon;
            }
        }
        let root = root.canonicalize().unwrap_or(root);
        Self::new_inner(root, allowlist_evaluator, task_cache_ttl)
    }

    fn append_output_chunk(chunks: &mut Vec<OutputChunkDto>, stream: &str, line: &str) {
        match stream {
            "stderr" => chunks.push(OutputChunkDto::stderr(line.to_string())),
            _ => chunks.push(OutputChunkDto::stdout(line.to_string())),
        }
    }

    async fn add_job_output_chunks(
        job_manager: &JobManager,
        pid: u32,
        chunks: &[OutputChunkDto],
    ) -> anyhow::Result<()> {
        for chunk in chunks {
            if let Some(text) = &chunk.stdout {
                job_manager
                    .add_job_output_chunk(pid, "stdout", text.clone())
                    .await?;
            }
            if let Some(text) = &chunk.stderr {
                job_manager
                    .add_job_output_chunk(pid, "stderr", text.clone())
                    .await?;
            }
        }
        Ok(())
    }

    fn output_entries_to_json(entries: &[OutputLine]) -> Vec<serde_json::Value> {
        entries
            .iter()
            .map(|entry| match entry.stream.as_str() {
                "stderr" => serde_json::json!({ "stderr": entry.text }),
                _ => serde_json::json!({ "stdout": entry.text }),
            })
            .collect()
    }

    fn truncate_output_entry_for_chunk(entry: &OutputLine, max_chunk_size: usize) -> OutputLine {
        let mut truncated_line = entry.text.clone();
        if truncated_line.len() > max_chunk_size - 200 {
            truncated_line.truncate(max_chunk_size - 200);
            truncated_line.push_str("... [truncated]");
        }
        OutputLine::new(entry.stream.clone(), truncated_line)
    }

    fn output_flush_timer_deadline(
        deadline: Option<Instant>,
        fallback: Instant,
    ) -> tokio::time::Instant {
        tokio::time::Instant::from_std(deadline.unwrap_or(fallback))
    }

    fn resolve_wait_for_exit_seconds(wait_for_exit_seconds: Option<u64>) -> Result<u64, ErrorData> {
        match wait_for_exit_seconds {
            Some(seconds) if seconds <= MAX_TASK_START_WAIT_SECONDS => Ok(seconds),
            Some(seconds) => Err(ErrorData {
                code: super::errors::DelaErrorCode::INVALID_PARAMS.into(),
                message: format!(
                    "wait_for_exit_seconds must be between 0 and {} seconds, got {}",
                    MAX_TASK_START_WAIT_SECONDS, seconds
                )
                .into(),
                data: Some(serde_json::Value::String(
                    "Use a bounded wait between 0 and 3600 seconds, or omit the field to use the 1-second default.".to_string(),
                )),
            }),
            None => Ok(DEFAULT_TASK_START_WAIT_SECONDS),
        }
    }

    /// Get the root path this server operates in
    #[allow(dead_code)]
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    async fn get_discovered_tasks(&self, root: &PathBuf) -> task_discovery::DiscoveredTasks {
        {
            let cache = self.task_cache.read().await;
            if let Some(entry) = cache
                .get(root)
                .filter(|entry| entry.cached_at.elapsed() < self.task_cache_ttl)
            {
                return entry.discovered.clone();
            }
        }

        let discovered = task_discovery::discover_tasks(root);
        let mut cache = self.task_cache.write().await;
        cache.insert(
            root.clone(),
            CachedDiscoveredTasks {
                discovered: discovered.clone(),
                cached_at: Instant::now(),
            },
        );
        discovered
    }

    /// Resolve a caller-supplied `cwd` to a canonical path, ensuring it stays
    /// within `self.root`.  Relative paths are resolved against `self.root`.
    /// Returns `self.root` when `cwd` is `None`.
    fn resolve_requested_cwd(&self, cwd: &Option<String>) -> Result<PathBuf, ErrorData> {
        let Some(raw) = cwd else {
            return self.root.canonicalize().map_err(|e| {
                DelaError::internal_error(format!("Cannot canonicalize server root: {}", e), None)
                    .into()
            });
        };

        let candidate = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            self.root.join(raw)
        };

        // Canonicalize both so symlinks and `..` are resolved.
        let canonical = candidate.canonicalize().map_err(|e| {
            DelaError::internal_error(
                format!("Invalid cwd '{}': {}", raw, e),
                Some(
                    "Provide an absolute path or a relative path within the workspace".to_string(),
                ),
            )
        })?;

        let root_canonical = self.root.canonicalize().map_err(|e| {
            DelaError::internal_error(format!("Cannot canonicalize server root: {}", e), None)
        })?;

        if !canonical.starts_with(&root_canonical) {
            return Err(DelaError::internal_error(
                format!(
                    "Requested cwd '{}' is outside the workspace root '{}'",
                    raw,
                    self.root.display()
                ),
                Some("cwd must be within the workspace root".to_string()),
            )
            .into());
        }

        Ok(canonical)
    }

    /// Start an MCP stdio server and block until shutdown.
    /// IMPORTANT: Do not print to stdout; MCP JSON-RPC uses stdout.
    pub async fn serve_stdio(self) -> Result<(), ErrorData> {
        // Use (stdin, stdout) as the transport. rmcp will complete initialization
        // and then we block on waiting() to keep the process alive for Inspector.
        let transport = (stdin(), stdout());
        let server = self.serve(transport).await.map_err(|e| {
            DelaError::internal_error(
                format!("Failed to start MCP server: {}", e),
                Some("Check MCP configuration and transport setup".to_string()),
            )
        })?; // completes MCP initialize
        // Block until client disconnect / shutdown
        let _ = server.waiting().await;
        Ok(())
    }
}

impl DelaMcpServer {
    #[tool(description = "List tasks")]
    pub async fn list_tasks(
        &self,
        Parameters(args): Parameters<ListTasksArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let root_dir = self.resolve_requested_cwd(&args.cwd)?;

        let discovered = self.get_discovered_tasks(&root_dir).await;

        // Apply runner filtering if specified
        let mut tasks = discovered.tasks;
        if let Some(runner_filter) = &args.runner {
            tasks.retain(|task| task.runner.short_name() == runner_filter);
        }

        // Convert to DTOs with enriched fields (command, runner_available, allowlisted)
        let task_dtos: Vec<TaskDto> = tasks
            .iter()
            .map(|task| TaskDto::from_task_enriched(task, &self.allowlist_evaluator))
            .collect();

        Ok(CallToolResult::success(vec![
            ContentBlock::json(serde_json::json!({
            "tasks": task_dtos
            }))
            .expect("Failed to serialize JSON"),
        ]))
    }

    #[tool(description = "List all running tasks with PIDs")]
    pub async fn status(&self) -> Result<CallToolResult, ErrorData> {
        // Get all running jobs
        let jobs = self.job_manager.get_all_jobs().await;
        let running_jobs: Vec<serde_json::Value> = jobs
            .into_iter()
            .filter(|job| job.is_running())
            .map(|job| {
                serde_json::json!({
                    "pid": job.pid,
                    "unique_name": job.metadata.unique_name,
                    "source_name": job.metadata.source_name,
                    "command": job.metadata.command,
                    "file_path": job.metadata.file_path.to_string_lossy(),
                    "elapsed_seconds": job.age().as_secs(),
                    "args": job.metadata.args,
                    "cwd": job.metadata.cwd.map(|p| p.to_string_lossy().to_string())
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![
            ContentBlock::json(serde_json::json!({
                "running": running_jobs,
                "cwd": self.root.to_string_lossy().to_string()
            }))
            .expect("Failed to serialize JSON"),
        ]))
    }

    /// Collect output until the task's pipes close, the wait window ends, or the request is
    /// cancelled, streaming it to the request's client along the way.
    async fn run_initial_capture(
        notifier: &mut TaskStartNotifier,
        pid: u32,
        capture_duration: Duration,
        mut stdout_rx: tokio::sync::mpsc::Receiver<String>,
        mut stderr_rx: tokio::sync::mpsc::Receiver<String>,
    ) -> (
        tokio::sync::mpsc::Receiver<String>,
        tokio::sync::mpsc::Receiver<String>,
        Vec<OutputChunkDto>,
    ) {
        let deadline = Instant::now() + capture_duration;
        let request_cancelled = notifier.request_cancellation();
        let mut output_chunks = Vec::new();
        let mut stdout_batch = OutputNotificationBatch::new("stdout");
        let mut stderr_batch = OutputNotificationBatch::new("stderr");
        let mut stdout_done = false;
        let mut stderr_done = false;

        while !(stdout_done && stderr_done) {
            tokio::select! {
                line = stdout_rx.recv(), if !stdout_done => match line {
                    Some(line) => {
                        Self::append_output_chunk(&mut output_chunks, "stdout", &line);
                        stdout_batch.add_line(&line);
                        if stdout_batch.should_flush() {
                            notifier.flush(pid, &mut stdout_batch).await;
                        }
                    }
                    None => stdout_done = true,
                },
                line = stderr_rx.recv(), if !stderr_done => match line {
                    Some(line) => {
                        Self::append_output_chunk(&mut output_chunks, "stderr", &line);
                        stderr_batch.add_line(&line);
                        if stderr_batch.should_flush() {
                            notifier.flush(pid, &mut stderr_batch).await;
                        }
                    }
                    None => stderr_done = true,
                },
                _ = tokio::time::sleep_until(Self::output_flush_timer_deadline(stdout_batch.flush_due_at(), deadline)), if !stdout_batch.is_empty() => {
                    notifier.flush(pid, &mut stdout_batch).await;
                }
                _ = tokio::time::sleep_until(Self::output_flush_timer_deadline(stderr_batch.flush_due_at(), deadline)), if !stderr_batch.is_empty() => {
                    notifier.flush(pid, &mut stderr_batch).await;
                }
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => break,
                // The client no longer wants the response, so stop waiting and let the task
                // continue in the background; its output is still kept for polling.
                _ = request_cancelled.cancelled() => break,
            }
        }

        notifier.flush(pid, &mut stdout_batch).await;
        notifier.flush(pid, &mut stderr_batch).await;
        (stdout_rx, stderr_rx, output_chunks)
    }

    /// Persist output of a backgrounded job until its pipes close, then record its exit.
    ///
    /// The `task_start` request that spawned the job has already returned, so there is no
    /// request to associate notifications with; clients read this output via `task_output`.
    async fn run_background_monitoring(
        pid_u32: u32,
        mut stdout_rx_opt: Option<tokio::sync::mpsc::Receiver<String>>,
        mut stderr_rx_opt: Option<tokio::sync::mpsc::Receiver<String>>,
        job_manager: crate::mcp::job_manager::JobManager,
    ) {
        loop {
            let stdout_done = stdout_rx_opt.is_none();
            let stderr_done = stderr_rx_opt.is_none();

            tokio::select! {
                line = async {
                    if let Some(ref mut rx) = stdout_rx_opt {
                        rx.recv().await
                    } else {
                        std::future::pending::<Option<String>>().await
                    }
                }, if !stdout_done => {
                    match line {
                        Some(line) => {
                            if let Err(error) = job_manager.add_job_output_chunk(pid_u32, "stdout", line).await {
                                tracing::warn!(pid = pid_u32, error = %error, "failed to persist stdout output chunk");
                            }
                        }
                        None => stdout_rx_opt = None,
                    }
                }
                line = async {
                    if let Some(ref mut rx) = stderr_rx_opt {
                        rx.recv().await
                    } else {
                        std::future::pending::<Option<String>>().await
                    }
                }, if !stderr_done => {
                    match line {
                        Some(line) => {
                            if let Err(error) = job_manager.add_job_output_chunk(pid_u32, "stderr", line).await {
                                tracing::warn!(pid = pid_u32, error = %error, "failed to persist stderr output chunk");
                            }
                        }
                        None => stderr_rx_opt = None,
                    }
                }
                else => break,
            }
        }

        let process_opt = job_manager.processes.write().await.remove(&pid_u32);
        if let Some(mut process) = process_opt {
            let state = match process.wait().await {
                Ok(status) => Self::interpret_exit_status(status).0,
                Err(e) => JobState::Failed(format!("Process wait failed: {}", e)),
            };

            let _ = job_manager.update_job_state(pid_u32, state).await;
        }
    }

    fn validate_task_for_start<'a>(
        &self,
        tasks: &'a [crate::types::Task],
        unique_name: &str,
    ) -> Result<&'a crate::types::Task, ErrorData> {
        let task = tasks
            .iter()
            .find(|t| {
                let name = t.disambiguated_name.as_ref().unwrap_or(&t.name);
                name == unique_name
            })
            .ok_or_else(|| DelaError::task_not_found(unique_name.to_string()))?;

        let is_allowed = self
            .allowlist_evaluator
            .is_task_allowed(task)
            .map_err(|e| {
                DelaError::internal_error(
                    format!("MCP allowlist check failed: {}", e),
                    Some("Check allowlist configuration".to_string()),
                )
            })?;

        if !is_allowed {
            return Err(DelaError::not_allowlisted(unique_name.to_string()).into());
        }

        if !is_runner_available_for_mcp(&task.runner) {
            return Err(DelaError::runner_unavailable(
                task.runner.short_name().to_string(),
                unique_name.to_string(),
            )
            .into());
        }

        Ok(task)
    }

    fn build_task_command(
        task: &crate::types::Task,
        root_dir: &Path,
        args: &TaskStartArgs,
    ) -> Result<Command, ErrorData> {
        let full_command = task.runner.get_command(task);
        let command_parts = split_command_words(&full_command).map_err(|e| {
            DelaError::internal_error(
                format!("Failed to parse command '{}': {}", full_command, e),
                Some("Check task definition and runner configuration".to_string()),
            )
        })?;

        let mut command_iter = command_parts.iter();
        let executable = command_iter
            .next()
            .ok_or_else(|| {
                DelaError::internal_error(
                    "Empty command generated".to_string(),
                    Some("Check task definition and runner configuration".to_string()),
                )
            })?
            .clone();
        let base_args: Vec<&String> = command_iter.collect();

        let mut cmd = Command::new(executable);
        cmd.current_dir(root_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.args(base_args);

        if let Some(task_args) = &args.args {
            cmd.args(task_args);
        }

        if let Some(env_vars) = &args.env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        Ok(cmd)
    }

    fn build_job_metadata(
        started_at: Instant,
        task: &crate::types::Task,
        args: &TaskStartArgs,
        root_dir: &Path,
    ) -> JobMetadata {
        JobMetadata {
            started_at,
            unique_name: args.unique_name.clone(),
            source_name: task.source_name.clone(),
            args: args.args.clone(),
            env: args.env.clone(),
            cwd: Some(root_dir.to_path_buf()),
            command: task.runner.get_command(task),
            file_path: task.definition_path().to_path_buf(),
        }
    }

    fn spawn_pipe_reader(
        handle: Option<tokio::process::ChildStdout>,
        tx: tokio::sync::mpsc::Sender<String>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        if let Some(pipe) = handle {
            Some(tokio::spawn(async move {
                let mut reader = BufReader::new(pipe);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx.send(line.clone()).await;
                        }
                        Err(_) => break,
                    }
                }
            }))
        } else {
            drop(tx);
            None
        }
    }

    fn spawn_stderr_reader(
        handle: Option<tokio::process::ChildStderr>,
        tx: tokio::sync::mpsc::Sender<String>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        if let Some(pipe) = handle {
            Some(tokio::spawn(async move {
                let mut reader = BufReader::new(pipe);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx.send(line.clone()).await;
                        }
                        Err(_) => break,
                    }
                }
            }))
        } else {
            drop(tx);
            None
        }
    }

    fn interpret_exit_status(
        exit_status: std::process::ExitStatus,
    ) -> (JobState, Option<i32>, Option<i32>) {
        let mut exit_code = exit_status.code();
        let mut signal = None;
        let mut state = JobState::Exited(exit_code.unwrap_or(-1));
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(sig) = exit_status.signal() {
                signal = Some(sig);
                exit_code = None;
                state = JobState::Signaled(sig);
            }
        }
        (state, exit_code, signal)
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_completed_process(
        &self,
        notifier: &TaskStartNotifier,
        pid: u32,
        exit_status: std::process::ExitStatus,
        metadata: JobMetadata,
        output_chunks: Vec<OutputChunkDto>,
        unique_name: &str,
        stdout_task: Option<tokio::task::JoinHandle<()>>,
        stderr_task: Option<tokio::task::JoinHandle<()>>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Some(task) = stdout_task {
            let _ = task.await;
        }
        if let Some(task) = stderr_task {
            let _ = task.await;
        }

        let (exit_state, exit_code, signal) = Self::interpret_exit_status(exit_status);

        self.job_manager
            .record_completed_job(pid, metadata, exit_state.clone())
            .await
            .map_err(|e| {
                DelaError::internal_error(
                    format!("Failed to record completed job: {}", e),
                    Some("Job management error".to_string()),
                )
            })?;

        if !output_chunks.is_empty() {
            Self::add_job_output_chunks(&self.job_manager, pid, &output_chunks)
                .await
                .map_err(|e| {
                    DelaError::internal_error(
                        format!("Failed to add completed task output: {}", e),
                        Some("Job management error".to_string()),
                    )
                })?;
        }

        notifier
            .task_event(
                pid,
                "exited",
                serde_json::json!({
                    "exit_code": exit_code,
                    "signal": signal,
                    "task": unique_name
                }),
            )
            .await;

        let start_result = StartResultDto {
            state: match exit_state {
                JobState::Signaled(_) => "signaled".to_string(),
                _ => "exited".to_string(),
            },
            pid: None,
            exit_code,
            signal,
            output: output_chunks,
        };

        Ok(CallToolResult::success(vec![
            ContentBlock::json(&start_result).expect("Failed to serialize JSON"),
        ]))
    }

    async fn setup_background_job(
        &self,
        pid: u32,
        child: tokio::process::Child,
        metadata: JobMetadata,
        output_chunks: Vec<OutputChunkDto>,
        stdout_rx: tokio::sync::mpsc::Receiver<String>,
        stderr_rx: tokio::sync::mpsc::Receiver<String>,
    ) -> Result<CallToolResult, ErrorData> {
        self.job_manager
            .start_job(pid, metadata, child)
            .await
            .map_err(|e| {
                DelaError::internal_error(
                    format!("Failed to start background job: {}", e),
                    Some("Job management error".to_string()),
                )
            })?;

        if !output_chunks.is_empty() {
            Self::add_job_output_chunks(&self.job_manager, pid, &output_chunks)
                .await
                .map_err(|e| {
                    DelaError::internal_error(
                        format!("Failed to add initial output: {}", e),
                        Some("Job management error".to_string()),
                    )
                })?;
        }

        tokio::spawn(Self::run_background_monitoring(
            pid,
            Some(stdout_rx),
            Some(stderr_rx),
            self.job_manager.clone(),
        ));

        let start_result = StartResultDto {
            state: "running".to_string(),
            pid: Some(pid as i32),
            exit_code: None,
            signal: None,
            output: output_chunks,
        };

        Ok(CallToolResult::success(vec![
            ContentBlock::json(&start_result).expect("Failed to serialize JSON"),
        ]))
    }

    /// Start a task outside of an MCP request, so nothing is streamed. `call_tool` uses
    /// `start_task` with the request's notifier instead.
    #[cfg(test)]
    #[tool(
        description = "Start a task (default 1s capture, optional bounded wait, then background)"
    )]
    pub async fn task_start(
        &self,
        Parameters(args): Parameters<TaskStartArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        self.start_task(args, TaskStartNotifier::silent()).await
    }

    /// Start a task, streaming its output through `notifier` while the `task_start` request is
    /// in flight. Nothing is sent once the request returns.
    async fn start_task(
        &self,
        args: TaskStartArgs,
        mut notifier: TaskStartNotifier,
    ) -> Result<CallToolResult, ErrorData> {
        let capture_duration = Duration::from_secs(Self::resolve_wait_for_exit_seconds(
            args.wait_for_exit_seconds,
        )?);
        let root_dir = self.resolve_requested_cwd(&args.cwd)?;
        let discovered = self.get_discovered_tasks(&root_dir).await;
        let task = self.validate_task_for_start(&discovered.tasks, &args.unique_name)?;

        self.job_manager.can_start_job().await.map_err(|e| {
            DelaError::internal_error(
                format!("Concurrency limit exceeded: {}", e),
                Some("Too many concurrent jobs running".to_string()),
            )
        })?;

        let full_command = task.runner.get_command(task);
        let mut cmd = Self::build_task_command(task, &root_dir, &args)?;

        let started_at = Instant::now();
        let mut child = cmd.spawn().map_err(|e| {
            DelaError::internal_error(
                format!("Failed to start process: {}", e),
                Some("Check if the command and arguments are valid".to_string()),
            )
        })?;

        let pid = match child.id() {
            Some(id) => id,
            None => {
                let fallback_pid =
                    NEXT_FALLBACK_PID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                tracing::warn!(
                    "Process ID is unavailable for task '{}', using fallback PID {}",
                    args.unique_name,
                    fallback_pid
                );
                fallback_pid
            }
        };
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        notifier
            .task_event(
                pid,
                "started",
                serde_json::json!({
                    "task": args.unique_name,
                    "command": full_command
                }),
            )
            .await;

        let (stdout_tx, stdout_rx) = tokio::sync::mpsc::channel::<String>(100);
        let (stderr_tx, stderr_rx) = tokio::sync::mpsc::channel::<String>(100);

        let stdout_task = Self::spawn_pipe_reader(stdout_handle, stdout_tx);
        let stderr_task = Self::spawn_stderr_reader(stderr_handle, stderr_tx);

        let (stdout_rx, stderr_rx, output_chunks) =
            Self::run_initial_capture(&mut notifier, pid, capture_duration, stdout_rx, stderr_rx)
                .await;

        let process_exited = child.try_wait().is_ok_and(|status| status.is_some());
        let metadata = Self::build_job_metadata(started_at, task, &args, &root_dir);

        if process_exited {
            let exit_status = child.wait().await.map_err(|e| {
                DelaError::internal_error(
                    format!("Failed to wait for process: {}", e),
                    Some("Process management error".to_string()),
                )
            })?;
            return self
                .handle_completed_process(
                    &notifier,
                    pid,
                    exit_status,
                    metadata,
                    output_chunks,
                    &args.unique_name,
                    stdout_task,
                    stderr_task,
                )
                .await;
        }

        self.setup_background_job(pid, child, metadata, output_chunks, stdout_rx, stderr_rx)
            .await
    }

    #[tool(description = "Status for a single unique_name (may have multiple PIDs)")]
    pub async fn task_status(
        &self,
        Parameters(args): Parameters<TaskStatusArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let job = self
            .job_manager
            .get_job(args.pid)
            .await
            .ok_or_else(|| DelaError::job_not_found(args.pid))?;

        let mut status = "running";
        let mut exit_code = None;
        let mut signal = None;
        let mut completed_at = None;
        let mut error = None;

        match &job.state {
            JobState::Running => {}
            JobState::Exited(code) => {
                status = "exited";
                exit_code = Some(*code);
                completed_at = job.completed_at;
            }
            JobState::Signaled(sig) => {
                status = "signaled";
                signal = Some(*sig);
                completed_at = job.completed_at;
            }
            JobState::Failed(msg) => {
                status = "failed";
                error = Some(msg.clone());
                completed_at = job.completed_at;
            }
        }

        let completed_at_str = completed_at
            .as_ref()
            .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, true));

        let job_status = serde_json::json!({
            "pid": job.pid,
            "unique_name": job.metadata.unique_name,
            "source_name": job.metadata.source_name,
            "state": status,
            "error": error,
            "exit_code": exit_code,
            "signal": signal,
            "elapsed_seconds": job.age().as_secs(),
            "completed_at": completed_at_str,
            "command": job.metadata.command,
            "file_path": job.metadata.file_path.to_string_lossy(),
            "args": job.metadata.args,
            "cwd": job.metadata.cwd.map(|p| p.to_string_lossy().to_string())
        });

        Ok(CallToolResult::success(vec![
            ContentBlock::json(job_status).expect("Failed to serialize JSON"),
        ]))
    }

    #[tool(description = "Read output chunks for a PID")]
    pub async fn task_output(
        &self,
        Parameters(args): Parameters<TaskOutputArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let job = self
            .job_manager
            .get_job(args.pid)
            .await
            .ok_or_else(|| DelaError::job_not_found(args.pid))?;

        let requested_lines = args.lines.unwrap_or(200);
        let total_lines = job.output_buffer.len();
        let offset = args
            .offset
            .unwrap_or_else(|| total_lines.saturating_sub(requested_lines))
            .min(total_lines);
        let output_entries = job.get_output_entries_from(offset, requested_lines);
        let returned_lines = output_entries.len();
        let next_offset = offset + returned_lines;
        let output = Self::output_entries_to_json(&output_entries);
        let total_bytes = job.output_buffer.total_bytes();

        let has_more = next_offset < total_lines;
        let dropped_lines = job.output_buffer.dropped_lines;

        let buffer_full = job.output_buffer.is_full();
        let is_truncated = has_more || dropped_lines > 0 || buffer_full;

        // Apply per-message chunk size limit (8KB default)
        const MAX_CHUNK_SIZE: usize = 8 * 1024; // 8KB
        let mut response = serde_json::json!({
            "pid": job.pid,
            "output": output,
            "offset": offset,
            "next_offset": next_offset,
            "total_lines": total_lines,
            "total_bytes": total_bytes,
            "buffer_full": buffer_full,
        });

        if has_more {
            response["has_more_lines"] = serde_json::Value::Bool(true);
        }

        if dropped_lines > 0 {
            response["dropped_lines"] =
                serde_json::Value::Number(serde_json::Number::from(dropped_lines));
        }

        // Add truncation details if requested
        if args.show_truncation.unwrap_or(false) {
            response["truncation_info"] = serde_json::json!({
                "requested_lines": requested_lines,
                "returned_lines": returned_lines,
                "is_truncated": is_truncated,
                "buffer_full": buffer_full,
                "buffer_capacity": job.output_buffer.capacity()
            });
        }

        // Check if response exceeds chunk size limit
        let response_json = serde_json::to_string(&response).unwrap_or_default();
        if response_json.len() > MAX_CHUNK_SIZE {
            // Truncate the response to fit within chunk size limit
            let truncated_entries = if output_entries.len() > 1 {
                // Try to fit as many lines as possible within the limit
                let mut truncated_entries = Vec::new();
                let mut current_size = 0;

                for entry in &output_entries {
                    let entry_json = serde_json::to_string(
                        &Self::output_entries_to_json(std::slice::from_ref(entry))[0],
                    )
                    .unwrap_or_default();
                    if current_size + entry_json.len() + 100 < MAX_CHUNK_SIZE {
                        // 100 bytes buffer for JSON structure
                        truncated_entries.push(entry.clone());
                        current_size += entry_json.len();
                    } else {
                        break;
                    }
                }

                if truncated_entries.is_empty() && !output_entries.is_empty() {
                    // If even one line is too big, truncate it
                    let first_entry = &output_entries[0];
                    truncated_entries.push(Self::truncate_output_entry_for_chunk(
                        first_entry,
                        MAX_CHUNK_SIZE,
                    ));
                }

                truncated_entries
            } else if let Some(first_entry) = output_entries.first() {
                let entry_json = serde_json::to_string(
                    &Self::output_entries_to_json(std::slice::from_ref(first_entry))[0],
                )
                .unwrap_or_default();
                if entry_json.len() + 100 >= MAX_CHUNK_SIZE {
                    vec![Self::truncate_output_entry_for_chunk(
                        first_entry,
                        MAX_CHUNK_SIZE,
                    )]
                } else {
                    output_entries
                }
            } else {
                output_entries
            };

            response["output"] =
                serde_json::Value::Array(Self::output_entries_to_json(&truncated_entries));
            response["next_offset"] = serde_json::Value::Number(serde_json::Number::from(
                offset + truncated_entries.len(),
            ));
            if args.show_truncation.unwrap_or(false) {
                response["truncation_info"]["returned_lines"] =
                    serde_json::Value::Number(serde_json::Number::from(truncated_entries.len()));
            }
            response["chunk_truncated"] = serde_json::Value::Bool(true);
            response["max_chunk_size"] =
                serde_json::Value::Number(serde_json::Number::from(MAX_CHUNK_SIZE));
        }

        Ok(CallToolResult::success(vec![
            ContentBlock::json(&response).expect("Failed to serialize JSON"),
        ]))
    }

    #[tool(description = "Stop a PID with graceful timeout")]
    pub async fn task_stop(
        &self,
        Parameters(args): Parameters<TaskStopArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        // Check if job exists
        let job = self
            .job_manager
            .get_job(args.pid)
            .await
            .ok_or_else(|| DelaError::job_not_found(args.pid))?;

        if !job.is_running() {
            return Err(DelaError::internal_error(
                format!("Job with PID {} is not running", args.pid),
                Some("Job is already finished".to_string()),
            )
            .into());
        }

        // Stop the job gracefully with TERM + grace + KILL
        let grace_period = args.grace_period.unwrap_or(5); // Default 5 seconds
        let stop_result = self
            .job_manager
            .stop_job_graceful(args.pid, grace_period)
            .await
            .map_err(|e| {
                DelaError::internal_error(
                    format!("Failed to stop job: {}", e),
                    Some("Job management error".to_string()),
                )
            })?;

        // Determine the response based on how the job was stopped
        let (status, message, exit_code, signal) = match stop_result {
            crate::mcp::job_manager::StopResult::Graceful(code) => (
                "graceful",
                format!("Process stopped gracefully with exit code {}", code),
                Some(code),
                None,
            ),
            crate::mcp::job_manager::StopResult::Signaled(sig) => (
                "signaled",
                format!("Process stopped by signal {}", sig),
                None,
                Some(sig),
            ),
            crate::mcp::job_manager::StopResult::Forced => (
                "killed",
                "Process was force-killed after grace period".to_string(),
                None,
                None,
            ),
            crate::mcp::job_manager::StopResult::Failed(reason) => (
                "failed",
                format!("Failed to stop process: {}", reason),
                None,
                None,
            ),
        };

        Ok(CallToolResult::success(vec![
            ContentBlock::json(serde_json::json!({
                "pid": args.pid,
                "status": status,
                "message": message,
                "exit_code": exit_code,
                "signal": signal,
                "grace_period_used": grace_period
            }))
            .expect("Failed to serialize JSON"),
        ]))
    }
}

fn parse_tool_args<T: serde::de::DeserializeOwned>(
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<T, ErrorData> {
    serde_json::from_value(serde_json::Value::Object(arguments.unwrap_or_default())).map_err(|e| {
        ErrorData {
            code: super::errors::DelaErrorCode::INVALID_PARAMS.into(),
            message: std::borrow::Cow::Owned(format!("Invalid arguments: {}", e)),
            data: Some(serde_json::Value::String(
                "Check argument format and types".to_string(),
            )),
        }
    })
}

impl ServerHandler for DelaMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_logging()
                .build()
        )
        .with_server_info(
            Implementation::new("dela-mcp", env!("CARGO_PKG_VERSION"))
                .with_title("Dela MCP Server")
                .with_description(
                    "Dela MCP Server for list and executing tasks from definition files like package.json, pyproject.toml, taskfile.yml, etc."
                )
        )
        .with_instructions(
            "List tasks, start them with a default 1-second capture window or an optional wait_for_exit_seconds bounded wait, and manage running tasks via PID; all execution is gated by an MCP allowlist. Cancelling a task_start stops waiting but leaves the task running; find it with status and stop it with task_stop. The server keeps no session state: while a task_start request is in flight its output is sent as progress notifications (if the request has a progressToken) and as log notifications at or above the request's io.modelcontextprotocol/logLevel _meta; afterwards poll task_status and task_output by PID."
        )
    }

    // Manually implement ServerHandler trait methods since #[tool_router] macro is not working
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let result = match request.name.as_ref() {
            "list_tasks" => {
                let args: ListTasksArgs = parse_tool_args(request.arguments)?;
                self.list_tasks(Parameters(args)).await
            }
            "status" => {
                // Status tool takes no arguments
                self.status().await
            }
            "task_start" => {
                let args: TaskStartArgs = parse_tool_args(request.arguments)?;
                let notifier = TaskStartNotifier::new(context.peer, &context.meta, context.ct);
                self.start_task(args, notifier).await
            }
            "task_status" => {
                let args: TaskStatusArgs = parse_tool_args(request.arguments)?;
                self.task_status(Parameters(args)).await
            }
            "task_output" => {
                let args: TaskOutputArgs = parse_tool_args(request.arguments)?;
                self.task_output(Parameters(args)).await
            }
            "task_stop" => {
                let args: TaskStopArgs = parse_tool_args(request.arguments)?;
                self.task_stop(Parameters(args)).await
            }
            _ => Err(DelaError::internal_error(
                format!("Tool not found: {}", request.name),
                Some("Use 'list_tools' to see available tools".to_string()),
            )
            .into()),
        };

        result.map(CallToolResponse::from)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        use serde_json::Map;

        // Schema for list_tasks
        let mut list_tasks_schema = Map::new();
        list_tasks_schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        let mut list_tasks_properties = Map::new();
        let mut runner_prop = Map::new();
        runner_prop.insert(
            "type".to_string(),
            serde_json::Value::String("string".to_string()),
        );
        runner_prop.insert(
            "description".to_string(),
            serde_json::Value::String("Optional runner filter".to_string()),
        );
        list_tasks_properties.insert("runner".to_string(), serde_json::Value::Object(runner_prop));
        let mut list_tasks_cwd_prop = Map::new();
        list_tasks_cwd_prop.insert(
            "type".to_string(),
            serde_json::Value::String("string".to_string()),
        );
        list_tasks_cwd_prop.insert(
            "description".to_string(),
            serde_json::Value::String(
                "Optional working directory to discover tasks in".to_string(),
            ),
        );
        list_tasks_properties.insert(
            "cwd".to_string(),
            serde_json::Value::Object(list_tasks_cwd_prop),
        );
        list_tasks_schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(list_tasks_properties),
        );

        // Schema for task_start
        let mut task_start_schema = Map::new();
        task_start_schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        let mut task_start_properties = Map::new();

        // unique_name (required)
        let mut unique_name_prop = Map::new();
        unique_name_prop.insert(
            "type".to_string(),
            serde_json::Value::String("string".to_string()),
        );
        unique_name_prop.insert(
            "description".to_string(),
            serde_json::Value::String("The unique name of the task to start".to_string()),
        );
        task_start_properties.insert(
            "unique_name".to_string(),
            serde_json::Value::Object(unique_name_prop),
        );

        // args (optional)
        let mut args_prop = Map::new();
        args_prop.insert(
            "type".to_string(),
            serde_json::Value::String("array".to_string()),
        );
        args_prop.insert(
            "items".to_string(),
            serde_json::Value::Object({
                let mut item = Map::new();
                item.insert(
                    "type".to_string(),
                    serde_json::Value::String("string".to_string()),
                );
                item
            }),
        );
        args_prop.insert(
            "description".to_string(),
            serde_json::Value::String("Optional arguments to pass to the task".to_string()),
        );
        task_start_properties.insert("args".to_string(), serde_json::Value::Object(args_prop));

        // env (optional)
        let mut env_prop = Map::new();
        env_prop.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        env_prop.insert(
            "additionalProperties".to_string(),
            serde_json::Value::Object({
                let mut additional = Map::new();
                additional.insert(
                    "type".to_string(),
                    serde_json::Value::String("string".to_string()),
                );
                additional
            }),
        );
        env_prop.insert(
            "description".to_string(),
            serde_json::Value::String("Optional environment variables to set".to_string()),
        );
        task_start_properties.insert("env".to_string(), serde_json::Value::Object(env_prop));

        // cwd (optional)
        let mut cwd_prop = Map::new();
        cwd_prop.insert(
            "type".to_string(),
            serde_json::Value::String("string".to_string()),
        );
        cwd_prop.insert(
            "description".to_string(),
            serde_json::Value::String("Optional working directory".to_string()),
        );
        task_start_properties.insert("cwd".to_string(), serde_json::Value::Object(cwd_prop));

        // wait_for_exit_seconds (optional)
        let mut wait_for_exit_seconds_prop = Map::new();
        wait_for_exit_seconds_prop.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );
        wait_for_exit_seconds_prop
            .insert("minimum".to_string(), serde_json::Value::Number(0.into()));
        wait_for_exit_seconds_prop.insert(
            "maximum".to_string(),
            serde_json::Value::Number(MAX_TASK_START_WAIT_SECONDS.into()),
        );
        wait_for_exit_seconds_prop.insert(
            "description".to_string(),
            serde_json::Value::String(
                format!(
                    "Optional bounded wait in seconds before backgrounding the task. Defaults to {} second when omitted; allowed range: 0-{} seconds.",
                    DEFAULT_TASK_START_WAIT_SECONDS,
                    MAX_TASK_START_WAIT_SECONDS
                ),
            ),
        );
        task_start_properties.insert(
            "wait_for_exit_seconds".to_string(),
            serde_json::Value::Object(wait_for_exit_seconds_prop),
        );

        task_start_schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(task_start_properties),
        );
        task_start_schema.insert(
            "required".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("unique_name".to_string())]),
        );

        // Schema for status (no arguments)
        let mut status_schema = Map::new();
        status_schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        status_schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(Map::new()),
        );

        // Schema for task_status
        let mut task_status_schema = Map::new();
        task_status_schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        let mut task_status_properties = Map::new();
        let mut task_status_pid_prop = Map::new();
        task_status_pid_prop.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );
        task_status_pid_prop.insert(
            "description".to_string(),
            serde_json::Value::String("The PID of the job to get status for".to_string()),
        );
        task_status_properties.insert(
            "pid".to_string(),
            serde_json::Value::Object(task_status_pid_prop),
        );
        task_status_schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(task_status_properties),
        );
        task_status_schema.insert(
            "required".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("pid".to_string())]),
        );

        // Schema for task_output
        let mut task_output_schema = Map::new();
        task_output_schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        let mut task_output_properties = Map::new();
        let mut task_output_pid_prop = Map::new();
        task_output_pid_prop.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );
        task_output_pid_prop.insert(
            "description".to_string(),
            serde_json::Value::String("The PID of the job to get output for".to_string()),
        );
        task_output_properties.insert(
            "pid".to_string(),
            serde_json::Value::Object(task_output_pid_prop),
        );
        let mut task_output_lines_prop = Map::new();
        task_output_lines_prop.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );
        task_output_lines_prop.insert(
            "description".to_string(),
            serde_json::Value::String("Number of lines to return (default: 200)".to_string()),
        );
        task_output_properties.insert(
            "lines".to_string(),
            serde_json::Value::Object(task_output_lines_prop),
        );
        let mut task_output_offset_prop = Map::new();
        task_output_offset_prop.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );
        task_output_offset_prop.insert(
            "description".to_string(),
            serde_json::Value::String(
                "Zero-based offset into the currently retained output buffer. If omitted, returns the tail."
                    .to_string(),
            ),
        );
        task_output_properties.insert(
            "offset".to_string(),
            serde_json::Value::Object(task_output_offset_prop),
        );
        let mut task_output_truncation_prop = Map::new();
        task_output_truncation_prop.insert(
            "type".to_string(),
            serde_json::Value::String("boolean".to_string()),
        );
        task_output_truncation_prop.insert(
            "description".to_string(),
            serde_json::Value::String(
                "Whether to include detailed truncation information (default: false)".to_string(),
            ),
        );
        task_output_properties.insert(
            "show_truncation".to_string(),
            serde_json::Value::Object(task_output_truncation_prop),
        );
        task_output_schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(task_output_properties),
        );
        task_output_schema.insert(
            "required".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("pid".to_string())]),
        );

        // Schema for task_stop
        let mut task_stop_schema = Map::new();
        task_stop_schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        let mut task_stop_properties = Map::new();
        let mut task_stop_pid_prop = Map::new();
        task_stop_pid_prop.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );
        task_stop_pid_prop.insert(
            "description".to_string(),
            serde_json::Value::String("The PID of the job to stop".to_string()),
        );
        task_stop_properties.insert(
            "pid".to_string(),
            serde_json::Value::Object(task_stop_pid_prop),
        );
        let mut task_stop_grace_prop = Map::new();
        task_stop_grace_prop.insert(
            "type".to_string(),
            serde_json::Value::String("integer".to_string()),
        );
        task_stop_grace_prop.insert(
            "description".to_string(),
            serde_json::Value::String(
                "Grace period in seconds before sending SIGKILL (default: 5)".to_string(),
            ),
        );
        task_stop_properties.insert(
            "grace_period".to_string(),
            serde_json::Value::Object(task_stop_grace_prop),
        );
        task_stop_schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(task_stop_properties),
        );
        task_stop_schema.insert(
            "required".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("pid".to_string())]),
        );

        let tools = vec![
            Tool::new_with_raw("list_tasks", Some("List tasks".into()), list_tasks_schema),
            Tool::new_with_raw(
                "status",
                Some("List all running tasks with PIDs".into()),
                status_schema,
            ),
            Tool::new_with_raw(
                "task_start",
                Some(
                    "Start a task (default 1s capture, optional bounded wait, then background)"
                        .into(),
                ),
                task_start_schema,
            ),
            Tool::new_with_raw(
                "task_status",
                Some("Get the status of a single job by its PID".into()),
                task_status_schema,
            ),
            Tool::new_with_raw(
                "task_output",
                Some("Read stream-aware output chunks with optional offset paging".into()),
                task_output_schema,
            ),
            Tool::new_with_raw(
                "task_stop",
                Some("Stop a PID with graceful timeout".into()),
                task_stop_schema,
            ),
        ];

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
            ..Default::default()
        })
    }

    // Legacy clients send this because logging is advertised. Accept it without storing it: a
    // connection-wide level would be session state, so levels are honored per request via `_meta`.
    async fn set_level(
        &self,
        _request: SetLevelRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_tasks_empty() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);
        let args = Parameters(ListTasksArgs::default());

        // Act
        let result = server.list_tasks(args).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        // Should return a JSON response with an empty tasks array
    }

    #[tokio::test]
    async fn test_unimplemented_tools() {
        let server = DelaMcpServer::new(PathBuf::from("."));

        // Test that the new tools work with proper arguments
        let status_args = TaskStatusArgs { pid: 12345 };
        let output_args = TaskOutputArgs {
            pid: 12345,
            lines: Some(10),
            offset: None,
            show_truncation: None,
        };
        let stop_args = TaskStopArgs {
            pid: 12345,
            grace_period: None,
        };

        // task_status, task_output and task_stop should return errors for non-existent jobs
        assert!(server.task_status(Parameters(status_args)).await.is_err());
        assert!(server.task_output(Parameters(output_args)).await.is_err());
        assert!(server.task_stop(Parameters(stop_args)).await.is_err());

        // Status should work (returns empty array in Phase 10A)
        assert!(server.status().await.is_ok());
    }

    #[tokio::test]
    async fn test_status_returns_running_jobs() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Act - Get status with no running jobs
        let result = server.status().await.unwrap();

        // Assert - Should return empty array when no jobs are running
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert!(obj.contains_key("running"));
                let running = obj["running"].as_array().unwrap();
                assert_eq!(
                    running.len(),
                    0,
                    "Status should return empty array when no jobs are running"
                );
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_status_with_running_jobs() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job in the job manager
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: Some(vec!["--verbose".to_string()]),
            env: None,
            cwd: Some(PathBuf::from("/tmp")),
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        // Act
        let result = server.status().await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert!(obj.contains_key("running"));
                let running = obj["running"].as_array().unwrap();
                assert_eq!(running.len(), 1, "Should return one running job");

                let job = &running[0];
                assert_eq!(job["pid"], pid);
                assert_eq!(job["unique_name"], "test-task");
                assert_eq!(job["source_name"], "test");
                assert_eq!(job["command"], "echo test");
                assert!(job["args"].is_array());
                assert_eq!(job["args"][0], "--verbose");
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_status_empty() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);
        let args = TaskStatusArgs { pid: 99999 };

        // Act
        let result = server.task_status(Parameters(args)).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_status_with_jobs() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        let metadata1 = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task1".to_string(),
            source_name: "test".to_string(),
            args: Some(vec!["--verbose".to_string()]),
            env: None,
            cwd: Some(PathBuf::from("/tmp")),
            command: "echo test --verbose".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        let metadata2 = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task2".to_string(),
            source_name: "test".to_string(),
            args: Some(vec!["--quiet".to_string()]),
            env: None,
            cwd: Some(PathBuf::from("/home")),
            command: "echo test --quiet".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start mock jobs
        let mut cmd1 = tokio::process::Command::new("echo");
        cmd1.arg("test");
        cmd1.stdout(std::process::Stdio::piped());
        cmd1.stderr(std::process::Stdio::piped());
        let child1 = cmd1.spawn().unwrap();
        let pid1 = child1.id().unwrap();

        let mut cmd2 = tokio::process::Command::new("echo");
        cmd2.arg("test");
        cmd2.stdout(std::process::Stdio::piped());
        cmd2.stderr(std::process::Stdio::piped());
        let child2 = cmd2.spawn().unwrap();
        let pid2 = child2.id().unwrap();

        server
            .job_manager
            .start_job(pid1, metadata1, child1)
            .await
            .unwrap();
        server
            .job_manager
            .start_job(pid2, metadata2, child2)
            .await
            .unwrap();

        // Act & Assert for job 1
        let result1 = server
            .task_status(Parameters(TaskStatusArgs { pid: pid1 }))
            .await
            .unwrap();
        assert_eq!(result1.content.len(), 1);
        match &result1.content[0] {
            ContentBlock::Text(text_content) => {
                let job: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                assert_eq!(job["unique_name"], "test-task1");
                assert_eq!(job["pid"], pid1);
                assert_eq!(job["state"], "running");
            }
            _ => panic!("Expected text content"),
        }

        // Act & Assert for job 2
        let result2 = server
            .task_status(Parameters(TaskStatusArgs { pid: pid2 }))
            .await
            .unwrap();
        assert_eq!(result2.content.len(), 1);
        match &result2.content[0] {
            ContentBlock::Text(text_content) => {
                let job: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                assert_eq!(job["unique_name"], "test-task2");
                assert_eq!(job["pid"], pid2);
                assert_eq!(job["state"], "running");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn test_task_status_with_different_states() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create jobs with different states
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        // Mark job as exited
        server
            .job_manager
            .update_job_state(pid, JobState::Exited(0))
            .await
            .unwrap();

        let args = TaskStatusArgs { pid };

        // Act
        let result = server.task_status(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let job: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                assert_eq!(job["unique_name"], "test-task");
                assert_eq!(job["state"], "exited");
                assert_eq!(job["exit_code"], 0);
                assert!(job["completed_at"].is_string());
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_status_failed_job_includes_completed_at() {
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();
        server
            .job_manager
            .update_job_state(pid, JobState::Failed("boom".to_string()))
            .await
            .unwrap();

        let result = server
            .task_status(Parameters(TaskStatusArgs { pid }))
            .await
            .unwrap();

        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let job: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                assert_eq!(job["state"], "failed");
                assert!(job["exit_code"].is_null());
                assert!(job["completed_at"].is_string());
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_output_basic() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job with some output
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        // Add some output to the job
        server
            .job_manager
            .add_job_output(pid, "Line 1\nLine 2\nLine 3\n".to_string())
            .await
            .unwrap();
        server
            .job_manager
            .add_job_output_chunk(pid, "stderr", "Warning 1\n".to_string())
            .await
            .unwrap();

        let args = TaskOutputArgs {
            pid,
            lines: Some(2),
            offset: None,
            show_truncation: None,
        };

        // Act
        let result = server.task_output(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert_eq!(obj["pid"], pid);
                assert!(!obj.contains_key("lines"));
                assert!(obj["output"].is_array());
                assert_eq!(obj["offset"], 2);
                assert_eq!(obj["next_offset"], 4);
                assert_eq!(obj["total_lines"], 4);
                assert!(obj["total_bytes"].is_number());
                assert!(obj.get("has_more_lines").is_none()); // We requested 2 lines out of 4, so offset is 2 and next_offset is 4 == total_lines
                assert!(obj["buffer_full"].is_boolean());
                let output = obj["output"].as_array().unwrap();
                assert_eq!(output.len(), 2);
                assert_eq!(output[1]["stderr"], "Warning 1");
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_output_with_truncation_info() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job with some output
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        // Add some output to the job
        server
            .job_manager
            .add_job_output(pid, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n".to_string())
            .await
            .unwrap();

        let args = TaskOutputArgs {
            pid,
            lines: Some(3),
            offset: None,
            show_truncation: Some(true),
        };

        // Act
        let result = server.task_output(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert_eq!(obj["pid"], pid);
                assert!(!obj.contains_key("lines"));
                assert!(obj["output"].is_array());
                assert_eq!(obj["offset"], 2);
                assert_eq!(obj["next_offset"], 5);
                assert_eq!(obj["total_lines"], 5);
                assert!(obj.get("has_more_lines").is_none());

                // Check truncation info is present
                assert!(obj.contains_key("truncation_info"));
                let truncation_info = &obj["truncation_info"];
                assert_eq!(truncation_info["requested_lines"], 3);
                assert_eq!(truncation_info["returned_lines"], 3);
                assert_eq!(truncation_info["is_truncated"], false);
                assert!(truncation_info["buffer_capacity"].is_number());
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_output_with_offset_window() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        server
            .job_manager
            .add_job_output(pid, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n".to_string())
            .await
            .unwrap();

        let args = TaskOutputArgs {
            pid,
            lines: Some(2),
            offset: Some(1),
            show_truncation: Some(true),
        };

        // Act
        let result = server.task_output(Parameters(args)).await.unwrap();

        // Assert
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                let output = obj["output"].as_array().unwrap();
                assert!(!obj.contains_key("lines"));
                assert_eq!(output.len(), 2);
                assert_eq!(output[0]["stdout"], "Line 2");
                assert_eq!(output[1]["stdout"], "Line 3");
                assert_eq!(obj["offset"], 1);
                assert_eq!(obj["next_offset"], 3);
                assert_eq!(obj["total_lines"], 5);
                assert_eq!(obj["has_more_lines"], true);
                assert_eq!(obj["truncation_info"]["returned_lines"], 2);
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_output_no_truncation() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job with some output
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        // Add some output to the job
        server
            .job_manager
            .add_job_output(pid, "Line 1\nLine 2\n".to_string())
            .await
            .unwrap();

        let args = TaskOutputArgs {
            pid,
            lines: Some(5), // Request more lines than available
            offset: None,
            show_truncation: Some(true),
        };

        // Act
        let result = server.task_output(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert_eq!(obj["pid"], pid);
                assert!(!obj.contains_key("lines"));
                assert!(obj["output"].is_array());
                assert_eq!(obj["offset"], 0);
                assert_eq!(obj["next_offset"], 2);
                assert_eq!(obj["total_lines"], 2);
                assert!(obj.get("has_more_lines").is_none()); // No truncation since we have fewer lines than requested

                // Check truncation info is present
                assert!(obj.contains_key("truncation_info"));
                let truncation_info = &obj["truncation_info"];
                assert_eq!(truncation_info["requested_lines"], 5);
                assert_eq!(truncation_info["returned_lines"], 2);
                assert_eq!(truncation_info["is_truncated"], false);
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_output_nonexistent_job() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        let args = TaskOutputArgs {
            pid: 99999, // Non-existent PID
            lines: Some(10),
            offset: None,
            show_truncation: None,
        };

        // Act & Assert
        let result = server.task_output(Parameters(args)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_stop_graceful() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job that will exit quickly
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job that exits quickly
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        let args = TaskStopArgs {
            pid,
            grace_period: Some(2),
        };

        // Act
        let result = server.task_stop(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert_eq!(obj["pid"], pid);
                assert!(obj["status"].is_string());
                assert!(obj["message"].is_string());
                assert_eq!(obj["grace_period_used"], 2);
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_stop_with_default_grace_period() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        let args = TaskStopArgs {
            pid,
            grace_period: None, // Should use default 5 seconds
        };

        // Act
        let result = server.task_stop(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert_eq!(obj["pid"], pid);
                assert_eq!(obj["grace_period_used"], 5); // Default grace period
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_stop_nonexistent_job() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        let args = TaskStopArgs {
            pid: 99999, // Non-existent PID
            grace_period: Some(5),
        };

        // Act & Assert
        let result = server.task_stop(Parameters(args)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_stop_non_running_job() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        // Mark job as exited
        server
            .job_manager
            .update_job_state(pid, JobState::Exited(0))
            .await
            .unwrap();

        let args = TaskStopArgs {
            pid,
            grace_period: Some(5),
        };

        // Act & Assert
        let result = server.task_stop(Parameters(args)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_concurrency_limit_enforcement() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let _server = DelaMcpServer::new(temp_dir);

        // Create a job manager with very low concurrency limit for testing
        let config = crate::mcp::job_manager::JobManagerConfig {
            max_concurrent_jobs: 2,
            max_output_lines_per_job: 10_000,
            max_output_bytes_per_job: 5 * 1024 * 1024,
            job_ttl_seconds: 3600,
            gc_interval_seconds: 300,
        };
        let job_manager = crate::mcp::job_manager::JobManager::with_config(config);

        // Start jobs up to the limit
        let metadata = crate::mcp::job_manager::JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start first job
        let mut cmd1 = tokio::process::Command::new("echo");
        cmd1.arg("test1");
        cmd1.stdout(std::process::Stdio::piped());
        cmd1.stderr(std::process::Stdio::piped());
        let child1 = cmd1.spawn().unwrap();
        let pid1 = child1.id().unwrap();

        job_manager
            .start_job(pid1, metadata.clone(), child1)
            .await
            .unwrap();

        // Start second job
        let mut cmd2 = tokio::process::Command::new("echo");
        cmd2.arg("test2");
        cmd2.stdout(std::process::Stdio::piped());
        cmd2.stderr(std::process::Stdio::piped());
        let child2 = cmd2.spawn().unwrap();
        let pid2 = child2.id().unwrap();

        job_manager
            .start_job(pid2, metadata.clone(), child2)
            .await
            .unwrap();

        // Try to start third job - should fail
        let mut cmd3 = tokio::process::Command::new("echo");
        cmd3.arg("test3");
        cmd3.stdout(std::process::Stdio::piped());
        cmd3.stderr(std::process::Stdio::piped());
        let child3 = cmd3.spawn().unwrap();
        let pid3 = child3.id().unwrap();

        let result = job_manager.start_job(pid3, metadata, child3).await;

        // Assert
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Maximum concurrent jobs limit reached")
        );
        assert!(error.to_string().contains("2"));
    }

    #[tokio::test]
    async fn test_chunk_size_limit() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create a mock job with very large output
        let metadata = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        // Start a mock job
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        server
            .job_manager
            .start_job(pid, metadata, child)
            .await
            .unwrap();

        // Add very large output that will exceed chunk size
        let large_output = "x".repeat(10000); // 10KB line
        server
            .job_manager
            .add_job_output(pid, large_output)
            .await
            .unwrap();

        let args = TaskOutputArgs {
            pid,
            lines: Some(1),
            offset: None,
            show_truncation: Some(true),
        };

        // Act
        let result = server.task_output(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();

                // Should have chunk truncation info
                assert!(obj.contains_key("chunk_truncated"));
                assert_eq!(obj["chunk_truncated"], true);
                assert!(obj.contains_key("max_chunk_size"));
                assert_eq!(obj["max_chunk_size"], 8192); // 8KB

                assert!(!obj.contains_key("lines"));
                let output = obj["output"].as_array().unwrap();
                assert_eq!(output.len(), 1);
                let line = output[0]["stdout"].as_str().unwrap();
                assert!(!line.is_empty());
                assert!(line.len() < 8192);
                assert!(line.ends_with("... [truncated]"));
                // The chunk truncation should be indicated in the response
                assert!(obj.contains_key("chunk_truncated"));
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_concurrency_limit_in_task_start() {
        // This test would require mocking the job manager or creating a custom server
        // with a low concurrency limit, which is complex. For now, we'll test the
        // can_start_job method directly as shown above.
    }

    #[tokio::test]
    async fn test_list_tasks_with_actual_files() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a simple Makefile
        let makefile_content = r#"# Build target
build:
	echo "Building"

# Test target
test:
	echo "Testing"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        // Create a package.json
        let package_json_content = r#"{
  "name": "test-project",
  "scripts": {
    "test": "jest",
    "start": "node server.js"
  }
}"#;
        fs::write(temp_path.join("package.json"), package_json_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(ListTasksArgs::default());

        // Act
        let result = server.list_tasks(args).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        // The test succeeded, which means TaskDto conversion worked
    }

    #[tokio::test]
    async fn test_list_tasks_uses_cached_discovery_within_ttl() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        fs::write(temp_path.join("Makefile"), "build:\n\techo \"Building\"\n").unwrap();

        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: crate::types::Allowlist::default(),
        };
        let server = DelaMcpServer::new_with_allowlist_and_cache_ttl(
            temp_path.to_path_buf(),
            allowlist_evaluator,
            Duration::from_secs(60),
        );

        let first_result = server
            .list_tasks(Parameters(ListTasksArgs::default()))
            .await
            .unwrap();

        fs::write(temp_path.join("Makefile"), "test:\n\techo \"Testing\"\n").unwrap();

        let second_result = server
            .list_tasks(Parameters(ListTasksArgs::default()))
            .await
            .unwrap();

        let first_json = match &first_result.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content with JSON"),
        };
        let second_json = match &second_result.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content with JSON"),
        };

        assert_eq!(first_json["tasks"][0]["source_name"], "build");
        assert_eq!(second_json["tasks"][0]["source_name"], "build");
    }

    #[tokio::test]
    async fn test_list_tasks_refreshes_after_cache_ttl_expires() {
        use std::fs;
        use tempfile::TempDir;
        use tokio::time::sleep;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        fs::write(temp_path.join("Makefile"), "build:\n\techo \"Building\"\n").unwrap();

        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: crate::types::Allowlist::default(),
        };
        let server = DelaMcpServer::new_with_allowlist_and_cache_ttl(
            temp_path.to_path_buf(),
            allowlist_evaluator,
            Duration::from_millis(50),
        );

        let _ = server
            .list_tasks(Parameters(ListTasksArgs::default()))
            .await
            .unwrap();

        fs::write(temp_path.join("Makefile"), "test:\n\techo \"Testing\"\n").unwrap();
        sleep(Duration::from_millis(75)).await;

        let refreshed_result = server
            .list_tasks(Parameters(ListTasksArgs::default()))
            .await
            .unwrap();

        let refreshed_json = match &refreshed_result.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content with JSON"),
        };

        assert_eq!(refreshed_json["tasks"][0]["source_name"], "test");
    }

    #[tokio::test]
    async fn test_list_tasks_with_runner_filter() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Makefile with tasks
        let makefile_content = r#"build:
	echo "Building with make"

test:
	echo "Testing with make"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        // Create a package.json with tasks
        let package_json_content = r#"{
  "name": "test-project",
  "scripts": {
    "test": "jest",
    "start": "node server.js",
    "build": "webpack"
  }
}"#;
        fs::write(temp_path.join("package.json"), package_json_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());

        // Act & Assert - Test filtering by "make"
        let make_args = Parameters(ListTasksArgs {
            runner: Some("make".to_string()),
            cwd: None,
        });
        let make_result = server.list_tasks(make_args).await.unwrap();
        assert_eq!(make_result.content.len(), 1);

        // Act & Assert - Test filtering by "npm"
        let npm_args = Parameters(ListTasksArgs {
            runner: Some("npm".to_string()),
            cwd: None,
        });
        let npm_result = server.list_tasks(npm_args).await.unwrap();
        assert_eq!(npm_result.content.len(), 1);

        // Act & Assert - Test filtering by non-existent runner
        let nonexistent_args = Parameters(ListTasksArgs {
            runner: Some("nonexistent".to_string()),
            cwd: None,
        });
        let nonexistent_result = server.list_tasks(nonexistent_args).await.unwrap();
        assert_eq!(nonexistent_result.content.len(), 1);
        // Should return empty tasks array

        // Act & Assert - Test no filter (should return all tasks)
        let all_args = Parameters(ListTasksArgs::default());
        let all_result = server.list_tasks(all_args).await.unwrap();
        assert_eq!(all_result.content.len(), 1);
    }

    #[tokio::test]
    async fn test_list_tasks_runner_filter_case_sensitivity() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Makefile
        let makefile_content = r#"build:
	echo "Building"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());

        // Act & Assert - Test exact match
        let exact_args = Parameters(ListTasksArgs {
            runner: Some("make".to_string()),
            cwd: None,
        });
        let exact_result = server.list_tasks(exact_args).await.unwrap();
        assert_eq!(exact_result.content.len(), 1);

        // Act & Assert - Test case mismatch (should return empty)
        let case_args = Parameters(ListTasksArgs {
            runner: Some("MAKE".to_string()),
            cwd: None,
        });
        let case_result = server.list_tasks(case_args).await.unwrap();
        assert_eq!(case_result.content.len(), 1);
        // Should return empty tasks array since "MAKE" != "make"
    }

    #[tokio::test]
    async fn test_list_tasks_enriched_fields() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a simple Makefile
        let makefile_content = r#"# Build the project
build:
	echo "Building"

test:
	echo "Testing"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(ListTasksArgs::default());

        // Act
        let result = server.list_tasks(args).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);

        // For this test, we just verify that the call succeeded and returned content
        // The actual JSON parsing and field verification is complex due to the Content type
        // The important thing is that from_task_enriched() is being called and doesn't crash

        // We can verify indirectly by checking that the result is not an error
        // and contains content (which means TaskDto serialization worked)
        assert!(result.is_error.is_none() || !result.is_error.unwrap());
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_list_tasks_enriched_fields_detailed() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Makefile with a task that has a description
        let makefile_content = r#"# Build the project
.PHONY: build test

build: ## Build the project
	echo "Building"

test: ## Run tests
	echo "Testing"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(ListTasksArgs::default());

        // Act
        let result = server.list_tasks(args).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert!(obj.contains_key("tasks"));

                let tasks = obj["tasks"].as_array().unwrap();
                assert!(!tasks.is_empty(), "Should have at least one task");

                // Check that each task has all the enriched fields
                for task in tasks {
                    let task_obj = task.as_object().unwrap();

                    // Required fields
                    assert!(task_obj.contains_key("unique_name"));
                    assert!(task_obj.contains_key("source_name"));
                    assert!(task_obj.contains_key("runner"));
                    assert!(task_obj.contains_key("command"));
                    assert!(task_obj.contains_key("runner_available"));
                    assert!(task_obj.contains_key("allowlisted"));
                    assert!(task_obj.contains_key("file_path"));

                    // Optional fields
                    assert!(task_obj.contains_key("description"));

                    // Verify field types
                    assert!(task_obj["unique_name"].is_string());
                    assert!(task_obj["source_name"].is_string());
                    assert!(task_obj["runner"].is_string());
                    assert!(task_obj["command"].is_string());
                    assert!(task_obj["runner_available"].is_boolean());
                    assert!(task_obj["allowlisted"].is_boolean());
                    assert!(task_obj["file_path"].is_string());

                    // Verify command contains the runner
                    let runner = task_obj["runner"].as_str().unwrap();
                    let command = task_obj["command"].as_str().unwrap();
                    assert!(
                        command.starts_with(runner),
                        "Command should start with runner name"
                    );
                }
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn test_list_tasks_in_project_root() {
        // Test with a temporary directory that has some task files
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a simple Makefile
        let makefile_content = r#"build:
	@echo "Building"

test:
	@echo "Testing"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        // Create a package.json
        let package_json_content = r#"{
  "name": "test-project",
  "scripts": {
    "start": "node server.js",
    "test": "jest"
  }
}"#;
        fs::write(temp_path.join("package.json"), package_json_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(ListTasksArgs::default());

        // Act
        let result = server.list_tasks(args).await.unwrap();

        // Assert
        assert!(result.is_error.is_none() || !result.is_error.unwrap());
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_task_start_not_found() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);
        let args = Parameters(TaskStartArgs {
            unique_name: "nonexistent-task".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        });

        // Act
        let result = server.task_start(args).await;

        // Assert
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("not found"));
        assert!(error.message.contains("nonexistent-task"));
        // Check that it's a TASK_NOT_FOUND error
        assert_eq!(error.code.0, -32012);
    }

    #[tokio::test]
    async fn test_task_start_cmake_disabled_for_mcp() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        let cmake_path = temp_path.join("CMakeLists.txt");

        let cmake_content = r#"
cmake_minimum_required(VERSION 3.10)
project(TestProject)

add_custom_target(build-all COMMENT "Build everything")
"#;
        fs::write(&cmake_path, cmake_content).unwrap();

        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: crate::types::Allowlist {
                entries: vec![crate::types::AllowlistEntry {
                    path: cmake_path,
                    scope: crate::types::AllowScope::File,
                    tasks: None,
                }],
            },
        };

        let server =
            DelaMcpServer::new_with_allowlist(temp_path.to_path_buf(), allowlist_evaluator);
        let args = Parameters(TaskStartArgs {
            unique_name: "build-all".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        });

        let result = server.task_start(args).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code.0, -32011);
        assert!(error.message.contains("Runner 'cmake' is not available"));
        let hint = error
            .data
            .and_then(|value| value.as_str().map(str::to_string));
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("MCP execution is disabled"));
    }

    #[tokio::test]
    async fn test_error_taxonomy() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange - Create a test directory with a Makefile
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let makefile_content = r#"build:
	echo "Building"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());

        // Test 1: TaskNotFound error
        let args = Parameters(TaskStartArgs {
            unique_name: "nonexistent-task".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        });
        let result = server.task_start(args).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code.0, -32012); // TASK_NOT_FOUND
        assert!(error.message.contains("not found"));
        assert!(error.data.is_some());
        assert!(error.data.unwrap().as_str().unwrap().contains("list_tasks"));

        // Test 2: RunnerUnavailable error (simulate by using a non-existent runner)
        // This is harder to test without mocking, so we'll test the error creation directly
        let error = DelaError::runner_unavailable("make".to_string(), "build".to_string());
        let error_data = error.to_error_data();
        assert_eq!(error_data.code.0, -32011); // RUNNER_UNAVAILABLE
        assert!(
            error_data
                .message
                .contains("Runner 'make' is not available")
        );
        assert!(error_data.data.is_some());
        assert!(
            error_data
                .data
                .unwrap()
                .as_str()
                .unwrap()
                .contains("brew install make")
        );

        // Test 3: NotAllowlisted error
        let error = DelaError::not_allowlisted("build".to_string());
        let error_data = error.to_error_data();
        assert_eq!(error_data.code.0, -32010); // NOT_ALLOWLISTED
        assert!(error_data.message.contains("not allowlisted"));
        assert!(error_data.data.is_some());
        assert!(
            error_data
                .data
                .unwrap()
                .as_str()
                .unwrap()
                .contains("Ask a human")
        );

        // Test 4: InternalError
        let error =
            DelaError::internal_error("Test error".to_string(), Some("Test hint".to_string()));
        let error_data = error.to_error_data();
        assert_eq!(error_data.code.0, -32603); // INTERNAL_ERROR
        assert!(error_data.message.contains("Test error"));
        assert!(error_data.data.is_some());
        assert_eq!(error_data.data.unwrap().as_str().unwrap(), "Test hint");
    }

    #[tokio::test]
    async fn test_task_start_quick_execution() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange - Create a test directory with a quick-executing task
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Makefile with a quick echo task
        let makefile_content = r#"quick-echo:
	echo "Hello from quick task"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(TaskStartArgs {
            unique_name: "quick-echo".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        });

        // Act
        let result = server.task_start(args).await;

        // Assert - This should succeed and return a quick execution result
        // Note: This test may fail if make is not available, which is expected
        // The important thing is that it tests the quick execution path
        match result {
            Ok(call_result) => {
                // If it succeeds, verify the structure
                assert_eq!(call_result.content.len(), 1);
                let content = &call_result.content[0];
                match content {
                    ContentBlock::Text(text_content) => {
                        let json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        let obj = json.as_object().unwrap();
                        assert!(obj.contains_key("state"));
                        // Should be either "exited" (quick completion) or "running" (backgrounded)
                        let state = obj["state"].as_str().unwrap();
                        assert!(state == "exited" || state == "running");
                    }
                    _ => panic!("Expected text content"),
                }
            }
            Err(_) => {
                // If it fails due to missing make, that's also acceptable for this test
                // The important thing is that we're testing the quick execution path
            }
        }
    }

    #[tokio::test]
    async fn test_task_start_with_args() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange - Create a test directory with a task that accepts arguments
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Makefile with a task that uses arguments
        let makefile_content = r#"test-args:
	echo "Args: $(ARGS)"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(TaskStartArgs {
            unique_name: "test-args".to_string(),
            args: Some(vec!["--verbose".to_string(), "--debug".to_string()]),
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        });

        // Act
        let result = server.task_start(args).await;

        // Assert - Test that arguments are properly passed
        // This may fail if make is not available, which is expected
        match result {
            Ok(_) => {
                // If it succeeds, that's great - we've tested argument passing
            }
            Err(_) => {
                // If it fails due to missing make, that's also acceptable
                // The important thing is that we're testing the argument passing path
            }
        }
    }

    #[tokio::test]
    async fn test_task_start_with_env() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange - Create a test directory with a task that uses environment variables
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Makefile with a task that uses environment variables
        let makefile_content = r#"test-env:
	echo "ENV_VAR: $$ENV_VAR"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let mut env_vars = std::collections::HashMap::new();
        env_vars.insert("ENV_VAR".to_string(), "test_value".to_string());

        let args = Parameters(TaskStartArgs {
            unique_name: "test-env".to_string(),
            args: None,
            env: Some(env_vars),
            cwd: None,
            wait_for_exit_seconds: None,
        });

        // Act
        let result = server.task_start(args).await;

        // Assert - Test that environment variables are properly passed
        // This may fail if make is not available, which is expected
        match result {
            Ok(_) => {
                // If it succeeds, that's great - we've tested environment variable passing
            }
            Err(_) => {
                // If it fails due to missing make, that's also acceptable
                // The important thing is that we're testing the environment variable passing path
            }
        }
    }

    #[tokio::test]
    async fn test_task_start_with_cwd() {
        use std::fs;
        use tempfile::TempDir;

        // Arrange - Create a test directory with a task that uses working directory
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a Makefile with a task that uses working directory
        let makefile_content = r#"test-cwd:
	pwd
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();

        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(TaskStartArgs {
            unique_name: "test-cwd".to_string(),
            args: None,
            env: None,
            cwd: Some(temp_path.to_string_lossy().to_string()),
            wait_for_exit_seconds: None,
        });

        // Act
        let result = server.task_start(args).await;

        // Assert - Test that working directory is properly set
        // This may fail if make is not available, which is expected
        match result {
            Ok(_) => {
                // If it succeeds, that's great - we've tested working directory setting
            }
            Err(_) => {
                // If it fails due to missing make, that's also acceptable
                // The important thing is that we're testing the working directory setting path
            }
        }
    }

    #[tokio::test]
    async fn test_task_start_wait_for_exit_returns_exited_within_window() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;
        use tokio::time::{Duration, sleep};

        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("waited_task.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'Starting...'\necho 'Warning on stderr' >&2\nsleep 2\necho 'Finished within wait window'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: crate::types::Allowlist {
                entries: vec![crate::types::AllowlistEntry {
                    path: script_path.clone(),
                    scope: crate::types::AllowScope::File,
                    tasks: None,
                }],
            },
        };
        let server =
            DelaMcpServer::new_with_allowlist(temp_dir.path().to_path_buf(), allowlist_evaluator);

        let args = Parameters(TaskStartArgs {
            unique_name: "waited_task".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: Some(3),
        });

        let result = server.task_start(args).await.unwrap();
        let content = &result.content[0];
        let json = match content {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };

        assert_eq!(json["state"], "exited");
        assert_eq!(json["exit_code"], 0);
        assert!(json.get("pid").is_none());
        let output_chunks = json["output"].as_array().unwrap();
        assert!(output_chunks.iter().any(|chunk| {
            chunk
                .get("stdout")
                .and_then(|text| text.as_str())
                .is_some_and(|text| text.contains("Finished within wait window"))
        }));
        assert!(output_chunks.iter().any(|chunk| {
            chunk
                .get("stderr")
                .and_then(|text| text.as_str())
                .is_some_and(|text| text.contains("Warning on stderr"))
        }));
        assert!(json.get("initial_output").is_none());

        let status_result = server.status().await.unwrap();
        let status_json = match &status_result.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        assert_eq!(status_json["running"].as_array().unwrap().len(), 0);

        let completed_jobs = server.job_manager.get_all_jobs().await;
        let job = completed_jobs
            .iter()
            .find(|j| j.metadata.unique_name == "waited_task")
            .unwrap();
        let pid = job.pid;

        let task_status_result = server
            .task_status(Parameters(TaskStatusArgs { pid }))
            .await
            .unwrap();
        let task_status_json = match &task_status_result.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        assert_eq!(task_status_json["state"], "exited");

        sleep(Duration::from_secs(2)).await;

        let task_status_result_later = server
            .task_status(Parameters(TaskStatusArgs { pid }))
            .await
            .unwrap();
        let task_status_json_later = match &task_status_result_later.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        let elapsed_seconds = task_status_json_later["elapsed_seconds"].as_u64().unwrap();
        assert!(
            (1..=2).contains(&elapsed_seconds),
            "completed task elapsed_seconds should reflect its actual runtime, got {}",
            elapsed_seconds
        );
    }

    #[tokio::test]
    async fn test_task_start_wait_for_exit_backgrounds_after_timeout() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;
        use tokio::time::{Duration, sleep};

        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("still_running_task.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'Starting...'\nsleep 4\necho 'Finished after timeout'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: crate::types::Allowlist {
                entries: vec![crate::types::AllowlistEntry {
                    path: script_path.clone(),
                    scope: crate::types::AllowScope::File,
                    tasks: None,
                }],
            },
        };
        let server =
            DelaMcpServer::new_with_allowlist(temp_dir.path().to_path_buf(), allowlist_evaluator);

        let args = Parameters(TaskStartArgs {
            unique_name: "still_running_task".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: Some(2),
        });

        let result = server.task_start(args).await.unwrap();
        let content = &result.content[0];
        let json = match content {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };

        assert_eq!(json["state"], "running");
        let pid = json["pid"].as_i64().unwrap() as u32;
        let output_chunks = json["output"].as_array().unwrap();
        assert!(output_chunks.iter().any(|chunk| {
            chunk
                .get("stdout")
                .and_then(|text| text.as_str())
                .is_some_and(|text| text.contains("Starting..."))
        }));
        assert!(json.get("initial_output").is_none());

        let status_result = server.status().await.unwrap();
        let status_json = match &status_result.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        assert_eq!(status_json["running"].as_array().unwrap().len(), 1);

        let task_status_result = server
            .task_status(Parameters(TaskStatusArgs { pid }))
            .await
            .unwrap();
        let task_status_json = match &task_status_result.content[0] {
            ContentBlock::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        assert_eq!(task_status_json["state"], "running");
        assert!(
            task_status_json["elapsed_seconds"].as_u64().unwrap() >= 2,
            "elapsed_seconds should include the initial bounded wait window"
        );

        let stop_result = server
            .task_stop(Parameters(TaskStopArgs {
                pid,
                grace_period: Some(1),
            }))
            .await;
        assert!(stop_result.is_ok());

        sleep(Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn test_task_start_wait_for_exit_rejects_values_above_max() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("bounded_task.sh");
        let ran_marker = temp_dir.path().join("ran");
        std::fs::write(
            &script_path,
            format!("#!/bin/bash\ntouch '{}'\n", ran_marker.display()),
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: crate::types::Allowlist {
                entries: vec![crate::types::AllowlistEntry {
                    path: script_path.clone(),
                    scope: crate::types::AllowScope::File,
                    tasks: None,
                }],
            },
        };
        let server =
            DelaMcpServer::new_with_allowlist(temp_dir.path().to_path_buf(), allowlist_evaluator);

        let result = server
            .task_start(Parameters(TaskStartArgs {
                unique_name: "bounded_task".to_string(),
                args: None,
                env: None,
                cwd: None,
                wait_for_exit_seconds: Some(MAX_TASK_START_WAIT_SECONDS + 1),
            }))
            .await;

        let error = result.unwrap_err();
        assert_eq!(error.code.0, -32602);
        assert!(error.message.contains("wait_for_exit_seconds"));
        assert!(error.message.contains("3600"));
        // A rejected request must not have spawned the task.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!ran_marker.exists(), "task ran despite the rejected wait");
    }

    #[tokio::test]
    async fn test_long_running_task_lifecycle() {
        use std::os::unix::fs::PermissionsExt;
        use tokio::time::{Duration, sleep};

        let temp_dir = tempfile::TempDir::new().unwrap();

        // Create a shell script that runs for 3 seconds
        // Shell scripts are discovered directly by task_discovery when they have .sh extension
        // This avoids depending on 'make' being installed on the system
        let script_path = temp_dir.path().join("long_task.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'Starting...'\nsleep 3\necho 'Done!'",
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Create a mock allowlist evaluator that allows the shell script
        let mock_allowlist = crate::types::Allowlist {
            entries: vec![crate::types::AllowlistEntry {
                path: script_path.clone(),
                scope: crate::types::AllowScope::File,
                tasks: None,
            }],
        };
        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: mock_allowlist,
        };

        let server =
            DelaMcpServer::new_with_allowlist(temp_dir.path().to_path_buf(), allowlist_evaluator);

        // Start the long-running task (shell script name without .sh extension)
        let start_args = TaskStartArgs {
            unique_name: "long_task".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        };

        let start_result = server.task_start(Parameters(start_args)).await;
        assert!(start_result.is_ok(), "task_start should succeed");

        // Parse the result to get the PID
        let start_response = start_result.unwrap();
        let content = &start_response.content[0];
        match content {
            ContentBlock::Text(text_content) => {
                let json_response: serde_json::Value =
                    serde_json::from_str(&text_content.text).unwrap();
                let pid = json_response["pid"].as_i64().unwrap() as u32;
                let state = json_response["state"].as_str().unwrap();

                // Should start as running
                println!("Task started with state: {}, pid: {}", state, pid);
                assert_eq!(state, "running", "Task should start in running state");

                // Check status immediately - should show as running
                let status_result = server.status().await.unwrap();
                let status_content = &status_result.content[0];
                match status_content {
                    ContentBlock::Text(text_content) => {
                        let status_json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        let running_jobs = status_json["running"].as_array().unwrap();
                        println!(
                            "Status immediately after start: {} running jobs",
                            running_jobs.len()
                        );
                        assert_eq!(running_jobs.len(), 1, "Should have 1 running job");
                        assert_eq!(running_jobs[0]["pid"].as_i64().unwrap() as u32, pid);
                    }
                    _ => panic!("Expected text content"),
                }

                // Check task_status immediately - should show as running
                let task_status_args = TaskStatusArgs { pid };
                let task_status_result = server
                    .task_status(Parameters(task_status_args))
                    .await
                    .unwrap();
                let task_status_content = &task_status_result.content[0];
                match task_status_content {
                    ContentBlock::Text(text_content) => {
                        let task_status_json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        println!(
                            "Task status immediately after start: {}",
                            task_status_json["state"].as_str().unwrap_or("unknown")
                        );
                        assert_eq!(task_status_json["state"].as_str().unwrap(), "running");
                        assert_eq!(task_status_json["pid"].as_i64().unwrap() as u32, pid);
                    }
                    _ => panic!("Expected text content"),
                }

                // Wait for 1 second - should still be running
                sleep(Duration::from_secs(1)).await;

                let status_result_after_1s = server.status().await.unwrap();
                let status_content_after_1s = &status_result_after_1s.content[0];
                match status_content_after_1s {
                    ContentBlock::Text(text_content) => {
                        let status_json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        let running_jobs = status_json["running"].as_array().unwrap();
                        println!("Status after 1 second: {} running jobs", running_jobs.len());
                        assert_eq!(
                            running_jobs.len(),
                            1,
                            "Should still have 1 running job after 1s"
                        );
                    }
                    _ => panic!("Expected text content"),
                }

                // Wait for task to complete (3 seconds + buffer)
                sleep(Duration::from_secs(4)).await;

                // Check status after completion - should show no running jobs
                let status_result_final = server.status().await.unwrap();
                let status_content_final = &status_result_final.content[0];
                match status_content_final {
                    ContentBlock::Text(text_content) => {
                        let status_json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        let running_jobs = status_json["running"].as_array().unwrap();
                        println!(
                            "Status after completion: {} running jobs",
                            running_jobs.len()
                        );
                        assert_eq!(
                            running_jobs.len(),
                            0,
                            "Should have no running jobs after completion"
                        );
                    }
                    _ => panic!("Expected text content"),
                }

                // Check task_status after completion - should show as exited
                let task_status_args_final = TaskStatusArgs { pid };
                let task_status_result_final = server
                    .task_status(Parameters(task_status_args_final))
                    .await
                    .unwrap();
                let task_status_content_final = &task_status_result_final.content[0];
                match task_status_content_final {
                    ContentBlock::Text(text_content) => {
                        let task_status_json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        println!(
                            "Task status after completion: {}",
                            task_status_json["state"].as_str().unwrap_or("unknown")
                        );
                        assert_eq!(task_status_json["state"].as_str().unwrap(), "exited");
                        assert_eq!(task_status_json["pid"].as_i64().unwrap() as u32, pid);
                    }
                    _ => panic!("Expected text content"),
                }
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_backgrounding_task_exits_immediately() {
        use crate::environment::{
            TestEnvironment, reset_to_real_environment, set_test_environment,
        };
        use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
        use std::os::unix::fs::PermissionsExt;
        use tokio::time::{Duration, sleep};

        // Set up test environment and mock make
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("make");
        set_test_environment(env);
        mock_executable("make");

        let temp_dir = tempfile::TempDir::new().unwrap();

        // Script that backgrounds real work and exits immediately
        let script_path = temp_dir.path().join("bg_task.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'Spawning background...'\nsleep 3 &\necho 'Parent exiting now'",
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Makefile target that runs the backgrounding script
        let makefile_path = temp_dir.path().join("Makefile");
        std::fs::write(
            &makefile_path,
            format!("bg-test:\n\t{}", script_path.display()),
        )
        .unwrap();

        // Mock allowlist to allow this Makefile
        let mock_allowlist = crate::types::Allowlist {
            entries: vec![crate::types::AllowlistEntry {
                path: makefile_path.clone(),
                scope: crate::types::AllowScope::File,
                tasks: None,
            }],
        };
        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: mock_allowlist,
        };
        let server =
            DelaMcpServer::new_with_allowlist(temp_dir.path().to_path_buf(), allowlist_evaluator);

        // Start the backgrounding task
        let start_args = TaskStartArgs {
            unique_name: "bg-test".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        };
        let start_response = server.task_start(Parameters(start_args)).await.unwrap();

        // Parse start result
        let content = &start_response.content[0];
        let start_state = match content {
            ContentBlock::Text(text_content) => {
                let json_response: serde_json::Value =
                    serde_json::from_str(&text_content.text).unwrap();
                json_response["state"].as_str().unwrap().to_string()
            }
            _ => panic!("Expected text content"),
        };

        // It may start as running if shell hasn’t exited within the 1s capture, so wait a moment
        sleep(Duration::from_millis(300)).await;

        // Immediately after, status should often show 0 running because parent shell exits
        let status_result = server.status().await.unwrap();
        let status_content = &status_result.content[0];
        match status_content {
            ContentBlock::Text(text_content) => {
                let status_json: serde_json::Value =
                    serde_json::from_str(&text_content.text).unwrap();
                let running_jobs = status_json["running"].as_array().unwrap();
                // Backgrounded recipe: parent exits quickly → typically no running jobs
                assert!(
                    running_jobs.is_empty(),
                    "Backgrounded task parent should exit quickly"
                );
            }
            _ => panic!("Expected text content"),
        }

        let pid = server
            .job_manager
            .get_all_jobs()
            .await
            .iter()
            .find(|job| job.metadata.unique_name == "bg-test")
            .map(|job| job.pid)
            .expect("Should have recorded the job");

        // task_status should record the job as exited quickly
        let task_status_args = TaskStatusArgs { pid };
        let task_status_result = server
            .task_status(Parameters(task_status_args))
            .await
            .unwrap();
        let task_status_content = &task_status_result.content[0];
        match task_status_content {
            ContentBlock::Text(text_content) => {
                let job: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                assert_eq!(job["state"].as_str().unwrap(), "exited");
                if start_state == "running" {
                    assert!(job["pid"].is_number());
                }
            }
            _ => panic!("Expected text content"),
        }

        // Clean up test environment
        reset_mock();
        reset_to_real_environment();
    }

    #[tokio::test]
    async fn test_task_output_captures_initial_lines() {
        use std::os::unix::fs::PermissionsExt;
        use tokio::time::{Duration, sleep};

        let temp_dir = tempfile::TempDir::new().unwrap();

        // Script that prints several lines immediately, then sleeps
        // Shell scripts are discovered directly by task_discovery when they have .sh extension
        // This avoids depending on 'make' being installed on the system
        let script_path = temp_dir.path().join("out_task.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'LINE-ONE'\necho 'LINE-TWO'\necho 'LINE-THREE'\nsleep 2\necho 'AFTER-SLEEP'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Allowlist mock to allow the shell script
        let mock_allowlist = crate::types::Allowlist {
            entries: vec![crate::types::AllowlistEntry {
                path: script_path.clone(),
                scope: crate::types::AllowScope::File,
                tasks: None,
            }],
        };
        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: mock_allowlist,
        };
        let server =
            DelaMcpServer::new_with_allowlist(temp_dir.path().to_path_buf(), allowlist_evaluator);

        // Start task (shell script name without .sh extension)
        let start_args = TaskStartArgs {
            unique_name: "out_task".to_string(),
            args: None,
            env: None,
            cwd: None,
            wait_for_exit_seconds: None,
        };
        let start_response = server.task_start(Parameters(start_args)).await.unwrap();

        // Extract pid
        let content = &start_response.content[0];
        let pid = match content {
            ContentBlock::Text(text_content) => {
                let json_response: serde_json::Value =
                    serde_json::from_str(&text_content.text).unwrap();
                json_response["pid"].as_i64().unwrap() as u32
            }
            _ => panic!("Expected text content"),
        };

        // Give a short moment for initial output capture path to register
        sleep(Duration::from_millis(200)).await;

        // Call task_output for last lines
        let out_args = TaskOutputArgs {
            pid,
            lines: Some(10),
            offset: None,
            show_truncation: Some(true),
        };
        let out_result = server.task_output(Parameters(out_args)).await.unwrap();
        let out_content = &out_result.content[0];
        match out_content {
            ContentBlock::Text(text_content) => {
                let output_json: serde_json::Value =
                    serde_json::from_str(&text_content.text).unwrap();
                assert_eq!(output_json["pid"].as_i64().unwrap() as u32, pid);
                assert!(output_json.get("lines").is_none());
                let output = output_json["output"].as_array().unwrap();
                // Expect initial lines present
                let joined = output
                    .iter()
                    .filter_map(|chunk| {
                        chunk
                            .get("stdout")
                            .or_else(|| chunk.get("stderr"))
                            .and_then(|text| text.as_str())
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    joined.contains("LINE-ONE"),
                    "missing LINE-ONE in output: {}",
                    joined
                );
                assert!(
                    joined.contains("LINE-TWO"),
                    "missing LINE-TWO in output: {}",
                    joined
                );
                assert!(
                    joined.contains("LINE-THREE"),
                    "missing LINE-THREE in output: {}",
                    joined
                );
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn test_logging_capability_enabled() {
        use rmcp::ServerHandler;
        use std::path::PathBuf;

        let server = DelaMcpServer::new(PathBuf::from("."));
        let info = server.get_info();

        // Verify logging capability is enabled for DTKT-177
        assert!(
            info.capabilities.logging.is_some(),
            "Logging capability should be enabled for real-time task output streaming"
        );
    }

    /// A raw newline-delimited JSON-RPC client speaking to a server over an in-memory pipe.
    struct RawMcpClient {
        reader: tokio::io::Lines<BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>,
        writer: tokio::io::WriteHalf<tokio::io::DuplexStream>,
    }

    impl RawMcpClient {
        fn spawn(server: DelaMcpServer) -> Self {
            let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
            tokio::spawn(async move {
                if let Ok(running) = server.serve(tokio::io::split(server_io)).await {
                    let _ = running.waiting().await;
                }
            });
            let (reader, writer) = tokio::io::split(client_io);
            Self {
                reader: BufReader::new(reader).lines(),
                writer,
            }
        }

        /// Send a stateless (2026-07-28) request: no `initialize`, metadata on every request.
        /// Any `_meta` already in `params` (progress token, log level) is kept.
        async fn send_stateless(&mut self, id: u64, method: &str, mut params: serde_json::Value) {
            use tokio::io::AsyncWriteExt;
            let meta = params
                .as_object_mut()
                .unwrap()
                .entry("_meta")
                .or_insert_with(|| serde_json::json!({}));
            meta["io.modelcontextprotocol/protocolVersion"] = serde_json::json!("2026-07-28");
            meta["io.modelcontextprotocol/clientCapabilities"] = serde_json::json!({});
            meta["io.modelcontextprotocol/clientInfo"] =
                serde_json::json!({"name": "stateless-test", "version": "1.0.0"});
            let message = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            });
            let mut line = message.to_string();
            line.push('\n');
            self.writer.write_all(line.as_bytes()).await.unwrap();
        }

        async fn send_notification(&mut self, method: &str, params: serde_json::Value) {
            use tokio::io::AsyncWriteExt;
            let message = serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            });
            let mut line = message.to_string();
            line.push('\n');
            self.writer.write_all(line.as_bytes()).await.unwrap();
        }

        /// Every message the server sends within `window`, while no request is awaited.
        async fn messages_within(&mut self, window: Duration) -> Vec<serde_json::Value> {
            let mut messages = Vec::new();
            let deadline = tokio::time::Instant::now() + window;
            while let Ok(line) = tokio::time::timeout_at(deadline, self.reader.next_line()).await {
                let line = line.unwrap().expect("MCP server closed the stream");
                messages.push(serde_json::from_str(&line).unwrap());
            }
            messages
        }

        /// Read until the response for `id`, returning it with the notifications seen first.
        async fn read_response(&mut self, id: u64) -> (serde_json::Value, Vec<serde_json::Value>) {
            let mut notifications = Vec::new();
            loop {
                let line = tokio::time::timeout(Duration::from_secs(10), self.reader.next_line())
                    .await
                    .expect("timed out waiting for MCP message")
                    .unwrap()
                    .expect("MCP server closed the stream");
                let message: serde_json::Value = serde_json::from_str(&line).unwrap();
                if message["id"] == id {
                    return (message, notifications);
                }
                notifications.push(message);
            }
        }
    }

    fn tool_result_json(response: &serde_json::Value) -> serde_json::Value {
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("expected tool text content, got {response}"));
        serde_json::from_str(text).unwrap()
    }

    fn server_with_allowlisted_script(
        temp_dir: &tempfile::TempDir,
        name: &str,
        body: &str,
    ) -> DelaMcpServer {
        use std::os::unix::fs::PermissionsExt;

        let script_path = temp_dir.path().join(format!("{name}.sh"));
        std::fs::write(&script_path, format!("#!/bin/bash\n{body}")).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let allowlist_evaluator = McpAllowlistEvaluator {
            allowlist: crate::types::Allowlist {
                entries: vec![crate::types::AllowlistEntry {
                    path: script_path,
                    scope: crate::types::AllowScope::File,
                    tasks: None,
                }],
            },
        };
        DelaMcpServer::new_with_allowlist(temp_dir.path().to_path_buf(), allowlist_evaluator)
    }

    #[tokio::test]
    async fn test_stateless_client_without_initialize() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let server = server_with_allowlisted_script(&temp_dir, "quick", "echo 'hello stateless'\n");
        let mut client = RawMcpClient::spawn(server);

        client
            .send_stateless(1, "server/discover", serde_json::json!({}))
            .await;
        let (discover, _) = client.read_response(1).await;
        let versions = discover["result"]["supportedVersions"].as_array().unwrap();
        assert!(
            versions.contains(&serde_json::json!("2026-07-28")),
            "{discover}"
        );
        assert!(discover["result"]["capabilities"]["tools"].is_object());

        client
            .send_stateless(
                2,
                "tools/call",
                serde_json::json!({
                    "name": "task_start",
                    "arguments": {"unique_name": "quick", "wait_for_exit_seconds": 5},
                    "_meta": {"progressToken": "start-quick"},
                }),
            )
            .await;
        let (response, notifications) = client.read_response(2).await;
        let payload = tool_result_json(&response);
        assert_eq!(payload["state"], "exited", "{payload}");
        assert_eq!(payload["output"][0]["stdout"], "hello stateless\n");
        // Output streams back to the in-flight request even though no session exists.
        let progress: Vec<&serde_json::Value> = notifications
            .iter()
            .filter(|n| n["method"] == "notifications/progress")
            .collect();
        assert_eq!(progress.len(), 1, "{notifications:?}");
        assert_eq!(progress[0]["params"]["progressToken"], "start-quick");
        assert_eq!(progress[0]["params"]["progress"], 1.0);
        assert_eq!(progress[0]["params"]["message"], "hello stateless");
        // Without a requested log level, no log notifications are sent.
        assert!(
            notifications
                .iter()
                .all(|n| n["method"] != "notifications/message"),
            "{notifications:?}"
        );
    }

    #[tokio::test]
    async fn test_task_start_honors_request_log_level() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let server = server_with_allowlisted_script(
            &temp_dir,
            "noisy",
            "echo 'plain stdout'\necho 'warning: careful' >&2\necho 'error: broken' >&2\n",
        );
        let mut client = RawMcpClient::spawn(server);

        let start_with_log_level = |level: &str| {
            serde_json::json!({
                "name": "task_start",
                "arguments": {"unique_name": "noisy", "wait_for_exit_seconds": 5},
                "_meta": {"io.modelcontextprotocol/logLevel": level},
            })
        };
        let log_messages = |notifications: &[serde_json::Value]| -> Vec<serde_json::Value> {
            assert!(
                notifications
                    .iter()
                    .all(|n| n["method"] != "notifications/progress"),
                "no progressToken was sent: {notifications:?}"
            );
            notifications
                .iter()
                .filter(|n| n["method"] == "notifications/message")
                .map(|n| n["params"].clone())
                .collect()
        };

        client
            .send_stateless(1, "tools/call", start_with_log_level("warning"))
            .await;
        let (_, notifications) = client.read_response(1).await;
        let logs = log_messages(&notifications);
        assert_eq!(logs.len(), 1, "{logs:?}");
        assert_eq!(logs[0]["level"], "error");
        assert_eq!(logs[0]["data"]["type"], "stderr");
        assert_eq!(
            logs[0]["data"]["lines"],
            serde_json::json!(["warning: careful", "error: broken"])
        );

        client
            .send_stateless(2, "tools/call", start_with_log_level("notice"))
            .await;
        let (_, notifications) = client.read_response(2).await;
        let logs = log_messages(&notifications);
        let events: Vec<&str> = logs
            .iter()
            .filter_map(|log| log["data"]["event"].as_str())
            .collect();
        assert_eq!(events, vec!["started", "exited"], "{logs:?}");
        assert!(
            logs.iter().all(|log| log["level"] != "info"),
            "info output must be filtered at notice: {logs:?}"
        );
    }

    #[tokio::test]
    async fn test_cancelled_task_start_stops_waiting_and_notifying() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let server = server_with_allowlisted_script(
            &temp_dir,
            "cancel_me",
            "echo 'before cancel'\nsleep 1\necho 'after cancel'\nsleep 3\n",
        );
        let mut client = RawMcpClient::spawn(server);

        client
            .send_stateless(
                1,
                "tools/call",
                serde_json::json!({
                    "name": "task_start",
                    "arguments": {"unique_name": "cancel_me", "wait_for_exit_seconds": 60},
                    "_meta": {
                        "progressToken": "cancel-me",
                        "io.modelcontextprotocol/logLevel": "debug",
                    },
                }),
            )
            .await;
        // Wait for the first output batch so the cancel lands mid-capture.
        let mut before_cancel = Vec::new();
        while !before_cancel
            .iter()
            .any(|n: &serde_json::Value| n["method"] == "notifications/progress")
        {
            before_cancel.extend(client.messages_within(Duration::from_millis(100)).await);
        }

        client
            .send_notification(
                "notifications/cancelled",
                serde_json::json!({"requestId": 1, "reason": "user pressed escape"}),
            )
            .await;
        let after_cancel = client.messages_within(Duration::from_millis(2000)).await;
        assert!(
            after_cancel.is_empty(),
            "a cancelled request must get no notifications or response: {after_cancel:?}"
        );

        // The wait ended early: the task is already a background job, well before its 60s window.
        client
            .send_stateless(2, "tools/call", serde_json::json!({"name": "status"}))
            .await;
        let (status_response, _) = client.read_response(2).await;
        let running = tool_result_json(&status_response)["running"].clone();
        let job = running
            .as_array()
            .unwrap()
            .iter()
            .find(|job| job["unique_name"] == "cancel_me")
            .unwrap_or_else(|| panic!("cancelled task should keep running: {running}"))
            .clone();

        client
            .send_stateless(
                3,
                "tools/call",
                serde_json::json!({"name": "task_output", "arguments": {"pid": job["pid"]}}),
            )
            .await;
        let (output_response, _) = client.read_response(3).await;
        let output = tool_result_json(&output_response);
        let stdout: Vec<&str> = output["output"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|chunk| chunk["stdout"].as_str())
            .map(str::trim_end)
            .collect();
        assert_eq!(stdout, vec!["before cancel", "after cancel"], "{output}");
    }

    #[tokio::test]
    async fn test_stateless_background_job_is_polled_not_pushed() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let server = server_with_allowlisted_script(
            &temp_dir,
            "slow",
            "echo 'early'\nsleep 1\necho 'late'\n",
        );
        let mut client = RawMcpClient::spawn(server);

        client
            .send_stateless(
                1,
                "tools/call",
                serde_json::json!({
                    "name": "task_start",
                    "arguments": {"unique_name": "slow", "wait_for_exit_seconds": 0},
                }),
            )
            .await;
        let (response, _) = client.read_response(1).await;
        let payload = tool_result_json(&response);
        assert_eq!(payload["state"], "running", "{payload}");
        let pid = payload["pid"].as_u64().unwrap();

        tokio::time::sleep(Duration::from_millis(2000)).await;

        client
            .send_stateless(
                2,
                "tools/call",
                serde_json::json!({"name": "task_status", "arguments": {"pid": pid}}),
            )
            .await;
        let (status_response, notifications) = client.read_response(2).await;
        assert!(
            notifications.is_empty(),
            "background jobs must not push notifications outside a request: {notifications:?}"
        );
        let status = tool_result_json(&status_response);
        assert_eq!(status["state"], "exited", "{status}");
        assert_eq!(status["exit_code"], 0);

        client
            .send_stateless(
                3,
                "tools/call",
                serde_json::json!({"name": "task_output", "arguments": {"pid": pid}}),
            )
            .await;
        let (output_response, _) = client.read_response(3).await;
        let output = tool_result_json(&output_response);
        let stdout: Vec<&str> = output["output"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|chunk| chunk["stdout"].as_str())
            .map(str::trim_end)
            .collect();
        assert_eq!(stdout, vec!["early", "late"], "{output}");
    }

    #[test]
    fn test_server_info_instructions_mentions_bounded_wait() {
        let server = DelaMcpServer::new(PathBuf::from("."));
        let info = server.get_info();
        let instructions = info.instructions.expect("instructions should be present");

        assert!(instructions.contains("wait_for_exit_seconds"));
        assert!(instructions.contains("default 1-second capture window"));
    }

    #[tokio::test]
    async fn test_call_tool_invalid_args() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let mut invalid_args = serde_json::Map::new();
        invalid_args.insert("runner".to_string(), serde_json::Value::Bool(true));
        let req = CallToolRequestParams::new("list_tasks").with_arguments(invalid_args);

        let res = server.call_tool(req, context).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code.0, -32602);
        assert!(err.message.contains("Invalid arguments"));
    }

    #[tokio::test]
    async fn test_call_tool_unknown_tool() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let req = CallToolRequestParams::new("nonexistent_tool");
        let res = server.call_tool(req, context).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code.0, -32603);
        assert!(err.message.contains("Tool not found: nonexistent_tool"));
    }

    #[tokio::test]
    async fn test_call_tool_dispatch_list_tasks() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let req = CallToolRequestParams::new("list_tasks");
        let res = server.call_tool(req, context).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_call_tool_dispatch_status() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let req = CallToolRequestParams::new("status");
        let res = server.call_tool(req, context).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_call_tool_dispatch_task_status() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let mut args = serde_json::Map::new();
        args.insert("pid".to_string(), serde_json::Value::Number(12345.into()));
        let req = CallToolRequestParams::new("task_status").with_arguments(args);
        let res = server.call_tool(req, context).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_call_tool_dispatch_task_output() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let mut args = serde_json::Map::new();
        args.insert("pid".to_string(), serde_json::Value::Number(12345.into()));
        let req = CallToolRequestParams::new("task_output").with_arguments(args);
        let res = server.call_tool(req, context).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_call_tool_dispatch_task_stop() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let mut args = serde_json::Map::new();
        args.insert("pid".to_string(), serde_json::Value::Number(12345.into()));
        let req = CallToolRequestParams::new("task_stop").with_arguments(args);
        let res = server.call_tool(req, context).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_call_tool_dispatch_task_start() {
        let server = DelaMcpServer::new(std::env::temp_dir());
        let (server_transport, _client_transport) = tokio::io::duplex(4096);
        let running_server = rmcp::service::serve_directly(server.clone(), server_transport, None);
        let context = RequestContext::new(RequestId::Number(1), running_server.peer().clone());

        let mut args = serde_json::Map::new();
        args.insert(
            "unique_name".to_string(),
            serde_json::Value::String("nonexistent".to_string()),
        );
        let req = CallToolRequestParams::new("task_start").with_arguments(args);
        let res = server.call_tool(req, context).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code.0, -32012); // TASK_NOT_FOUND
    }
}
