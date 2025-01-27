use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Status of a task definition file
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum TaskFileStatus {
    /// File exists and was successfully parsed
    Parsed,
    /// File exists but parsing is not yet implemented
    NotImplemented,
    /// File exists but had parsing errors
    ParseError(String),
    /// File exists but is not readable
    NotReadable(String),
    /// File does not exist
    NotFound,
}

/// Information about a task definition file
#[derive(Debug, Clone, PartialEq)]
pub struct TaskDefinitionFile {
    /// Path to the task definition file
    pub path: PathBuf,
    /// Type of the task runner for this file
    pub runner: TaskRunner,
    /// Status of the file
    pub status: TaskFileStatus,
}

/// Collection of discovered task definition files
#[derive(Debug, Default)]
pub struct DiscoveredTaskDefinitions {
    /// Makefile if found
    pub makefile: Option<TaskDefinitionFile>,
    /// package.json if found
    pub package_json: Option<TaskDefinitionFile>,
    /// pyproject.toml if found
    pub pyproject_toml: Option<TaskDefinitionFile>,
}

/// Represents a discovered task that can be executed
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Name of the task (e.g., "build", "test", "start")
    pub name: String,
    /// Path to the file containing this task
    pub file_path: PathBuf,
    /// The type of runner needed for this task
    pub runner: TaskRunner,
    /// Original task name in the source file (might be different from name)
    pub source_name: String,
    /// Description of the task if available
    pub description: Option<String>,
}

/// Different types of task runners supported by dela
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum TaskRunner {
    /// Make tasks from Makefile
    Make,
    /// npm scripts from package.json
    Npm,
    /// Python scripts from pyproject.toml using uv
    PythonUv,
    /// Python scripts from pyproject.toml using poetry
    PythonPoetry,
    /// Direct shell script execution
    ShellScript,
    // TODO(DTKT-52): Add plugin support for custom runners
}

impl TaskRunner {
    /// Get the command to run a task with this runner
    pub fn get_command(&self, task: &Task) -> String {
        match self {
            TaskRunner::Make => format!("make {}", task.source_name),
            TaskRunner::Npm => format!("npm run {}", task.source_name),
            TaskRunner::PythonUv => format!("uv run {}", task.source_name),
            TaskRunner::PythonPoetry => format!("poetry run {}", task.source_name),
            TaskRunner::ShellScript => format!("./{}", task.source_name),
        }
    }
}

/// Result of task discovery in a directory
#[derive(Debug, Default)]
pub struct DiscoveredTasks {
    /// All tasks found, grouped by name
    pub tasks: Vec<Task>,
    /// Any errors encountered during discovery
    pub errors: Vec<String>,
    /// Information about discovered task definition files
    pub definitions: DiscoveredTaskDefinitions,
}

/// Represents the scope of user approval
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AllowScope {
    /// Allow only once (not persisted for future runs)
    Once,
    /// Allow only this specific task
    Task,
    /// Allow all tasks from a specific file
    File,
    /// Allow all tasks from a directory (recursively)
    Directory,
    /// Deny execution
    Deny,
}

/// A single allowlist entry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllowlistEntry {
    /// The file or directory path
    pub path: PathBuf,
    /// The scope of the user’s decision
    pub scope: AllowScope,
    /// If scope is Task, hold the list of allowed tasks
    pub tasks: Option<Vec<String>>,
}

/// The full allowlist with multiple entries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Allowlist {
    pub entries: Vec<AllowlistEntry>,
}