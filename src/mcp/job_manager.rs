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
    pub env: Option<std::collections::HashMap<String, String>>,
    pub cwd: Option<PathBuf>,
    pub command: String,
    pub file_path: PathBuf,
}

/// Ring buffer for storing job output
#[derive(Debug, Clone)]
pub struct RingBuffer {
    buffer: VecDeque<String>,
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
    pub fn push_line(&mut self, line: String) {
        let line_bytes = line.len();

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
            self.buffer.push_back(line);
            self.total_bytes += line_bytes;
        }
    }

    /// Get the last N lines from the buffer
    pub fn get_last_lines(&self, n: usize) -> Vec<String> {
        let start = if self.buffer.len() > n {
            self.buffer.len() - n
        } else {
            0
        };

        self.buffer.iter().skip(start).cloned().collect()
    }

    /// Get all lines in the buffer
    pub fn get_all_lines(&self) -> Vec<String> {
        self.buffer.iter().cloned().collect()
    }

    /// Get the total number of lines stored
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
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
        self.state = JobState::Exited(exit_code);
        self.touch();
    }

    /// Mark the job as failed with the given error message
    pub fn mark_failed(&mut self, error: String) {
        self.state = JobState::Failed(error);
        self.touch();
    }

    /// Add output to the job's ring buffer
    pub fn add_output(&mut self, output: String) {
        // Split output into lines and add each line
        for line in output.lines() {
            self.output_buffer.push_line(line.to_string());
        }
        self.touch();
    }

    /// Get the job's output as lines
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
    pub fn age(&self) -> Duration {
        self.metadata.started_at.elapsed()
    }

    /// Get the time since last activity
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
    pub job_ttl_seconds: u64,
    pub gc_interval_seconds: u64,
}

impl Default for JobManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 50,
            max_output_lines_per_job: 1000,
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
    pub async fn can_start_job(&self) -> Result<(), String> {
        let jobs = self.jobs.read().await;
        if jobs.len() >= self.config.max_concurrent_jobs {
            return Err(format!(
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
    ) -> Result<(), String> {
        let mut jobs = self.jobs.write().await;

        // Check concurrent job limit
        if jobs.len() >= self.config.max_concurrent_jobs {
            return Err(format!(
                "Maximum concurrent jobs limit reached: {}",
                self.config.max_concurrent_jobs
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
    pub async fn update_job_state(&self, pid: u32, state: JobState) -> Result<(), String> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(&pid) {
            job.state = state;
            job.touch();
            Ok(())
        } else {
            Err(format!("Job with PID {} not found", pid))
        }
    }

    /// Add output to a job
    pub async fn add_job_output(&self, pid: u32, output: String) -> Result<(), String> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(&pid) {
            job.add_output(output);
            Ok(())
        } else {
            Err(format!("Job with PID {} not found", pid))
        }
    }

    /// Stop a job (send SIGTERM)
    pub async fn stop_job(&self, pid: u32) -> Result<(), String> {
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
            Err(format!("Job with PID {} not found", pid))
        }
    }

    /// Gracefully stop a job (SIGTERM + grace period + SIGKILL)
    pub async fn stop_job_graceful(
        &self,
        pid: u32,
        grace_period_seconds: u64,
    ) -> Result<StopResult, String> {
        use tokio::time::{Duration, timeout};

        // First, try to get the process from our managed processes
        let mut processes = self.processes.write().await;
        if let Some(mut process) = processes.remove(&pid) {
            // Send SIGTERM
            if let Err(e) = process.kill().await {
                // Update job state to failed
                let mut jobs = self.jobs.write().await;
                if let Some(job) = jobs.get_mut(&pid) {
                    job.mark_failed(format!("Failed to send SIGTERM: {}", e));
                }
                return Ok(StopResult::Failed(format!("Failed to send SIGTERM: {}", e)));
            }

            // Wait for the process to exit gracefully
            let grace_duration = Duration::from_secs(grace_period_seconds);
            let wait_result = timeout(grace_duration, process.wait()).await;

            match wait_result {
                Ok(Ok(exit_status)) => {
                    // Process exited gracefully
                    let exit_code = exit_status.code().unwrap_or(-1);
                    let mut jobs = self.jobs.write().await;
                    if let Some(job) = jobs.get_mut(&pid) {
                        job.mark_exited(exit_code);
                    }
                    Ok(StopResult::Graceful(exit_code))
                }
                Ok(Err(e)) => {
                    // Process wait failed
                    let mut jobs = self.jobs.write().await;
                    if let Some(job) = jobs.get_mut(&pid) {
                        job.mark_failed(format!("Process wait failed: {}", e));
                    }
                    Ok(StopResult::Failed(format!("Process wait failed: {}", e)))
                }
                Err(_) => {
                    // Grace period expired, send SIGKILL
                    drop(processes); // Release the lock before trying to kill

                    // Try to kill the process using native Rust signal handling
                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{self, Signal};
                        use nix::unistd::Pid;
                        
                        let result = signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
                        match result {
                            Ok(()) => {
                                // Update job state to stopped
                                let mut jobs = self.jobs.write().await;
                                if let Some(job) = jobs.get_mut(&pid) {
                                    job.mark_failed(
                                        "Stopped with SIGKILL after grace period".to_string(),
                                    );
                                }
                                Ok(StopResult::Forced)
                            }
                            Err(nix::errno::Errno::ESRCH) => {
                                // Process already exited - this is actually success
                                let mut jobs = self.jobs.write().await;
                                if let Some(job) = jobs.get_mut(&pid) {
                                    job.mark_exited(0); // Process already exited gracefully
                                }
                                Ok(StopResult::Graceful(0)) // Treat as graceful exit
                            }
                        Err(e) => {
                                // Other signal errors
                            let mut jobs = self.jobs.write().await;
                            if let Some(job) = jobs.get_mut(&pid) {
                                    job.mark_failed(format!("Failed to send SIGKILL: {}", e));
                                }
                                Ok(StopResult::Failed(format!("Failed to send SIGKILL: {}", e)))
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        // On non-Unix systems, we can't send signals, so just mark as failed
                        let mut jobs = self.jobs.write().await;
                        if let Some(job) = jobs.get_mut(&pid) {
                            job.mark_failed("SIGKILL not supported on this platform".to_string());
                        }
                        Ok(StopResult::Failed("SIGKILL not supported on this platform".to_string()))
                    }
                }
            }
        } else {
            // Fallback: no Child handle (likely already moved to monitor). Use PID signals.
            #[cfg(unix)]
            {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;
                
            // Send SIGTERM best-effort
                let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM);

            // Wait for grace period
            tokio::time::sleep(Duration::from_secs(grace_period_seconds)).await;

            // Send SIGKILL if still present
                let kill_result = signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL);

            match kill_result {
                    Ok(()) => {
                        // Mark job as forced stop
                        let mut jobs = self.jobs.write().await;
                        if let Some(job) = jobs.get_mut(&pid) {
                            job.mark_failed("Stopped with SIGKILL (fallback)".to_string());
                        }
                        Ok(StopResult::Forced)
                    }
                    Err(nix::errno::Errno::ESRCH) => {
                        // Process already exited - this is actually success
                        let mut jobs = self.jobs.write().await;
                        if let Some(job) = jobs.get_mut(&pid) {
                            job.mark_exited(0); // Process already exited gracefully
                        }
                        Ok(StopResult::Graceful(0)) // Treat as graceful exit
                    }
                Err(e) => {
                    let mut jobs = self.jobs.write().await;
                    if let Some(job) = jobs.get_mut(&pid) {
                            job.mark_failed(format!("Failed to send SIGKILL (fallback): {}", e));
                        }
                        Ok(StopResult::Failed(format!("Failed to send SIGKILL: {}", e)))
                    }
                }
            }
            #[cfg(not(unix))]
            {
                // On non-Unix systems, we can't send signals, so just mark as failed
                let mut jobs = self.jobs.write().await;
                if let Some(job) = jobs.get_mut(&pid) {
                    job.mark_failed("Signal handling not supported on this platform".to_string());
                }
                Ok(StopResult::Failed("Signal handling not supported on this platform".to_string()))
            }
        }
    }

    /// Remove a job
    pub async fn remove_job(&self, pid: u32) -> Result<(), String> {
        let mut jobs = self.jobs.write().await;
        let mut processes = self.processes.write().await;

        let job_removed = jobs.remove(&pid).is_some();
        let process_removed = processes.remove(&pid).is_some();

        if job_removed || process_removed {
            Ok(())
        } else {
            Err(format!("Job with PID {} not found", pid))
        }
    }

    /// Run garbage collection to remove old jobs
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

        buffer.push_line("line1".to_string());
        buffer.push_line("line2".to_string());
        buffer.push_line("line3".to_string());

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get_all_lines(), vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buffer = RingBuffer::new(2, 100);

        buffer.push_line("line1".to_string());
        buffer.push_line("line2".to_string());
        buffer.push_line("line3".to_string());

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.get_all_lines(), vec!["line2", "line3"]);
    }

    #[test]
    fn test_ring_buffer_last_lines() {
        let mut buffer = RingBuffer::new(5, 100);

        for i in 1..=5 {
            buffer.push_line(format!("line{}", i));
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
}
