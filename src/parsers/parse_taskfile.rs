use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct TaskfileTask {
    desc: Option<String>,
    cmds: Option<Vec<String>>,
    deps: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Taskfile {
    version: Option<String>,
    tasks: HashMap<String, TaskfileTask>,
}

/// Parse a Taskfile.yml file at the given path and extract tasks
pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Taskfile");

    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", file_name, e))?;

    let taskfile: Taskfile = serde_yaml::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", file_name, e))?;

    let mut tasks = Vec::new();

    for (name, task_def) in taskfile.tasks {
        let description = task_def.desc.or_else(|| {
            task_def.cmds.as_ref().map(|cmds| {
                if cmds.len() == 1 {
                    format!("command: {}", cmds[0])
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
"#
        )
        .unwrap();

        let tasks = parse(&taskfile_path).unwrap();
        assert_eq!(tasks.len(), 3);

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
    }
}
