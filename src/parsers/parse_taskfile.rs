use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SUPPORTED_TASKFILE_NAMES: [&str; 8] = [
    "Taskfile.yml",
    "taskfile.yml",
    "Taskfile.yaml",
    "taskfile.yaml",
    "Taskfile.dist.yml",
    "taskfile.dist.yml",
    "Taskfile.dist.yaml",
    "taskfile.dist.yaml",
];

const DEFAULT_TASKFILE_NAME: &str = SUPPORTED_TASKFILE_NAMES[0];

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
#[serde(untagged)]
enum TaskfileIncludeEntry {
    Shorthand(String),
    Detailed(TaskfileIncludeConfig),
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)] // We parse a broader subset of the schema than DTKT-200 consumes.
struct TaskfileIncludeConfig {
    taskfile: String,
    optional: Option<bool>,
    flatten: Option<bool>,
    internal: Option<bool>,
    excludes: Option<Vec<String>>,
    aliases: Option<Vec<String>>,
    dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskfileInclude {
    pub namespace: String,
    pub taskfile: PathBuf,
    pub optional: bool,
    pub flatten: bool,
    pub internal: bool,
    pub excludes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)] // Some top-level schema fields are currently only parsed for compatibility.
struct Taskfile {
    version: Option<String>,
    #[serde(default)]
    includes: HashMap<String, TaskfileIncludeEntry>,
    #[serde(default)]
    tasks: HashMap<String, TaskfileTask>,
}

pub fn find_taskfile_in_dir(dir: &Path) -> Option<PathBuf> {
    for filename in SUPPORTED_TASKFILE_NAMES {
        let path = dir.join(filename);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

pub fn resolve_taskfile_include_path(candidate: &Path) -> PathBuf {
    if candidate.is_dir() {
        return find_taskfile_in_dir(candidate)
            .unwrap_or_else(|| candidate.join(DEFAULT_TASKFILE_NAME));
    }

    if candidate.is_file() || looks_like_taskfile_file(candidate) || candidate.extension().is_some()
    {
        return candidate.to_path_buf();
    }

    candidate.join(DEFAULT_TASKFILE_NAME)
}

/// Parse a Taskfile.yml file at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    let taskfile = load_taskfile(path)?;
    let mut task_entries: Vec<_> = taskfile.tasks.into_iter().collect();
    task_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut tasks = Vec::new();

    for (name, task_def) in task_entries {
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
                            "complex command".to_string()
                        }
                    }
                } else {
                    format!("multiple commands: {}", cmds.len())
                }
            })
        });

        tasks.push(Task {
            name: name.clone(),
            file_path: path.to_path_buf(),
            definition_path: None,
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

pub fn extract_include_directives(path: &Path) -> Result<Vec<TaskfileInclude>, String> {
    let taskfile = load_taskfile(path)?;
    let mut include_entries: Vec<_> = taskfile.includes.into_iter().collect();
    include_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut includes = Vec::new();

    for (namespace, include) in include_entries {
        let include = match include {
            TaskfileIncludeEntry::Shorthand(taskfile) => TaskfileInclude {
                namespace,
                taskfile: PathBuf::from(taskfile),
                optional: false,
                flatten: false,
                internal: false,
                excludes: Vec::new(),
            },
            TaskfileIncludeEntry::Detailed(config) => TaskfileInclude {
                namespace,
                taskfile: PathBuf::from(config.taskfile),
                optional: config.optional.unwrap_or(false),
                flatten: config.flatten.unwrap_or(false),
                internal: config.internal.unwrap_or(false),
                excludes: config.excludes.unwrap_or_default(),
            },
        };

        if should_skip_non_local_include(&include.taskfile) {
            continue;
        }

        includes.push(include);
    }

    Ok(includes)
}

fn load_taskfile(path: &Path) -> Result<Taskfile, String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Taskfile");

    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", file_name, e))?;

    let taskfile = serde_yaml::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", file_name, e))?;
    Ok(taskfile)
}

fn looks_like_taskfile_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SUPPORTED_TASKFILE_NAMES.contains(&name))
}

fn should_skip_non_local_include(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.contains("://") || path.contains("{{") || path.contains("}}")
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
        assert!(!tasks.iter().any(|t| t.name == "internal-task"));
        assert!(!tasks.iter().any(|t| t.name == "helper"));

        // Verify the normal tasks are included
        assert!(tasks.iter().any(|t| t.name == "build"));
        assert!(tasks.iter().any(|t| t.name == "test"));
        assert!(tasks.iter().any(|t| t.name == "clean"));
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
      - cmd: |
          echo "Arguments received: {{.ARGS}}"
    vars:
      ARGS: "{{.CLI_ARGS | join \" \"}}"
"#
        )
        .unwrap();

        let tasks = parse(&taskfile_path).unwrap();

        // Should have 1 task
        assert_eq!(tasks.len(), 1);

        let args_task = tasks.iter().find(|t| t.name == "task-args").unwrap();
        assert_eq!(
            args_task.description.as_deref(),
            Some("Task that accepts and prints arguments")
        );
        assert_eq!(args_task.runner, TaskRunner::Task);
    }

    #[test]
    fn test_extract_include_directives() {
        let temp_dir = TempDir::new().unwrap();
        let taskfile_path = temp_dir.path().join("Taskfile.yml");

        std::fs::write(
            &taskfile_path,
            r#"
version: '3'
includes:
  docs: ./docs
  shared:
    taskfile: ./shared/Taskfile.dist.yml
    optional: true
    flatten: true
    internal: true
    excludes: [build]
  remote:
    taskfile: https://example.com/tasks.yml
  templated: ./Taskfile_{{OS}}.yml
"#,
        )
        .unwrap();

        let includes = extract_include_directives(&taskfile_path).unwrap();
        assert_eq!(includes.len(), 2);

        assert_eq!(includes[0].namespace, "docs");
        assert_eq!(includes[0].taskfile, PathBuf::from("./docs"));
        assert!(!includes[0].optional);
        assert!(!includes[0].flatten);
        assert!(!includes[0].internal);
        assert!(includes[0].excludes.is_empty());

        assert_eq!(includes[1].namespace, "shared");
        assert_eq!(
            includes[1].taskfile,
            PathBuf::from("./shared/Taskfile.dist.yml")
        );
        assert!(includes[1].optional);
        assert!(includes[1].flatten);
        assert!(includes[1].internal);
        assert_eq!(includes[1].excludes, vec!["build".to_string()]);
    }

    #[test]
    fn test_find_taskfile_in_dir_and_resolve_include_path() {
        let temp_dir = TempDir::new().unwrap();
        let include_dir = temp_dir.path().join("docs");
        std::fs::create_dir_all(&include_dir).unwrap();
        std::fs::write(include_dir.join("taskfile.yaml"), "version: '3'\n").unwrap();

        let resolved_path = find_taskfile_in_dir(&include_dir).unwrap();
        assert!(
            resolved_path == include_dir.join("Taskfile.yaml")
                || resolved_path == include_dir.join("taskfile.yaml")
        );
        let include_path = resolve_taskfile_include_path(&include_dir);
        assert!(
            include_path == include_dir.join("Taskfile.yaml")
                || include_path == include_dir.join("taskfile.yaml")
        );

        let missing_dir = temp_dir.path().join("missing");
        assert_eq!(
            resolve_taskfile_include_path(&missing_dir),
            missing_dir.join("Taskfile.yml")
        );

        let explicit_file = temp_dir.path().join("Shared.yml");
        assert_eq!(resolve_taskfile_include_path(&explicit_file), explicit_file);
    }
}
