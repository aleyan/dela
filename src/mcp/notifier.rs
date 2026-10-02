//! Notifications sent on behalf of an in-flight `task_start` request.
//!
//! Everything here is derived from the request itself (its peer and `_meta`), never from
//! connection state, so stateless clients and clients that ran `initialize` behave the same.

// Logging is SEP-2577-deprecated but remains the only way to carry severity-tagged output.
#![allow(deprecated)]

use rmcp::model::{
    LoggingLevel, LoggingMessageNotificationParam, ProgressNotificationParam, ProgressToken,
    RequestMetaObject,
};
use rmcp::service::{Peer, RoleServer};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

const OUTPUT_NOTIFICATION_FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const OUTPUT_NOTIFICATION_MAX_BYTES: usize = 4 * 1024;
const OUTPUT_NOTIFICATION_MAX_LINES: usize = 100;

fn classify_output_log_level(line: &str) -> LoggingLevel {
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

    LoggingLevel::Info
}

/// `LoggingLevel` has no ordering, so rank it by RFC 5424 severity.
fn severity(level: LoggingLevel) -> u8 {
    match level {
        LoggingLevel::Debug => 0,
        LoggingLevel::Info => 1,
        LoggingLevel::Notice => 2,
        LoggingLevel::Warning => 3,
        LoggingLevel::Error => 4,
        LoggingLevel::Critical => 5,
        LoggingLevel::Alert => 6,
        LoggingLevel::Emergency => 7,
    }
}

fn is_at_least(level: LoggingLevel, min_level: LoggingLevel) -> bool {
    severity(level) >= severity(min_level)
}

#[derive(Debug, Clone)]
pub(super) struct OutputNotificationEntry {
    line: String,
    level: LoggingLevel,
}

/// Accumulates output lines of one stream so they are sent as batches rather than per line.
#[derive(Debug, Clone)]
pub(super) struct OutputNotificationBatch {
    stream: &'static str,
    entries: Vec<OutputNotificationEntry>,
    total_bytes: usize,
    started_at: Option<Instant>,
}

impl OutputNotificationBatch {
    pub(super) fn new(stream: &'static str) -> Self {
        Self {
            stream,
            entries: Vec::new(),
            total_bytes: 0,
            started_at: None,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn add_line(&mut self, line: &str) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }

        self.total_bytes += line.len();
        self.entries.push(OutputNotificationEntry {
            line: line.trim_end().to_string(),
            level: classify_output_log_level(line),
        });
    }

    pub(super) fn should_flush(&self) -> bool {
        self.entries.len() >= OUTPUT_NOTIFICATION_MAX_LINES
            || self.total_bytes >= OUTPUT_NOTIFICATION_MAX_BYTES
    }

    pub(super) fn flush_due_at(&self) -> Option<Instant> {
        self.started_at
            .map(|started_at| started_at + OUTPUT_NOTIFICATION_FLUSH_INTERVAL)
    }

    fn take_entries(&mut self) -> Vec<OutputNotificationEntry> {
        self.total_bytes = 0;
        self.started_at = None;
        std::mem::take(&mut self.entries)
    }
}

fn progress_message(entries: &[OutputNotificationEntry]) -> String {
    entries
        .iter()
        .map(|entry| entry.line.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The lines at or above `min_level`, tagged with the most severe level among them.
fn log_output_lines(
    entries: &[OutputNotificationEntry],
    min_level: LoggingLevel,
) -> Option<(LoggingLevel, Vec<&str>)> {
    let kept: Vec<&OutputNotificationEntry> = entries
        .iter()
        .filter(|entry| is_at_least(entry.level, min_level))
        .collect();
    let level = kept
        .iter()
        .map(|entry| entry.level)
        .max_by_key(|level| severity(*level))?;
    Some((
        level,
        kept.iter().map(|entry| entry.line.as_str()).collect(),
    ))
}

/// Streams a task's output to the client waiting on the `task_start` request that started it.
///
/// Output is sent as progress notifications when the request carried a `progressToken`, and as
/// log notifications only when the request's `_meta` asked for a log level. Nothing is sent once
/// the request is cancelled.
#[derive(Clone)]
pub(super) struct TaskStartNotifier {
    peer: Peer<RoleServer>,
    request_ct: CancellationToken,
    progress_token: Option<ProgressToken>,
    min_log_level: Option<LoggingLevel>,
    lines_sent: u64,
}

impl TaskStartNotifier {
    pub(super) fn new(
        peer: Peer<RoleServer>,
        meta: &RequestMetaObject,
        request_ct: CancellationToken,
    ) -> Self {
        Self {
            peer,
            request_ct,
            progress_token: meta.get_progress_token(),
            min_log_level: meta.log_level(),
            lines_sent: 0,
        }
    }

    /// Cancelled when the client cancels the request; rmcp then drops the response but does
    /// not stop the handler, so callers must stop waiting on their own.
    pub(super) fn request_cancellation(&self) -> CancellationToken {
        self.request_ct.clone()
    }

    pub(super) async fn task_event(&self, pid: u32, event: &str, details: serde_json::Value) {
        self.log(
            pid,
            LoggingLevel::Notice,
            serde_json::json!({
                "event": event,
                "pid": pid,
                "details": details,
            }),
        )
        .await;
    }

    async fn flush(&mut self, pid: u32, batch: &mut OutputNotificationBatch) {
        let entries = batch.take_entries();
        if entries.is_empty() {
            return;
        }
        self.progress(&entries).await;
        self.log_output(pid, batch.stream, &entries).await;
    }

    async fn progress(&mut self, entries: &[OutputNotificationEntry]) {
        let Some(token) = self.progress_token.clone() else {
            return;
        };
        if self.request_ct.is_cancelled() {
            return;
        }
        // Progress must increase on every notification and the task's total is unknown, so
        // report the running count of streamed lines.
        self.lines_sent += entries.len() as u64;
        let _ = self
            .peer
            .notify_progress(
                ProgressNotificationParam::new(token, self.lines_sent as f64)
                    .with_message(progress_message(entries)),
            )
            .await;
    }

    async fn log_output(&self, pid: u32, stream: &str, entries: &[OutputNotificationEntry]) {
        let Some(min_level) = self.min_log_level else {
            return;
        };
        let Some((level, lines)) = log_output_lines(entries, min_level) else {
            return;
        };
        self.log(
            pid,
            level,
            serde_json::json!({
                "type": stream,
                "pid": pid,
                "lines": lines,
            }),
        )
        .await;
    }

    async fn log(&self, pid: u32, level: LoggingLevel, data: serde_json::Value) {
        if self.request_ct.is_cancelled()
            || !self
                .min_log_level
                .is_some_and(|min_level| is_at_least(level, min_level))
        {
            return;
        }
        let _ = self
            .peer
            .notify_logging_message(
                LoggingMessageNotificationParam::new(level, data)
                    .with_logger(format!("task:{pid}")),
            )
            .await;
    }
}

/// Send `batch` to the request's client, or just drain it when there is no request to notify.
pub(super) async fn flush_batch(
    notifier: Option<&mut TaskStartNotifier>,
    pid: u32,
    batch: &mut OutputNotificationBatch,
) {
    match notifier {
        Some(notifier) => notifier.flush(pid, batch).await,
        None => {
            batch.take_entries();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch_of(stream: &'static str, lines: &[&str]) -> OutputNotificationBatch {
        let mut batch = OutputNotificationBatch::new(stream);
        for line in lines {
            batch.add_line(&format!("{line}\n"));
        }
        batch
    }

    #[test]
    fn test_classify_output_log_level() {
        assert_eq!(
            classify_output_log_level("   Compiling dela v0.0.6"),
            LoggingLevel::Info
        );
        assert_eq!(
            classify_output_log_level("warning: unused variable"),
            LoggingLevel::Warning
        );
        assert_eq!(
            classify_output_log_level("error: could not compile `dela`"),
            LoggingLevel::Error
        );
        assert_eq!(
            classify_output_log_level("regular test output"),
            LoggingLevel::Info
        );
    }

    #[test]
    fn test_log_output_lines_escalates_to_most_severe_line() {
        let mut batch = batch_of(
            "stderr",
            &[
                "plain stderr line",
                "warning: this is a warning",
                "error: this is an error",
            ],
        );
        let entries = batch.take_entries();

        let (level, lines) = log_output_lines(&entries, LoggingLevel::Info).unwrap();
        assert_eq!(level, LoggingLevel::Error);
        assert_eq!(
            lines,
            vec![
                "plain stderr line",
                "warning: this is a warning",
                "error: this is an error"
            ]
        );
        assert!(batch.is_empty());
    }

    #[test]
    fn test_log_output_lines_honors_min_level() {
        let entries = batch_of(
            "stderr",
            &["plain stderr line", "warning: careful", "error: broken"],
        )
        .take_entries();

        let (level, lines) = log_output_lines(&entries, LoggingLevel::Warning).unwrap();
        assert_eq!(level, LoggingLevel::Error);
        assert_eq!(lines, vec!["warning: careful", "error: broken"]);

        let (level, lines) = log_output_lines(&entries, LoggingLevel::Error).unwrap();
        assert_eq!(level, LoggingLevel::Error);
        assert_eq!(lines, vec!["error: broken"]);

        assert!(log_output_lines(&entries, LoggingLevel::Critical).is_none());
    }

    #[test]
    fn test_progress_message_joins_trimmed_lines() {
        let entries = batch_of("stdout", &["first", "second"]).take_entries();
        assert_eq!(progress_message(&entries), "first\nsecond");
    }

    #[test]
    fn test_output_notification_batch_flushes_at_line_limit() {
        let mut batch = OutputNotificationBatch::new("stdout");
        for index in 0..OUTPUT_NOTIFICATION_MAX_LINES {
            batch.add_line(&format!("line {}\n", index));
        }

        assert!(batch.should_flush());
    }
}
