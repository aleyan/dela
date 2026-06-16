use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Child;
use tokio::sync::RwLock;

/// Result of a graceful stop operation
#[derive(Debug, Clone, PartialEq)]
pub enum StopResult {
    /// Process stopped gracefully with exit code
    Graceful(i32),
    /// Process was force-killed after grace period
    Forced,
    /// Stop operation failed
    Failed(String),
}

/// State of a background job
#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Running,
    Exited(i32),    // exit code
    Failed(String), // error message
}

/// Metadata for a background job
#[derive(Debug, Clone)]
pub struct JobMetadata {
    pub started_at: Instant,
    pub unique_name: String,
    pub source_name: String,
    pub args: Option<Vec<String>>,
    #[allow(dead_code)]
    pub env: Option<std::collections::HashMap<String, String>>,
    pub cwd: Option<PathBuf>,
    pub command: String,
    pub file_path: PathBuf,
}

/// A single captured output line with its source stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLine {
    pub stream: String,
    pub text: String,
}

impl OutputLine {
    pub fn new(stream: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            stream: stream.into(),
            text: text.into(),
        }
    }

    fn len(&self) -> usize {
        self.text.len()
    }
}

/// Ring buffer for storing job output
#[derive(Debug, Clone)]
pub struct RingBuffer {
    buffer: VecDeque<OutputLine>,
    max_size: usize,
    total_bytes: usize,
    max_bytes: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with specified line and byte limits
    pub fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_lines),
            max_size: max_lines,
            total_bytes: 0,
            max_bytes,
        }
    }

    /// Add a line to the buffer, maintaining size limits
    pub fn push_line(&mut self, stream: impl Into<String>, line: String) {
        let entry = OutputLine::new(stream, line);
        let line_bytes = entry.len();

        // Remove lines from the front if we exceed the line limit
        while self.buffer.len() >= self.max_size {
            if let Some(removed) = self.buffer.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(removed.len());
            }
        }

        // Remove lines from the front if we exceed the byte limit
        while self.total_bytes + line_bytes > self.max_bytes && !self.buffer.is_empty() {
            if let Some(removed) = self.buffer.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(removed.len());
            }
        }

        // Add the new line if we have space
        if self.total_bytes + line_bytes <= self.max_bytes {
            self.buffer.push_back(entry);
            self.total_bytes += line_bytes;
        }
    }

    /// Get the last N entries from the buffer
    #[cfg(test)]
    pub fn get_last_entries(&self, n: usize) -> Vec<OutputLine> {
        let start = if self.buffer.len() > n {
            self.buffer.len() - n
        } else {
            0
        };

        self.buffer.iter().skip(start).cloned().collect()
    }

    /// Get all entries in the buffer
    #[cfg(test)]
    pub fn get_all_entries(&self) -> Vec<OutputLine> {
        self.buffer.iter().cloned().collect()
    }

    /// Get entries from a zero-based offset into the retained buffer.
    pub fn get_entries_from(&self, offset: usize, count: usize) -> Vec<OutputLine> {
        self.buffer
            .iter()
            .skip(offset)
            .take(count)
            .cloned()
            .collect()
    }

    /// Get the last N lines from the buffer
    #[cfg(test)]
    pub fn get_last_lines(&self, n: usize) -> Vec<String> {
        self.get_last_entries(n)
            .into_iter()
            .map(|entry| entry.text)
            .collect()
    }

    /// Get all lines in the buffer
    #[cfg(test)]
    pub fn get_all_lines(&self) -> Vec<String> {
        self.get_all_entries()
            .into_iter()
            .map(|entry| entry.text)
            .collect()
    }

    /// Get the total number of lines stored
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Check if the buffer is full (at line limit)
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.max_size
    }

    /// Get the capacity of the buffer (max lines)
    pub fn capacity(&self) -> usize {
        self.max_size
    }

    /// Get the total bytes stored
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

/// A background job with its process and metadata
#[derive(Debug, Clone)]
pub struct Job {
    pub pid: u32,
    pub metadata: JobMetadata,
    pub state: JobState,
    pub completed_at: Option<DateTime<Utc>>,
    pub elapsed_at_completion: Option<Duration>,
    pub output_buffer: RingBuffer,
    pub last_activity: Instant,
}

impl Job {
    /// Create a new job
    pub fn new(
        pid: u32,
        metadata: JobMetadata,
        max_output_lines: usize,
        max_output_bytes: usize,
    ) -> Self {
        Self {
            pid,
            metadata,
            state: JobState::Running,
            completed_at: None,
            elapsed_at_completion: None,
            output_buffer: RingBuffer::new(max_output_lines, max_output_bytes),
            last_activity: Instant::now(),
        }
    }

    /// Update the job's last activity time
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Mark the job as exited with the given exit code
    pub fn mark_exited(&mut self, exit_code: i32) {
        self.elapsed_at_completion = Some(self.metadata.started_at.elapsed());
        self.state = JobState::Exited(exit_code);
        self.completed_at = Some(Utc::now());
        self.touch();
    }

    /// Mark the job as failed with the given error message
    pub fn mark_failed(&mut self, error: String) {
        self.elapsed_at_completion = Some(self.metadata.started_at.elapsed());
        self.state = JobState::Failed(error);
        self.completed_at = Some(Utc::now());
        self.touch();
    }

    /// Add output to the job's ring buffer
    pub fn add_output(&mut self, stream: &str, output: String) {
        // Split output into lines and add each line
        for line in output.lines() {
            self.output_buffer.push_line(stream, line.to_string());
        }
        self.touch();
    }

    /// Get the job's output as stream-tagged lines from a retained-buffer offset.
    pub fn get_output_entries_from(&self, offset: usize, count: usize) -> Vec<OutputLine> {
        self.output_buffer.get_entries_from(offset, count)
    }

    /// Get the job's output as lines
    #[cfg(test)]
    pub fn get_output_lines(&self, max_lines: Option<usize>) -> Vec<String> {
        match max_lines {
            Some(n) => self.output_buffer.get_last_lines(n),
            None => self.output_buffer.get_all_lines(),
        }
    }

    /// Check if the job is still running
    pub fn is_running(&self) -> bool {
        matches!(self.state, JobState::Running)
    }

    /// Get the job's age
    #[allow(dead_code)]
    pub fn age(&self) -> Duration {
        self.elapsed_at_completion
            .unwrap_or_else(|| self.metadata.started_at.elapsed())
    }

    /// Get the time since last activity
    #[allow(dead_code)]
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

/// Configuration for job management
#[derive(Debug, Clone)]
pub struct JobManagerConfig {
    pub max_concurrent_jobs: usize,
    pub max_output_lines_per_job: usize,
    pub max_output_bytes_per_job: usize,
    #[allow(dead_code)]
    pub job_ttl_seconds: u64,
    #[allow(dead_code)]
    pub gc_interval_seconds: u64,
}

impl Default for JobManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 50,
            max_output_lines_per_job: 10_000,
            max_output_bytes_per_job: 5 * 1024 * 1024, // 5MB
            job_ttl_seconds: 3600,                     // 1 hour
            gc_interval_seconds: 300,                  // 5 minutes
        }
    }
}

/// Manager for background jobs
#[derive(Clone)]
pub struct JobManager {
    jobs: Arc<RwLock<HashMap<u32, Job>>>,
    pub processes: Arc<RwLock<HashMap<u32, Child>>>,
    config: JobManagerConfig,
    #[allow(dead_code)]
    last_gc: Arc<RwLock<Instant>>,
}

impl JobManager {
    /// Create a new job manager with default configuration
    pub fn new() -> Self {
        Self::with_config(JobManagerConfig::default())
    }

    /// Create a new job manager with custom configuration
    pub fn with_config(config: JobManagerConfig) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            processes: Arc::new(RwLock::new(HashMap::new())),
            config,
            last_gc: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Check if we can start a new job (concurrency limit check)
    pub async fn can_start_job(&self) -> anyhow::Result<()> {
        self.garbage_collect().await;
        let jobs = self.jobs.read().await;
        let running_jobs = jobs.values().filter(|job| job.is_running()).count();
        if running_jobs >= self.config.max_concurrent_jobs {
            return Err(anyhow::anyhow!(
                "Maximum concurrent jobs limit reached: {}",
                self.config.max_concurrent_jobs
            ));
        }
        Ok(())
    }

    /// Start a new job
    pub async fn start_job(
        &self,
        pid: u32,
        metadata: JobMetadata,
        process: Child,
    ) -> anyhow::Result<()> {
        let mut jobs = self.jobs.write().await;

        // Check concurrent job limit
        if jobs.len() >= self.config.max_concurrent_jobs {
            return Err(anyhow::anyhow!(
                "Maximum concurrent jobs limit reached: {}",
                self.config.max_concurrent_jobs
            ));
        }

        if matches!(jobs.get(&pid), Some(existing) if existing.is_running()) {
            return Err(anyhow::anyhow!(
                "Refusing to overwrite running job with PID {}",
                pid
            ));
        }

        // Create the job
        let job = Job::new(
            pid,
            metadata,
            self.config.max_output_lines_per_job,
            self.config.max_output_bytes_per_job,
        );

        jobs.insert(pid, job);

        // Store the process separately
        let mut processes = self.processes.write().await;
        processes.insert(pid, process);

        Ok(())
    }

    /// Record a completed job without retaining a process handle
    pub async fn record_completed_job(
        &self,
        pid: u32,
        metadata: JobMetadata,
        state: JobState,
    ) -> anyhow::Result<()> {
        let mut jobs = self.jobs.write().await;
        if matches!(jobs.get(&pid), Some(existing) if existing.is_running()) {
            return Err(anyhow::anyhow!(
                "Refusing to overwrite running job with PID {}",
                pid
            ));
        }
        let elapsed_at_completion = match state {
            JobState::Running => None,
            JobState::Exited(_) | JobState::Failed(_) => Some(metadata.started_at.elapsed()),
        };
        jobs.insert(
            pid,
            Job {
                pid,
                metadata,
                completed_at: match state {
                    JobState::Running => None,
                    JobState::Exited(_) | JobState::Failed(_) => Some(Utc::now()),
                },
                elapsed_at_completion,
                state,
                output_buffer: RingBuffer::new(
                    self.config.max_output_lines_per_job,
                    self.config.max_output_bytes_per_job,
                ),
                last_activity: Instant::now(),
            },
        );
        Ok(())
    }

    /// Get a job by PID
    pub async fn get_job(&self, pid: u32) -> Option<Job> {
        let jobs = self.jobs.read().await;
        jobs.get(&pid).cloned()
    }

    /// Get all jobs
    pub async fn get_all_jobs(&self) -> Vec<Job> {
        let jobs = self.jobs.read().await;
        jobs.values().cloned().collect()
    }

    /// Get jobs by unique name
    pub async fn get_jobs_by_name(&self, unique_name: &str) -> Vec<Job> {
        let jobs = self.jobs.read().await;
        jobs.values()
            .filter(|job| job.metadata.unique_name == unique_name)
            .cloned()
            .collect()
    }

    /// Update a job's state
    pub async fn update_job_state(&self, pid: u32, state: JobState) -> anyhow::Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(&pid) {
            match state {
                JobState::Running => {
                    job.state = JobState::Running;
                    job.completed_at = None;
                    job.elapsed_at_completion = None;
                    job.touch();
                }
                JobState::Exited(exit_code) => job.mark_exited(exit_code),
                JobState::Failed(error) => job.mark_failed(error),
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Job with PID {} not found", pid))
        }
    }

    async fn update_job_exited(&self, pid: u32, exit_code: i32) {
        let _ = self
            .update_job_state(pid, JobState::Exited(exit_code))
            .await;
    }

    async fn update_job_failed(&self, pid: u32, error_message: String) {
        let _ = self
            .update_job_state(pid, JobState::Failed(error_message))
            .await;
    }

    /// Add stdout output to a job for tests that do not care about stream identity.
    #[cfg(test)]
    pub async fn add_job_output(&self, pid: u32, output: String) -> anyhow::Result<()> {
        self.add_job_output_chunk(pid, "stdout", output).await
    }

    /// Add stream-tagged output to a job.
    pub async fn add_job_output_chunk(
        &self,
        pid: u32,
        stream: &str,
        output: String,
    ) -> anyhow::Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(&pid) {
            job.add_output(stream, output);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Job with PID {} not found", pid))
        }
    }

    /// Stop a job (send SIGTERM)
    #[allow(dead_code)]
    pub async fn stop_job(&self, pid: u32) -> anyhow::Result<()> {
        let mut processes = self.processes.write().await;
        if let Some(mut process) = processes.remove(&pid) {
            if let Err(e) = process.kill().await {
                // Update job state to failed
                let mut jobs = self.jobs.write().await;
                if let Some(job) = jobs.get_mut(&pid) {
                    job.mark_failed(format!("Failed to kill process: {}", e));
                }
            } else {
                // Update job last activity
                let mut jobs = self.jobs.write().await;
                if let Some(job) = jobs.get_mut(&pid) {
                    job.touch();
                }
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Job with PID {} not found", pid))
        }
    }

    /// Gracefully stop a job (SIGTERM + grace period + SIGKILL)
    pub async fn stop_job_graceful(
        &self,
        pid: u32,
        grace_period_seconds: u64,
    ) -> anyhow::Result<StopResult> {
        let maybe_process = {
            let mut processes = self.processes.write().await;
            processes.remove(&pid)
        };
        if let Some(process) = maybe_process {
            self.stop_managed_job(pid, process, grace_period_seconds)
                .await
        } else {
            let is_tracked = {
                let jobs = self.jobs.read().await;
                jobs.contains_key(&pid)
            };
            if !is_tracked {
                return Err(anyhow::anyhow!("Job with PID {} not found", pid));
            }
            self.stop_unmanaged_job_fallback(pid, grace_period_seconds)
                .await
        }
    }

    async fn stop_managed_job(
        &self,
        pid: u32,
        mut process: Child,
        grace_period_seconds: u64,
    ) -> anyhow::Result<StopResult> {
        use tokio::time::{Duration, timeout};

        #[cfg(unix)]
        {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;

            match signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                Ok(()) => {}
                Err(nix::errno::Errno::ESRCH) => {
                    let exit_status = process.wait().await.map_err(|e| {
                        anyhow::anyhow!("Process already exited but wait failed: {}", e)
                    })?;
                    let exit_code = exit_status.code().unwrap_or(0);
                    self.update_job_exited(pid, exit_code).await;
                    return Ok(StopResult::Graceful(exit_code));
                }
                Err(e) => {
                    self.update_job_failed(pid, format!("Failed to send SIGTERM: {}", e))
                        .await;
                    return Ok(StopResult::Failed(format!("Failed to send SIGTERM: {}", e)));
                }
            }
        }
        #[cfg(not(unix))]
        {
            if let Err(e) = process.kill().await {
                self.update_job_failed(pid, format!("Failed to stop process: {}", e))
                    .await;
                return Ok(StopResult::Failed(format!("Failed to stop process: {}", e)));
            }
        }

        // Wait for the process to exit gracefully
        let grace_duration = Duration::from_secs(grace_period_seconds);
        let wait_result = timeout(grace_duration, process.wait()).await;

        match wait_result {
            Ok(Ok(exit_status)) => {
                // Process exited gracefully
                let exit_code = exit_status.code().unwrap_or(-1);
                self.update_job_exited(pid, exit_code).await;
                Ok(StopResult::Graceful(exit_code))
            }
            Ok(Err(e)) => {
                // Process wait failed
                self.update_job_failed(pid, format!("Process wait failed: {}", e))
                    .await;
                Ok(StopResult::Failed(format!("Process wait failed: {}", e)))
            }
            Err(_) => {
                // Grace period expired, send SIGKILL
                #[cfg(unix)]
                {
                    use nix::sys::signal::{self, Signal};
                    use nix::unistd::Pid;

                    let result = signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
                    match result {
                        Ok(()) => {
                            // Wait/reap the child process to prevent zombie processes
                            let _ = process.wait().await;
                            self.update_job_failed(
                                pid,
                                "Stopped with SIGKILL after grace period".to_string(),
                            )
                            .await;
                            Ok(StopResult::Forced)
                        }
                        Err(nix::errno::Errno::ESRCH) => {
                            let exit_status = process.wait().await.ok().and_then(|s| s.code()).unwrap_or(0);
                            self.update_job_exited(pid, exit_status).await;
                            Ok(StopResult::Graceful(exit_status))
                        }
                        Err(e) => {
                            self.update_job_failed(pid, format!("Failed to send SIGKILL: {}", e))
                                .await;
                            Ok(StopResult::Failed(format!("Failed to send SIGKILL: {}", e)))
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = process.kill().await;
                    let _ = process.wait().await;
                    self.update_job_failed(
                        pid,
                        "SIGKILL not supported on this platform".to_string(),
                    )
                    .await;
                    Ok(StopResult::Failed(
                        "SIGKILL not supported on this platform".to_string(),
                    ))
                }
            }
        }
    }

    async fn stop_unmanaged_job_fallback(
        &self,
        pid: u32,
        grace_period_seconds: u64,
    ) -> anyhow::Result<StopResult> {
        #[cfg(unix)]
        {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;
            use tokio::time::Duration;

            // Send SIGTERM best-effort
            let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM);

            // Wait for grace period
            tokio::time::sleep(Duration::from_secs(grace_period_seconds)).await;

            // Send SIGKILL if still present
            let kill_result = signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL);

            match kill_result {
                Ok(()) => {
                    self.update_job_failed(pid, "Stopped with SIGKILL (fallback)".to_string())
                        .await;
                    Ok(StopResult::Forced)
                }
                Err(nix::errno::Errno::ESRCH) => {
                    self.update_job_exited(pid, 0).await;
                    Ok(StopResult::Graceful(0))
                }
                Err(e) => {
                    self.update_job_failed(
                        pid,
                        format!("Failed to send SIGKILL (fallback): {}", e),
                    )
                    .await;
                    Ok(StopResult::Failed(format!("Failed to send SIGKILL: {}", e)))
                }
            }
        }
        #[cfg(not(unix))]
        {
            self.update_job_failed(
                pid,
                "Signal handling not supported on this platform".to_string(),
            )
            .await;
            Ok(StopResult::Failed(
                "Signal handling not supported on this platform".to_string(),
            ))
        }
    }

    /// Remove a job
    #[allow(dead_code)]
    pub async fn remove_job(&self, pid: u32) -> anyhow::Result<()> {
        let mut jobs = self.jobs.write().await;
        let mut processes = self.processes.write().await;

        let job_removed = jobs.remove(&pid).is_some();
        let process_removed = processes.remove(&pid).is_some();

        if job_removed || process_removed {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Job with PID {} not found", pid))
        }
    }

    /// Run garbage collection to remove old jobs
    #[allow(dead_code)]
    pub async fn garbage_collect(&self) {
        let now = Instant::now();

        // Check if enough time has passed since last GC
        {
            let last_gc = self.last_gc.read().await;
            if now.duration_since(*last_gc).as_secs() < self.config.gc_interval_seconds {
                return;
            }
        }

        let mut jobs = self.jobs.write().await;
        let mut processes = self.processes.write().await;
        let ttl = Duration::from_secs(self.config.job_ttl_seconds);

        // Collect PIDs to remove
        let mut pids_to_remove = Vec::new();

        for (pid, job) in jobs.iter() {
            let age = job.age();
            let idle = job.idle_time();

            // Keep jobs that are still running and not too old
            if job.is_running() && age < ttl {
                continue;
            }

            // Keep finished jobs that haven't been idle too long
            if !job.is_running() && idle < Duration::from_secs(300) {
                // 5 minutes
                continue;
            }

            // Mark this job for removal
            pids_to_remove.push(*pid);
        }

        // Remove jobs and processes
        for pid in pids_to_remove {
            jobs.remove(&pid);
            processes.remove(&pid);
        }

        // Update last GC time
        {
            let mut last_gc = self.last_gc.write().await;
            *last_gc = now;
        }
    }

    /// Get job statistics
    #[allow(dead_code)]
    pub async fn get_stats(&self) -> JobStats {
        let jobs = self.jobs.read().await;
        let mut running = 0;
        let mut exited = 0;
        let mut failed = 0;

        for job in jobs.values() {
            match job.state {
                JobState::Running => running += 1,
                JobState::Exited(_) => exited += 1,
                JobState::Failed(_) => failed += 1,
            }
        }

        JobStats {
            total_jobs: jobs.len(),
            running_jobs: running,
            exited_jobs: exited,
            failed_jobs: failed,
        }
    }
}

/// Statistics about jobs
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct JobStats {
    pub total_jobs: usize,
    pub running_jobs: usize,
    pub exited_jobs: usize,
    pub failed_jobs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::process::Command;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buffer = RingBuffer::new(3, 100);

        buffer.push_line("stdout", "line1".to_string());
        buffer.push_line("stderr", "line2".to_string());
        buffer.push_line("stdout", "line3".to_string());

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get_all_lines(), vec!["line1", "line2", "line3"]);
        assert_eq!(buffer.get_all_entries()[1].stream, "stderr");
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buffer = RingBuffer::new(2, 100);

        buffer.push_line("stdout", "line1".to_string());
        buffer.push_line("stdout", "line2".to_string());
        buffer.push_line("stdout", "line3".to_string());

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.get_all_lines(), vec!["line2", "line3"]);
    }

    #[test]
    fn test_ring_buffer_last_lines() {
        let mut buffer = RingBuffer::new(5, 100);

        for i in 1..=5 {
            buffer.push_line("stdout", format!("line{}", i));
        }

        assert_eq!(buffer.get_last_lines(2), vec!["line4", "line5"]);
        assert_eq!(
            buffer.get_last_lines(10),
            vec!["line1", "line2", "line3", "line4", "line5"]
        );
    }

    #[test]
    fn test_job_metadata_creation() {
        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: Some(vec!["--verbose".to_string()]),
            env: None,
            cwd: Some(PathBuf::from("/tmp")),
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        assert_eq!(metadata.unique_name, "test-task");
        assert_eq!(metadata.source_name, "test");
        assert!(metadata.args.is_some());
        assert_eq!(metadata.args.unwrap(), vec!["--verbose"]);
    }

    #[tokio::test]
    async fn test_job_manager_creation() {
        let manager = JobManager::new();
        let stats = manager.get_stats().await;

        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.running_jobs, 0);
    }

    #[tokio::test]
    async fn test_job_manager_start_job() {
        let manager = JobManager::new();

        // Create a simple command
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        let result = manager.start_job(pid, metadata, child).await;
        assert!(result.is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_jobs, 1);
        assert_eq!(stats.running_jobs, 1);
    }

    #[tokio::test]
    async fn test_job_manager_get_job() {
        let manager = JobManager::new();

        let mut cmd = Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager.start_job(pid, metadata, child).await.unwrap();

        let job = manager.get_job(pid).await;
        assert!(job.is_some());
        let job = job.unwrap();
        assert_eq!(job.pid, pid);
        assert_eq!(job.metadata.unique_name, "test-task");
        assert!(job.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_job_manager_add_output() {
        let manager = JobManager::new();

        let mut cmd = Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager.start_job(pid, metadata, child).await.unwrap();

        // Add some output
        manager
            .add_job_output(pid, "Hello, world!".to_string())
            .await
            .unwrap();
        manager
            .add_job_output(pid, "This is a test".to_string())
            .await
            .unwrap();

        let job = manager.get_job(pid).await.unwrap();
        let output = job.get_output_lines(None);
        assert_eq!(output, vec!["Hello, world!", "This is a test"]);
    }

    #[tokio::test]
    async fn test_job_manager_garbage_collect() {
        let manager = JobManager::with_config(JobManagerConfig {
            max_concurrent_jobs: 10,
            max_output_lines_per_job: 10,
            max_output_bytes_per_job: 1000,
            job_ttl_seconds: 0,     // Very short TTL for testing
            gc_interval_seconds: 0, // Run GC immediately
        });

        let mut cmd = Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager.start_job(pid, metadata, child).await.unwrap();

        // Mark job as exited
        manager
            .update_job_state(pid, JobState::Exited(0))
            .await
            .unwrap();

        // Manually remove the job to test the remove functionality
        manager.remove_job(pid).await.unwrap();

        // Job should be removed
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_jobs, 0);
    }

    #[tokio::test]
    async fn test_job_manager_records_completion_metadata() {
        let manager = JobManager::new();

        let mut cmd = Command::new("echo");
        cmd.arg("test");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo test".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager.start_job(pid, metadata, child).await.unwrap();
        manager
            .update_job_state(pid, JobState::Exited(0))
            .await
            .unwrap();

        let job = manager.get_job(pid).await.unwrap();
        assert_eq!(job.state, JobState::Exited(0));
        assert!(job.completed_at.is_some());
        assert!(job.elapsed_at_completion.is_some());
    }

    #[tokio::test]
    async fn test_record_completed_job_rejects_overwriting_running_job() {
        let manager = JobManager::new();

        let mut cmd = Command::new("sleep");
        cmd.arg("5");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-task".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "sleep 5".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager
            .start_job(pid, metadata.clone(), child)
            .await
            .unwrap();

        let result = manager
            .record_completed_job(pid, metadata, JobState::Exited(0))
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Refusing to overwrite running job")
        );

        let job = manager.get_job(pid).await.unwrap();
        assert!(job.is_running());

        let _ = manager.stop_job_graceful(pid, 0).await;
    }

    #[tokio::test]
    async fn test_stop_job_graceful_managed_success() {
        let manager = JobManager::new();

        let mut cmd = Command::new("sleep");
        cmd.arg("10");
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-sleep".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "sleep 10".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager.start_job(pid, metadata, child).await.unwrap();

        let stop_result = manager.stop_job_graceful(pid, 1).await.unwrap();
        assert!(matches!(
            stop_result,
            StopResult::Graceful(_) | StopResult::Forced
        ));
        let job = manager.get_job(pid).await.unwrap();
        assert!(!job.is_running());
        assert!(matches!(
            job.state,
            JobState::Exited(_) | JobState::Failed(_)
        ));
    }

    #[tokio::test]
    async fn test_stop_job_graceful_fallback() {
        let manager = JobManager::new();

        let mut child = tokio::process::Command::new("true")
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        
        // Wait for it to exit so it's fully reaped and no longer a zombie.
        // This gives us a safe PID that is guaranteed to be dead and return ESRCH.
        let _ = child.wait().await;

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-fallback".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "sleep 10".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        {
            let mut jobs = manager.jobs.write().await;
            jobs.insert(pid, Job::new(pid, metadata, 10, 1000));
        }

        let stop_result = manager.stop_job_graceful(pid, 1).await.unwrap();
        #[cfg(unix)]
        {
            assert_eq!(stop_result, StopResult::Graceful(0));
            let job = manager.get_job(pid).await.unwrap();
            assert_eq!(job.state, JobState::Exited(0));
        }
        #[cfg(not(unix))]
        {
            assert!(matches!(stop_result, StopResult::Failed(_)));
            let job = manager.get_job(pid).await.unwrap();
            assert!(matches!(job.state, JobState::Failed(_)));
        }

        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    #[tokio::test]
    async fn test_stop_job_graceful_managed_sigkill() {
        let manager = JobManager::new();

        let mut cmd = Command::new("python3");
        cmd.arg("-c");
        cmd.arg(
            "import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(10)",
        );
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-sigkill".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "ignore term".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager.start_job(pid, metadata, child).await.unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;

        let stop_result = manager.stop_job_graceful(pid, 1).await.unwrap();
        #[cfg(unix)]
        {
            assert_eq!(stop_result, StopResult::Forced);
            let job = manager.get_job(pid).await.unwrap();
            assert!(matches!(job.state, JobState::Failed(_)));
        }
        #[cfg(not(unix))]
        {
            assert!(matches!(
                stop_result,
                StopResult::Failed(_) | StopResult::Graceful(_)
            ));
            let job = manager.get_job(pid).await.unwrap();
            assert!(!job.is_running());
        }
    }

    #[tokio::test]
    async fn test_stop_job_graceful_already_exited() {
        let manager = JobManager::new();

        let mut cmd = Command::new("echo");
        let child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();

        let metadata = JobMetadata {
            started_at: Instant::now(),
            unique_name: "test-exited".to_string(),
            source_name: "test".to_string(),
            args: None,
            env: None,
            cwd: None,
            command: "echo".to_string(),
            file_path: PathBuf::from("Makefile"),
        };

        manager.start_job(pid, metadata, child).await.unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;

        let stop_result = manager.stop_job_graceful(pid, 1).await.unwrap();
        assert!(matches!(stop_result, StopResult::Graceful(_)));
        let job = manager.get_job(pid).await.unwrap();
        assert!(!job.is_running());
    }
}
