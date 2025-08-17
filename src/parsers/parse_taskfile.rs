use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum TaskCommand {
    String(String),
    Map(HashMap<String, serde_yaml::Value>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum TaskDependency {
    String(String),
    Map(HashMap<String, serde_yaml::Value>),
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskfileTask {
    desc: Option<String>,
    cmds: Option<Vec<TaskCommand>>,
    deps: Option<Vec<TaskDependency>>,
    internal: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Taskfile {
    version: Option<String>,
    tasks: HashMap<String, TaskfileTask>,
}

/// Parse a Taskfile.yml file at the given path and extract tasks
pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Taskfile");

    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", file_name, e))?;

    let taskfile: Taskfile = serde_yaml::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", file_name, e))?;

    let mut tasks = Vec::new();

    for (name, task_def) in taskfile.tasks {
        // Skip tasks marked as internal
        if task_def.internal.unwrap_or(false) {
            continue;
        }

        let description = task_def.desc.or_else(|| {
            task_def.cmds.as_ref().map(|cmds| {
                if cmds.len() == 1 {
                    match &cmds[0] {
                        TaskCommand::String(cmd) => format!("command: {}", cmd),
                        TaskCommand::Map(_map) => {
                            // Just indicate it's a complex command without parsing details
                            format!("complex command")
                        }
                    }
                } else {
                    format!("multiple commands: {}", cmds.len())
                }
            })
        });

        tasks.push(Task {
            name: name.clone(),
            file_path: path.clone(),
            definition_type: TaskDefinitionType::Taskfile,
            runner: TaskRunner::Task,
            source_name: name,
            description,
            shadowed_by: None,
            disambiguated_name: None,
        });
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_taskfile() {
        let temp_dir = TempDir::new().unwrap();
        let taskfile_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_path).unwrap();

        write!(
            file,
            r#"
version: '3'
tasks:
  build:
    desc: Build the project
    cmds:
      - cargo build
  test:
    cmds:
      - cargo test
  clean:
    desc: Clean build artifacts
    deps:
      - test
    cmds:
      - cargo clean
  format:
    cmds:
      - task: build
      - two
    deps: ['clean']
  fix:
    cmds:
      - one
    deps:
      - task: build
      - two
"#
        )
        .unwrap();

        let tasks = parse(&taskfile_path).unwrap();
        assert_eq!(tasks.len(), 5);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("Build the project"));
        assert_eq!(build_task.runner, TaskRunner::Task);

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(
            test_task.description.as_deref(),
            Some("command: cargo test")
        );
        assert_eq!(test_task.runner, TaskRunner::Task);

        let clean_task = tasks.iter().find(|t| t.name == "clean").unwrap();
        assert_eq!(
            clean_task.description.as_deref(),
            Some("Clean build artifacts")
        );
        assert_eq!(clean_task.runner, TaskRunner::Task);

        let format_task = tasks.iter().find(|t| t.name == "format").unwrap();
        assert_eq!(
            format_task.description.as_deref(),
            Some("multiple commands: 2")
        );
        assert_eq!(format_task.runner, TaskRunner::Task);
    }

    #[test]
    fn test_parse_taskfile_with_internal_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let taskfile_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_path).unwrap();

        write!(
            file,
            r#"
version: '3'
tasks:
  build:
    desc: Build the project
    cmds:
      - cargo build
  test:
    cmds:
      - cargo test
  clean:
    desc: Clean build artifacts
    deps:
      - test
    cmds:
      - cargo clean
  internal-task:
    desc: This task should not be exposed
    internal: true
    cmds:
      - echo "This is an internal task"
  helper:
    desc: Another internal task
    internal: true
    cmds:
      - echo "Helper task"
"#
        )
        .unwrap();

        let tasks = parse(&taskfile_path).unwrap();

        // Only 3 tasks should be returned, the 2 internal tasks should be filtered out
        assert_eq!(tasks.len(), 3);

        // Verify that the internal tasks are not included
        assert!(tasks.iter().find(|t| t.name == "internal-task").is_none());
        assert!(tasks.iter().find(|t| t.name == "helper").is_none());

        // Verify the normal tasks are included
        assert!(tasks.iter().find(|t| t.name == "build").is_some());
        assert!(tasks.iter().find(|t| t.name == "test").is_some());
        assert!(tasks.iter().find(|t| t.name == "clean").is_some());
    }
    #[test]
    fn test_parse_taskfile_with_nested_commands() {
        let temp_dir = TempDir::new().unwrap();
        let taskfile_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_path).unwrap();

        write!(
            file,
            r#"
version: '3'
tasks:
  build:
    desc: echo to the world
    cmds:
      - cmd: |
          echo "Hello, world!"
          echo "Hello, world!"
        silent: true
"#
        )
        .unwrap();

        let tasks = parse(&taskfile_path).unwrap();

        // Should have 1 task
        assert_eq!(tasks.len(), 1);

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.description.as_deref(), Some("echo to the world"));
        assert_eq!(build_task.runner, TaskRunner::Task);
    }

    #[test]
    fn test_parse_taskfile_with_shell_command() {
        let temp_dir = TempDir::new().unwrap();
        let taskfile_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_path).unwrap();

        write!(
            file,
            r#"
version: '3'
tasks:
  task-args:
    desc: Task that accepts and prints arguments
    cmds:
      - echo "Arguments received: {{.CLI_ARGS | join ' '}}"
"#
        )
        .unwrap();

        let tasks = parse(&taskfile_path).unwrap();

        // Should have 1 task
        assert_eq!(tasks.len(), 1);

        let args_task = tasks.iter().find(|t| t.name == "task-args").unwrap();
        assert_eq!(args_task.description.as_deref(), Some("Task that accepts and prints arguments"));
        assert_eq!(args_task.runner, TaskRunner::Task);
    }
}
