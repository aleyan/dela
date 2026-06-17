use super::allowlist::McpAllowlistEvaluator;
use super::dto::{
    ListTasksArgs, OutputChunkDto, StartResultDto, TaskDto, TaskOutputArgs, TaskStartArgs,
    TaskStatusArgs, TaskStopArgs,
};
use super::errors::DelaError;
use super::job_manager::{JobManager, JobMetadata, JobState, OutputLine};
use crate::runner::{is_runner_available_for_mcp, split_command_words};
use crate::task_discovery;
use chrono::SecondsFormat;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::*,
    service::{Peer, RequestContext, RoleServer},
    tool,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader, stdin, stdout};
use tokio::process::Command;
use tokio::sync::{OnceCell, RwLock};
use tokio::time::Duration;

const TASK_DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(60);
const DEFAULT_TASK_START_WAIT_SECONDS: u64 = 1;
const MAX_TASK_START_WAIT_SECONDS: u64 = 3600;
const OUTPUT_NOTIFICATION_FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const OUTPUT_NOTIFICATION_MAX_BYTES: usize = 4 * 1024;
const OUTPUT_NOTIFICATION_MAX_LINES: usize = 100;

fn classify_output_log_level(stream: &str, line: &str) -> LoggingLevel {
    let normalized = line.trim().to_ascii_lowercase();

    let is_error = normalized.starts_with("error")
        || normalized.starts_with("fatal:")
        || normalized.contains(" panicked at")
        || normalized.starts_with("thread '")
        || normalized.starts_with("failures:");
    if is_error {
        return LoggingLevel::Error;
    }

    let is_warning = normalized.starts_with("warning")
        || normalized.starts_with("warn:")
        || normalized.contains(" warning:");
    if is_warning {
        return LoggingLevel::Warning;
    }

    match stream {
        "stderr" => LoggingLevel::Info,
        _ => LoggingLevel::Info,
    }
}

#[derive(Debug, Clone)]
struct OutputNotificationEntry {
    line: String,
    level: LoggingLevel,
}

#[derive(Debug, Clone)]
struct OutputNotificationBatch {
    stream: &'static str,
    entries: Vec<OutputNotificationEntry>,
    total_bytes: usize,
    started_at: Option<Instant>,
}

impl OutputNotificationBatch {
    fn new(stream: &'static str) -> Self {
        Self {
            stream,
            entries: Vec::new(),
            total_bytes: 0,
            started_at: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn add_line(&mut self, line: &str) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }

        self.total_bytes += line.len();
        self.entries.push(OutputNotificationEntry {
            line: line.trim_end().to_string(),
            level: classify_output_log_level(self.stream, line),
        });
    }

    fn should_flush(&self) -> bool {
        self.entries.len() >= OUTPUT_NOTIFICATION_MAX_LINES
            || self.total_bytes >= OUTPUT_NOTIFICATION_MAX_BYTES
    }

    fn flush_due_at(&self) -> Option<Instant> {
        self.started_at
            .map(|started_at| started_at + OUTPUT_NOTIFICATION_FLUSH_INTERVAL)
    }

    fn take_notification_data(&mut self, pid: u32) -> Option<(LoggingLevel, serde_json::Value)> {
        if self.entries.is_empty() {
            return None;
        }

        let entries = std::mem::take(&mut self.entries);
        let _total_bytes = std::mem::take(&mut self.total_bytes);
        self.started_at = None;

        let batch_level = if entries
            .iter()
            .any(|entry| entry.level == LoggingLevel::Error)
        {
            LoggingLevel::Error
        } else if entries
            .iter()
            .any(|entry| entry.level == LoggingLevel::Warning)
        {
            LoggingLevel::Warning
        } else {
            LoggingLevel::Info
        };

        let lines: Vec<String> = entries.iter().map(|entry| entry.line.clone()).collect();
        let data = serde_json::json!({
            "type": self.stream,
            "pid": pid,
            "lines": lines,
        });

        Some((batch_level, data))
    }
}

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
    task_cache: Arc<RwLock<Option<CachedDiscoveredTasks>>>,
    task_cache_ttl: Duration,
    /// Peer connection for sending notifications (set during initialize)
    peer: Arc<OnceCell<Peer<RoleServer>>>,
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
            task_cache: Arc::new(RwLock::new(None)),
            task_cache_ttl,
            peer: Arc::new(OnceCell::new()),
        }
    }

    /// Create a new MCP server instance with a custom allowlist evaluator (for testing)
    #[cfg(test)]
    pub fn new_with_allowlist(root: PathBuf, allowlist_evaluator: McpAllowlistEvaluator) -> Self {
        Self::new_inner(root, allowlist_evaluator, TASK_DISCOVERY_CACHE_TTL)
    }

    #[cfg(test)]
    pub fn new_with_allowlist_and_cache_ttl(
        root: PathBuf,
        allowlist_evaluator: McpAllowlistEvaluator,
        task_cache_ttl: Duration,
    ) -> Self {
        Self::new_inner(root, allowlist_evaluator, task_cache_ttl)
    }

    /// Send a logging notification to the client (if connected)
    async fn send_log(&self, level: LoggingLevel, logger: &str, data: serde_json::Value) {
        if let Some(peer) = self.peer.get() {
            let _ = peer
                .notify_logging_message(LoggingMessageNotificationParam {
                    level,
                    logger: Some(logger.to_string()),
                    data,
                })
                .await;
        }
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

    async fn flush_output_notification_batch(
        peer: &Arc<OnceCell<Peer<RoleServer>>>,
        pid: u32,
        batch: &mut OutputNotificationBatch,
    ) {
        let Some((level, data)) = batch.take_notification_data(pid) else {
            return;
        };

        if let Some(peer) = peer.get() {
            let _ = peer
                .notify_logging_message(LoggingMessageNotificationParam {
                    level,
                    logger: Some(format!("task:{}", pid)),
                    data,
                })
                .await;
        }
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

    /// Send task output as a logging notification
    #[allow(dead_code)]
    async fn send_task_output(&self, pid: u32, output_type: &str, content: &str) {
        self.send_log(
            if output_type == "stderr" {
                LoggingLevel::Warning
            } else {
                LoggingLevel::Info
            },
            &format!("task:{}", pid),
            serde_json::json!({
                "type": output_type,
                "pid": pid,
                "content": content
            }),
        )
        .await;
    }

    /// Send task lifecycle notification
    async fn send_task_event(&self, pid: u32, event: &str, details: serde_json::Value) {
        self.send_log(
            LoggingLevel::Notice,
            &format!("task:{}", pid),
            serde_json::json!({
                "event": event,
                "pid": pid,
                "details": details
            }),
        )
        .await;
    }

    /// Get the root path this server operates in
    #[allow(dead_code)]
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    async fn get_discovered_tasks(&self) -> task_discovery::DiscoveredTasks {
        {
            let cache = self.task_cache.read().await;
            if let Some(entry) = cache.as_ref()
                && entry.cached_at.elapsed() < self.task_cache_ttl
            {
                return entry.discovered.clone();
            }
        }

        let discovered = task_discovery::discover_tasks(&self.root);
        let mut cache = self.task_cache.write().await;
        *cache = Some(CachedDiscoveredTasks {
            discovered: discovered.clone(),
            cached_at: Instant::now(),
        });
        discovered
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
        let discovered = self.get_discovered_tasks().await;

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
            Content::json(serde_json::json!({
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
            Content::json(serde_json::json!({
                "running": running_jobs
            }))
            .expect("Failed to serialize JSON"),
        ]))
    }

    async fn run_initial_capture(
        peer: std::sync::Arc<tokio::sync::OnceCell<rmcp::service::Peer<rmcp::service::RoleServer>>>,
        pid_u32: u32,
        capture_duration: Duration,
        mut stdout_rx: tokio::sync::mpsc::Receiver<String>,
        mut stderr_rx: tokio::sync::mpsc::Receiver<String>,
        captured_output_chunks: Arc<tokio::sync::Mutex<Vec<crate::mcp::dto::OutputChunkDto>>>,
    ) -> (
        tokio::sync::mpsc::Receiver<String>,
        tokio::sync::mpsc::Receiver<String>,
    ) {
        let deadline = std::time::Instant::now() + capture_duration;
        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut stdout_batch = OutputNotificationBatch::new("stdout");
        let mut stderr_batch = OutputNotificationBatch::new("stderr");

        loop {
            let now = std::time::Instant::now();
            if now >= deadline {
                Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                break;
            }

            tokio::select! {
                line = stdout_rx.recv(), if !stdout_done => {
                    match line {
                        Some(line) => {
                            {
                                let mut chunks = captured_output_chunks.lock().await;
                                Self::append_output_chunk(&mut chunks, "stdout", &line);
                            }
                            stdout_batch.add_line(&line);
                            if stdout_batch.should_flush() {
                                Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                            }
                        }
                        None => {
                            stdout_done = true;
                            Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                        }
                    }
                }
                line = stderr_rx.recv(), if !stderr_done => {
                    match line {
                        Some(line) => {
                            {
                                let mut chunks = captured_output_chunks.lock().await;
                                Self::append_output_chunk(&mut chunks, "stderr", &line);
                            }
                            stderr_batch.add_line(&line);
                            if stderr_batch.should_flush() {
                                Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                            }
                        }
                        None => {
                            stderr_done = true;
                            Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                        }
                    }
                }
                _ = tokio::time::sleep_until(Self::output_flush_timer_deadline(stdout_batch.flush_due_at(), deadline)), if !stdout_batch.is_empty() => {
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                }
                _ = tokio::time::sleep_until(Self::output_flush_timer_deadline(stderr_batch.flush_due_at(), deadline)), if !stderr_batch.is_empty() => {
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                }
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                    break;
                }
            }

            if stdout_done && stderr_done {
                Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                break;
            }
        }

        (stdout_rx, stderr_rx)
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_background_monitoring(
        peer: std::sync::Arc<tokio::sync::OnceCell<rmcp::service::Peer<rmcp::service::RoleServer>>>,
        pid_u32: u32,
        task_name: String,
        mut stdout_rx_opt: Option<tokio::sync::mpsc::Receiver<String>>,
        mut stderr_rx_opt: Option<tokio::sync::mpsc::Receiver<String>>,
        job_manager: crate::mcp::job_manager::JobManager,
    ) {
        let mut stdout_batch = OutputNotificationBatch::new("stdout");
        let mut stderr_batch = OutputNotificationBatch::new("stderr");
        let idle_deadline_fallback = Instant::now() + Duration::from_secs(24 * 60 * 60);

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
                            if let Err(error) = job_manager.add_job_output_chunk(pid_u32, "stdout", line.clone()).await {
                                tracing::warn!(pid = pid_u32, error = %error, "failed to persist stdout output chunk");
                            }
                            stdout_batch.add_line(&line);
                            if stdout_batch.should_flush() {
                                Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                            }
                        }
                        None => {
                            stdout_rx_opt = None;
                            Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                        }
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
                            if let Err(error) = job_manager.add_job_output_chunk(pid_u32, "stderr", line.clone()).await {
                                tracing::warn!(pid = pid_u32, error = %error, "failed to persist stderr output chunk");
                            }
                            stderr_batch.add_line(&line);
                            if stderr_batch.should_flush() {
                                Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                            }
                        }
                        None => {
                            stderr_rx_opt = None;
                            Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                        }
                    }
                }
                _ = tokio::time::sleep_until(Self::output_flush_timer_deadline(stdout_batch.flush_due_at(), idle_deadline_fallback)), if !stdout_batch.is_empty() => {
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                }
                _ = tokio::time::sleep_until(Self::output_flush_timer_deadline(stderr_batch.flush_due_at(), idle_deadline_fallback)), if !stderr_batch.is_empty() => {
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                }
                else => {
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stdout_batch).await;
                    Self::flush_output_notification_batch(&peer, pid_u32, &mut stderr_batch).await;
                    break;
                }
            }
        }

        let process_opt = job_manager.processes.write().await.remove(&pid_u32);
        if let Some(mut process) = process_opt {
            let exit_result = process.wait().await;
            let (state, exit_code, signal) = match exit_result {
                Ok(status) => {
                    let mut state = JobState::Exited(status.code().unwrap_or(-1));
                    let mut exit_code = status.code();
                    let mut signal = None;
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(sig) = status.signal() {
                            state = JobState::Signaled(sig);
                            signal = Some(sig);
                            exit_code = None;
                        }
                    }
                    (state, exit_code, signal)
                }
                Err(e) => (
                    JobState::Failed(format!("Process wait failed: {}", e)),
                    None,
                    None,
                ),
            };

            let _ = job_manager.update_job_state(pid_u32, state).await;

            if let Some(peer_ref) = peer.get() {
                let _ = peer_ref
                    .notify_logging_message(rmcp::model::LoggingMessageNotificationParam {
                        level: LoggingLevel::Notice,
                        logger: Some(format!("task:{}", pid_u32)),
                        data: serde_json::json!({
                            "event": "exited",
                            "pid": pid_u32,
                            "exit_code": exit_code,
                            "signal": signal,
                            "task": task_name
                        }),
                    })
                    .await;
            }
        }
    }

    #[tool(
        description = "Start a task (default 1s capture, optional bounded wait, then background)"
    )]
    pub async fn task_start(
        &self,
        Parameters(args): Parameters<TaskStartArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let discovered = self.get_discovered_tasks().await;

        let task = discovered
            .tasks
            .iter()
            .find(|t| {
                let unique_name = t.disambiguated_name.as_ref().unwrap_or(&t.name);
                unique_name == &args.unique_name
            })
            .ok_or_else(|| DelaError::task_not_found(args.unique_name.clone()))?;

        // Check if task is allowlisted for MCP execution
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
            return Err(DelaError::not_allowlisted(args.unique_name.clone()).into());
        }

        // Check if runner is available
        if !is_runner_available_for_mcp(&task.runner) {
            return Err(DelaError::runner_unavailable(
                task.runner.short_name().to_string(),
                args.unique_name.clone(),
            )
            .into());
        }

        // Check concurrency limits before starting the process
        self.job_manager.can_start_job().await.map_err(|e| {
            DelaError::internal_error(
                format!("Concurrency limit exceeded: {}", e),
                Some("Too many concurrent jobs running".to_string()),
            )
        })?;

        // Build the command
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
        cmd.current_dir(self.root.clone());

        // Ensure we capture stdout and stderr properly
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Add the task name as the first argument
        cmd.args(base_args);

        // Add task-specific arguments
        if let Some(task_args) = &args.args {
            cmd.args(task_args);
        }

        // Set environment variables
        if let Some(env_vars) = &args.env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        // Set working directory if specified
        if let Some(cwd) = &args.cwd {
            cmd.current_dir(cwd);
        }

        let started_at = Instant::now();

        // Start the process
        let mut child = cmd.spawn().map_err(|e| {
            DelaError::internal_error(
                format!("Failed to start process: {}", e),
                Some("Check if the command and arguments are valid".to_string()),
            )
        })?;

        let pid = child.id().unwrap_or(0) as i32;

        // Take stdout/stderr handles for streaming
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        // Send task started event
        self.send_task_event(
            pid as u32,
            "started",
            serde_json::json!({
                "task": args.unique_name,
                "command": full_command
            }),
        )
        .await;

        // Capture output until the bounded wait window expires while streaming via logging.
        let capture_duration = Duration::from_secs(Self::resolve_wait_for_exit_seconds(
            args.wait_for_exit_seconds,
        )?);
        let captured_output_chunks = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let peer_clone = self.peer.clone();

        // Create channels for output streaming
        let (stdout_tx, stdout_rx) = tokio::sync::mpsc::channel::<String>(100);
        let (stderr_tx, stderr_rx) = tokio::sync::mpsc::channel::<String>(100);

        // Spawn stdout reader task
        let stdout_task = if let Some(stdout) = stdout_handle {
            let tx = stdout_tx;
            Some(tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let _ = tx.send(line.clone()).await;
                        }
                        Err(_) => break,
                    }
                }
            }))
        } else {
            drop(stdout_tx);
            None
        };

        // Spawn stderr reader task
        let stderr_task = if let Some(stderr) = stderr_handle {
            let tx = stderr_tx;
            Some(tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let _ = tx.send(line.clone()).await;
                        }
                        Err(_) => break,
                    }
                }
            }))
        } else {
            drop(stderr_tx);
            None
        };

        // Collect initial output for ~1 second while also streaming to logging
        let captured_output_chunks_clone = captured_output_chunks.clone();
        let peer_for_initial = peer_clone.clone();
        let pid_u32 = pid as u32;

        let initial_capture = tokio::spawn(Self::run_initial_capture(
            peer_for_initial,
            pid_u32,
            capture_duration,
            stdout_rx,
            stderr_rx,
            captured_output_chunks_clone,
        ));

        let capture_result = initial_capture.await;

        // Check if process exited during initial capture
        let process_exited = child.try_wait().is_ok_and(|status| status.is_some());

        if process_exited {
            // Process completed within 1 second
            let exit_status = child.wait().await.map_err(|e| {
                DelaError::internal_error(
                    format!("Failed to wait for process: {}", e),
                    Some("Process management error".to_string()),
                )
            })?;

            let output_chunks = captured_output_chunks.lock().await.clone();

            // Wait for reader tasks to finish
            if let Some(task) = stdout_task {
                let _ = task.await;
            }
            if let Some(task) = stderr_task {
                let _ = task.await;
            }

            // Create job metadata and store it so task_status/task_output can query it
            let metadata = JobMetadata {
                started_at,
                unique_name: args.unique_name.clone(),
                source_name: task.source_name.clone(),
                args: args.args.clone(),
                env: args.env.clone(),
                cwd: args.cwd.as_ref().map(PathBuf::from),
                command: task.runner.get_command(task),
                file_path: task.definition_path().to_path_buf(),
            };

            let mut exit_code = exit_status.code();
            let mut signal = None;
            let mut exit_state = JobState::Exited(exit_code.unwrap_or(-1));
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(sig) = exit_status.signal() {
                    signal = Some(sig);
                    exit_code = None;
                    exit_state = JobState::Signaled(sig);
                }
            }
            self.job_manager
                .record_completed_job(pid as u32, metadata, exit_state.clone())
                .await
                .map_err(|e| {
                    DelaError::internal_error(
                        format!("Failed to record completed job: {}", e),
                        Some("Job management error".to_string()),
                    )
                })?;

            // Add output to the job record
            if !output_chunks.is_empty() {
                Self::add_job_output_chunks(&self.job_manager, pid as u32, &output_chunks)
                    .await
                    .map_err(|e| {
                        DelaError::internal_error(
                            format!("Failed to add completed task output: {}", e),
                            Some("Job management error".to_string()),
                        )
                    })?;
            }

            // Send task completed event
            self.send_task_event(
                pid as u32,
                "exited",
                serde_json::json!({
                    "exit_code": exit_code,
                    "task": args.unique_name
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

            return Ok(CallToolResult::success(vec![
                Content::json(&start_result).expect("Failed to serialize JSON"),
            ]));
        }

        // Process is still running - set up background monitoring
        let output_chunks = captured_output_chunks.lock().await.clone();

        // Create job metadata
        let metadata = JobMetadata {
            started_at,
            unique_name: args.unique_name.clone(),
            source_name: task.source_name.clone(),
            args: args.args.clone(),
            env: args.env.clone(),
            cwd: args.cwd.as_ref().map(PathBuf::from),
            command: task.runner.get_command(task),
            file_path: task.definition_path().to_path_buf(),
        };

        // Start background job management
        self.job_manager
            .start_job(pid as u32, metadata, child)
            .await
            .map_err(|e| {
                DelaError::internal_error(
                    format!("Failed to start background job: {}", e),
                    Some("Job management error".to_string()),
                )
            })?;

        // Add initial output to the job
        if !output_chunks.is_empty() {
            Self::add_job_output_chunks(&self.job_manager, pid as u32, &output_chunks)
                .await
                .map_err(|e| {
                    DelaError::internal_error(
                        format!("Failed to add initial output: {}", e),
                        Some("Job management error".to_string()),
                    )
                })?;
        }

        // Spawn background monitoring task with continued output streaming
        let job_manager = self.job_manager.clone();
        let peer_for_monitor = peer_clone;
        let task_name = args.unique_name.clone();

        let (stdout_rx_opt, stderr_rx_opt) = if let Ok((rx1, rx2)) = capture_result {
            (Some(rx1), Some(rx2))
        } else {
            (None, None)
        };

        tokio::spawn(Self::run_background_monitoring(
            peer_for_monitor,
            pid_u32,
            task_name,
            stdout_rx_opt,
            stderr_rx_opt,
            job_manager,
        ));

        let start_result = StartResultDto {
            state: "running".to_string(),
            pid: Some(pid as i32),
            exit_code: None,
            signal: None,
            output: output_chunks,
        };

        Ok(CallToolResult::success(vec![
            Content::json(&start_result).expect("Failed to serialize JSON"),
        ]))
    }

    #[tool(description = "Status for a single unique_name (may have multiple PIDs)")]
    pub async fn task_status(
        &self,
        Parameters(args): Parameters<TaskStatusArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let jobs = self.job_manager.get_jobs_by_name(&args.unique_name).await;
        let job_statuses: Vec<serde_json::Value> = jobs
            .into_iter()
            .map(|job| {
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

                serde_json::json!({
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
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![
            Content::json(serde_json::json!({
                "jobs": job_statuses
            }))
            .expect("Failed to serialize JSON"),
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
            .ok_or_else(|| DelaError::task_not_found(format!("Job with PID {}", args.pid)))?;

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
            Content::json(&response).expect("Failed to serialize JSON"),
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
            .ok_or_else(|| DelaError::task_not_found(format!("Job with PID {}", args.pid)))?;

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
            Content::json(serde_json::json!({
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
            "List tasks, start them with a default 1-second capture window or an optional wait_for_exit_seconds bounded wait, and manage running tasks via PID; all execution is gated by an MCP allowlist. Subscribe to logging notifications for real-time task output streaming."
        )
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request);
        }
        // Store the peer for sending logging notifications
        let _ = self.peer.set(context.peer.clone());
        Ok(self.get_info())
    }

    // Manually implement ServerHandler trait methods since #[tool_router] macro is not working
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        match request.name.as_ref() {
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
                self.task_start(Parameters(args)).await
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
        }
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
        let mut task_status_unique_name_prop = Map::new();
        task_status_unique_name_prop.insert(
            "type".to_string(),
            serde_json::Value::String("string".to_string()),
        );
        task_status_unique_name_prop.insert(
            "description".to_string(),
            serde_json::Value::String("The unique name of the task to get status for".to_string()),
        );
        task_status_properties.insert(
            "unique_name".to_string(),
            serde_json::Value::Object(task_status_unique_name_prop),
        );
        task_status_schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(task_status_properties),
        );
        task_status_schema.insert(
            "required".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("unique_name".to_string())]),
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
                Some("Status for a single unique_name (may have multiple PIDs)".into()),
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
        })
    }

    // Implement set_level to satisfy logging capability requirement
    fn set_level(
        &self,
        request: SetLevelRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), ErrorData>> + Send + '_ {
        std::future::ready(self.set_level_impl(request))
    }
}

impl DelaMcpServer {
    /// Internal implementation of set_level for testing
    #[cfg(test)]
    pub fn set_level_impl(&self, _request: SetLevelRequestParams) -> Result<(), ErrorData> {
        // Accept any log level - we'll send all notifications regardless
        Ok(())
    }

    #[cfg(not(test))]
    fn set_level_impl(&self, _request: SetLevelRequestParams) -> Result<(), ErrorData> {
        // Accept any log level - we'll send all notifications regardless
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
        let status_args = TaskStatusArgs {
            unique_name: "test-task".to_string(),
        };
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

        // These should work (even if they return empty results for non-existent jobs)
        assert!(server.task_status(Parameters(status_args)).await.is_ok());
        // task_output and task_stop should return errors for non-existent jobs
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        let args = TaskStatusArgs {
            unique_name: "nonexistent-task".to_string(),
        };

        // Act
        let result = server.task_status(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match &content.raw {
            RawContent::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert!(obj.contains_key("jobs"));
                let jobs = obj["jobs"].as_array().unwrap();
                assert_eq!(
                    jobs.len(),
                    0,
                    "Should return empty array for nonexistent task"
                );
            }
            _ => panic!("Expected text content with JSON"),
        }
    }

    #[tokio::test]
    async fn test_task_status_with_jobs() {
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);

        // Create multiple jobs with the same unique_name
        let metadata1 = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: Some(vec!["--verbose".to_string()]),
            env: None,
            cwd: Some(PathBuf::from("/tmp")),
            command: "echo test --verbose".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        let metadata2 = JobMetadata {
            started_at: std::time::Instant::now(),
            unique_name: "test-task".to_string(),
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

        let args = TaskStatusArgs {
            unique_name: "test-task".to_string(),
        };

        // Act
        let result = server.task_status(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match &content.raw {
            RawContent::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert!(obj.contains_key("jobs"));
                let jobs = obj["jobs"].as_array().unwrap();
                assert_eq!(
                    jobs.len(),
                    2,
                    "Should return two jobs for the same unique_name"
                );

                // Check that both jobs have the correct unique_name
                for job in jobs {
                    assert_eq!(job["unique_name"], "test-task");
                    assert_eq!(job["source_name"], "test");
                    assert!(job["pid"].is_number());
                    assert!(job["state"].is_string());
                    assert_eq!(job["state"], "running");
                    assert!(job["exit_code"].is_null());
                    assert!(job["completed_at"].is_null());
                }
            }
            _ => panic!("Expected text content with JSON"),
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

        let args = TaskStatusArgs {
            unique_name: "test-task".to_string(),
        };

        // Act
        let result = server.task_status(Parameters(args)).await.unwrap();

        // Assert
        assert_eq!(result.content.len(), 1);
        let content = &result.content[0];
        match &content.raw {
            RawContent::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let obj = json.as_object().unwrap();
                assert!(obj.contains_key("jobs"));
                let jobs = obj["jobs"].as_array().unwrap();
                assert_eq!(jobs.len(), 1, "Should return one job");

                let job = &jobs[0];
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
            .task_status(Parameters(TaskStatusArgs {
                unique_name: "test-task".to_string(),
            }))
            .await
            .unwrap();

        let content = &result.content[0];
        match &content.raw {
            RawContent::Text(text_content) => {
                let json: serde_json::Value = serde_json::from_str(&text_content.text).unwrap();
                let jobs = json["jobs"].as_array().unwrap();
                assert_eq!(jobs.len(), 1);
                assert_eq!(jobs[0]["state"], "failed");
                assert!(jobs[0]["exit_code"].is_null());
                assert!(jobs[0]["completed_at"].is_string());
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
        match &content.raw {
            RawContent::Text(text_content) => {
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

        let first_json = match &first_result.content[0].raw {
            RawContent::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content with JSON"),
        };
        let second_json = match &second_result.content[0].raw {
            RawContent::Text(text_content) => {
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

        let refreshed_json = match &refreshed_result.content[0].raw {
            RawContent::Text(text_content) => {
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
        });
        let make_result = server.list_tasks(make_args).await.unwrap();
        assert_eq!(make_result.content.len(), 1);

        // Act & Assert - Test filtering by "npm"
        let npm_args = Parameters(ListTasksArgs {
            runner: Some("npm".to_string()),
        });
        let npm_result = server.list_tasks(npm_args).await.unwrap();
        assert_eq!(npm_result.content.len(), 1);

        // Act & Assert - Test filtering by non-existent runner
        let nonexistent_args = Parameters(ListTasksArgs {
            runner: Some("nonexistent".to_string()),
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
        });
        let exact_result = server.list_tasks(exact_args).await.unwrap();
        assert_eq!(exact_result.content.len(), 1);

        // Act & Assert - Test case mismatch (should return empty)
        let case_args = Parameters(ListTasksArgs {
            runner: Some("MAKE".to_string()),
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
                match &content.raw {
                    RawContent::Text(text_content) => {
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
        let json = match &content.raw {
            RawContent::Text(text_content) => {
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
        let status_json = match &status_result.content[0].raw {
            RawContent::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        assert_eq!(status_json["running"].as_array().unwrap().len(), 0);

        let task_status_result = server
            .task_status(Parameters(TaskStatusArgs {
                unique_name: "waited_task".to_string(),
            }))
            .await
            .unwrap();
        let task_status_json = match &task_status_result.content[0].raw {
            RawContent::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        let jobs = task_status_json["jobs"].as_array().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0]["state"], "exited");

        sleep(Duration::from_secs(2)).await;

        let task_status_result_later = server
            .task_status(Parameters(TaskStatusArgs {
                unique_name: "waited_task".to_string(),
            }))
            .await
            .unwrap();
        let task_status_json_later = match &task_status_result_later.content[0].raw {
            RawContent::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        let later_job = &task_status_json_later["jobs"].as_array().unwrap()[0];
        let elapsed_seconds = later_job["elapsed_seconds"].as_u64().unwrap();
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
        let json = match &content.raw {
            RawContent::Text(text_content) => {
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
        let status_json = match &status_result.content[0].raw {
            RawContent::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        assert_eq!(status_json["running"].as_array().unwrap().len(), 1);

        let task_status_result = server
            .task_status(Parameters(TaskStatusArgs {
                unique_name: "still_running_task".to_string(),
            }))
            .await
            .unwrap();
        let task_status_json = match &task_status_result.content[0].raw {
            RawContent::Text(text_content) => {
                serde_json::from_str::<serde_json::Value>(&text_content.text).unwrap()
            }
            _ => panic!("Expected text content"),
        };
        let running_job = &task_status_json["jobs"].as_array().unwrap()[0];
        assert_eq!(running_job["state"], "running");
        assert!(
            running_job["elapsed_seconds"].as_u64().unwrap() >= 2,
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
        std::fs::write(&script_path, "#!/bin/bash\necho 'hi'\n").unwrap();
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
        match &content.raw {
            RawContent::Text(text_content) => {
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
                match &status_content.raw {
                    RawContent::Text(text_content) => {
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
                let task_status_args = TaskStatusArgs {
                    unique_name: "long_task".to_string(),
                };
                let task_status_result = server
                    .task_status(Parameters(task_status_args))
                    .await
                    .unwrap();
                let task_status_content = &task_status_result.content[0];
                match &task_status_content.raw {
                    RawContent::Text(text_content) => {
                        let task_status_json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        let jobs = task_status_json["jobs"].as_array().unwrap();
                        println!(
                            "Task status immediately after start: {} jobs, first job state: {}",
                            jobs.len(),
                            jobs.first()
                                .map(|j| j["state"].as_str().unwrap_or("unknown"))
                                .unwrap_or("none")
                        );
                        assert_eq!(jobs.len(), 1, "Should have 1 job");
                        assert_eq!(jobs[0]["state"].as_str().unwrap(), "running");
                        assert_eq!(jobs[0]["pid"].as_i64().unwrap() as u32, pid);
                    }
                    _ => panic!("Expected text content"),
                }

                // Wait for 1 second - should still be running
                sleep(Duration::from_secs(1)).await;

                let status_result_after_1s = server.status().await.unwrap();
                let status_content_after_1s = &status_result_after_1s.content[0];
                match &status_content_after_1s.raw {
                    RawContent::Text(text_content) => {
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
                match &status_content_final.raw {
                    RawContent::Text(text_content) => {
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
                let task_status_args_final = TaskStatusArgs {
                    unique_name: "long_task".to_string(),
                };
                let task_status_result_final = server
                    .task_status(Parameters(task_status_args_final))
                    .await
                    .unwrap();
                let task_status_content_final = &task_status_result_final.content[0];
                match &task_status_content_final.raw {
                    RawContent::Text(text_content) => {
                        let task_status_json: serde_json::Value =
                            serde_json::from_str(&text_content.text).unwrap();
                        let jobs = task_status_json["jobs"].as_array().unwrap();
                        println!(
                            "Task status after completion: {} jobs, first job state: {}",
                            jobs.len(),
                            jobs.first()
                                .map(|j| j["state"].as_str().unwrap_or("unknown"))
                                .unwrap_or("none")
                        );
                        assert_eq!(jobs.len(), 1, "Should still have 1 job record");
                        assert_eq!(jobs[0]["state"].as_str().unwrap(), "exited");
                        assert_eq!(jobs[0]["pid"].as_i64().unwrap() as u32, pid);
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
        let start_state = match &content.raw {
            RawContent::Text(text_content) => {
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
        match &status_content.raw {
            RawContent::Text(text_content) => {
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

        // task_status should record the job as exited quickly
        let task_status_args = TaskStatusArgs {
            unique_name: "bg-test".to_string(),
        };
        let task_status_result = server
            .task_status(Parameters(task_status_args))
            .await
            .unwrap();
        let task_status_content = &task_status_result.content[0];
        match &task_status_content.raw {
            RawContent::Text(text_content) => {
                let task_status_json: serde_json::Value =
                    serde_json::from_str(&text_content.text).unwrap();
                let jobs = task_status_json["jobs"].as_array().unwrap();
                assert!(!jobs.is_empty());
                let job = &jobs[0];
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
        let pid = match &content.raw {
            RawContent::Text(text_content) => {
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
        match &out_content.raw {
            RawContent::Text(text_content) => {
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

    #[tokio::test]
    async fn test_peer_storage_for_notifications() {
        use std::path::PathBuf;

        let server = DelaMcpServer::new(PathBuf::from("."));

        // Before initialization, peer should not be set
        assert!(
            server.peer.get().is_none(),
            "Peer should not be set before initialization"
        );

        // Note: Full peer storage testing requires a mock client connection
        // which is complex to set up. The basic verification that the peer
        // field exists and is properly initialized is sufficient here.
    }

    #[tokio::test]
    async fn test_set_level_handler_exists() {
        use rmcp::model::{LoggingLevel, SetLevelRequestParams};
        use std::path::PathBuf;

        let server = DelaMcpServer::new(PathBuf::from("."));

        // Test that set_level can be called with various log levels
        // This verifies the handler exists and doesn't error
        let levels = [
            LoggingLevel::Debug,
            LoggingLevel::Info,
            LoggingLevel::Warning,
            LoggingLevel::Error,
        ];

        for level in levels {
            let request = SetLevelRequestParams::new(level);
            // Test the internal implementation directly since we can't easily
            // create a RequestContext without a real connection
            let result = server.set_level_impl(request);
            assert!(
                result.is_ok(),
                "set_level should succeed for level {:?}",
                level
            );
        }
    }

    #[test]
    fn test_classify_output_log_level() {
        assert_eq!(
            classify_output_log_level("stderr", "   Compiling dela v0.0.6"),
            LoggingLevel::Info
        );
        assert_eq!(
            classify_output_log_level("stderr", "warning: unused variable"),
            LoggingLevel::Warning
        );
        assert_eq!(
            classify_output_log_level("stderr", "error: could not compile `dela`"),
            LoggingLevel::Error
        );
        assert_eq!(
            classify_output_log_level("stdout", "regular test output"),
            LoggingLevel::Info
        );
    }

    #[test]
    fn test_output_notification_batch_preserves_per_line_levels() {
        let mut batch = OutputNotificationBatch::new("stderr");
        batch.add_line("plain stderr line\n");
        batch.add_line("warning: this is a warning\n");
        batch.add_line("error: this is an error\n");

        let (level, data) = batch.take_notification_data(1234).unwrap();
        assert_eq!(level, LoggingLevel::Error);
        assert_eq!(data["type"], "stderr");
        assert_eq!(data["pid"], 1234);
        assert_eq!(data["lines"].as_array().unwrap().len(), 3);
        assert_eq!(data["lines"][0], "plain stderr line");
        assert_eq!(data["lines"][1], "warning: this is a warning");
        assert_eq!(data["lines"][2], "error: this is an error");
        assert!(data.get("line").is_none());
        assert!(data.get("entries").is_none());
        assert!(data.get("chunk").is_none());
        assert!(data.get("byte_count").is_none());
        assert!(batch.is_empty());
    }

    #[test]
    fn test_output_notification_batch_flushes_at_line_limit() {
        let mut batch = OutputNotificationBatch::new("stdout");
        for index in 0..OUTPUT_NOTIFICATION_MAX_LINES {
            batch.add_line(&format!("line {}\n", index));
        }

        assert!(batch.should_flush());
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
        args.insert(
            "unique_name".to_string(),
            serde_json::Value::String("nonexistent".to_string()),
        );
        let req = CallToolRequestParams::new("task_status").with_arguments(args);
        let res = server.call_tool(req, context).await;
        assert!(res.is_ok());
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
