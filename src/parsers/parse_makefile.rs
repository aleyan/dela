use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus, TaskRunner};
use makefile_lossless::Makefile;
use std::path::{Path, PathBuf};

/// Parse a Makefile at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read Makefile: {}", e))?;

    let makefile = Makefile::read(std::io::Cursor::new(content))
        .map_err(|e| format!("Failed to parse Makefile: {}", e))?;

    let mut tasks = Vec::new();

    for rule in makefile.rules() {
        // Skip pattern rules and those starting with '.'
        let targets = rule.targets().collect::<Vec<_>>();
        if targets.is_empty() || targets[0].contains('%') || targets[0].starts_with('.') {
            continue;
        }

        let name = targets[0].to_string();
        let description = rule.recipes().collect::<Vec<_>>().first().and_then(|line| {
            if line.starts_with('#') {
                Some(line.trim_start_matches('#').trim().to_string())
            } else if line.contains("@echo") {
                let parts: Vec<&str> = line.split("@echo").collect();
                if parts.len() > 1 {
                    Some(parts[1].trim().trim_matches('"').to_string())
                } else {
                    None
                }
            } else {
                None
            }
        });

        tasks.push(Task {
            name: name.clone(),
            file_path: path.to_path_buf(),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: name,
            description,
            shadowed_by: None,
        });
    }

    Ok(tasks)
}

/// Create a TaskDefinitionFile for a Makefile
pub fn create_definition(path: &Path, status: TaskFileStatus) -> TaskDefinitionFile {
    TaskDefinitionFile {
        path: path.to_path_buf(),
        definition_type: TaskDefinitionType::Makefile,
        status,
    }
}

/// Extract a task description from a rule's commands
fn extract_task_description(rule: &makefile_lossless::Rule) -> Option<String> {
    // Look for echo commands that might be descriptions
    for cmd in rule.recipes() {
        let cmd = cmd.trim();
        if cmd.starts_with("@echo") || cmd.starts_with("echo") {
            let desc = cmd
                .trim_start_matches("@echo")
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
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_makefile(dir: &Path, content: &str) -> PathBuf {
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
        assert_eq!(
            build_task.description,
            Some("Building the project".to_string())
        );

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
