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
        if targets.is_empty() || targets[0].contains('%') || targets[0].starts_with('.') {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_makefile(dir: &Path, content: &str) -> std::path::PathBuf {
        let makefile_path = dir.join("Makefile");
        let mut file = File::create(&makefile_path).unwrap();
        writeln!(file, "{}", content).unwrap();
        makefile_path
    }

    #[test]
    fn test_parse_empty_makefile() {
        let temp_dir = TempDir::new().unwrap();
        let makefile_path = create_test_makefile(temp_dir.path(), "");
        
        let tasks = parse(&makefile_path).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_simple_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#".PHONY: build test

build:
	@echo "Building the project"
	cargo build

test:
	@echo "Running tests"
	cargo test"#;
        let makefile_path = create_test_makefile(temp_dir.path(), content);
        
        let tasks = parse(&makefile_path).unwrap();
        assert_eq!(tasks.len(), 2);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Make);
        assert_eq!(build_task.source_name, "build");
        assert_eq!(build_task.description, Some("Building the project".to_string()));

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Make);
        assert_eq!(test_task.source_name, "test");
        assert_eq!(test_task.description, Some("Running tests".to_string()));
    }

    #[test]
    fn test_parse_task_without_description() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"clean:
	rm -rf target/"#;
        let makefile_path = create_test_makefile(temp_dir.path(), content);
        
        let tasks = parse(&makefile_path).unwrap();
        assert_eq!(tasks.len(), 1);

        let clean_task = &tasks[0];
        assert_eq!(clean_task.name, "clean");
        assert_eq!(clean_task.runner, TaskRunner::Make);
        assert_eq!(clean_task.source_name, "clean");
        assert_eq!(clean_task.description, None);
    }

    #[test]
    fn test_parse_ignores_pattern_rules() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"build:
	@echo "Building"
	make all

# Pattern rule for object files
.SUFFIXES: .o .c
.c.o:
	gcc -c $< -o $@"#;
        let makefile_path = create_test_makefile(temp_dir.path(), content);
        
        let tasks = parse(&makefile_path).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "build");
    }

    #[test]
    fn test_parse_ignores_dot_targets() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#".PHONY: all
.DEFAULT_GOAL := all

all:
	@echo "Building all"
	make build"#;
        let makefile_path = create_test_makefile(temp_dir.path(), content);
        
        let tasks = parse(&makefile_path).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "all");
    }
} 