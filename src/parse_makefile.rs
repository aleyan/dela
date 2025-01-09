use std::fs::File;
use std::path::Path;
use makefile_lossless::Makefile;

use crate::types::{Task, TaskRunner, TaskDefinitionFile, TaskFileStatus};

/// Parse a Makefile at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open Makefile: {}", e))?;

    let makefile = Makefile::read(file)
        .map_err(|e| format!("Failed to read/parse Makefile: {}", e))?;

    let mut tasks = Vec::new();
    for rule in makefile.rules() {
        // Skip pattern rules and those starting with '.'
        let targets = rule.targets().collect::<Vec<_>>();
        if targets.is_empty() || targets[0].starts_with('.') {
            continue;
        }

        let target = &targets[0];
        let description = extract_task_description(&rule);

        tasks.push(Task {
            name: target.to_string(),
            file_path: path.to_path_buf(),
            runner: TaskRunner::Make,
            source_name: target.to_string(),
            description,
        });
    }

    Ok(tasks)
}

/// Create a TaskDefinitionFile for a Makefile
pub fn create_definition(path: &Path, status: TaskFileStatus) -> TaskDefinitionFile {
    TaskDefinitionFile {
        path: path.to_path_buf(),
        runner: TaskRunner::Make,
        status,
    }
}

/// Extract a task description from a rule's commands
fn extract_task_description(rule: &makefile_lossless::Rule) -> Option<String> {
    // Look for echo commands that might be descriptions
    for cmd in rule.recipes() {
        let cmd = cmd.trim();
        if cmd.starts_with("@echo") || cmd.starts_with("echo") {
            let desc = cmd.trim_start_matches("@echo")
                .trim_start_matches("echo")
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !desc.is_empty() {
                return Some(desc);
            }
        }
    }
    None
} 