use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Information about what shadows a task name
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowType {
    /// Task is shadowed by a shell builtin
    ShellBuiltin(String), // shell name
    /// Task is shadowed by an executable in PATH
    PathExecutable(String), // full path
}

/// Different types of task definition files supported by dela
#[derive(Debug, Clone, PartialEq)]
pub enum TaskDefinitionType {
    /// Makefile
    Makefile,
    /// package.json scripts
    PackageJson,
    /// pyproject.toml scripts
    PyprojectToml,
    /// Shell script
    ShellScript,
    /// Taskfile.yml
    Taskfile,
    /// Maven pom.xml
    MavenPom,
    /// Gradle build files (build.gradle, build.gradle.kts)
    Gradle,
    /// GitHub Actions workflow files
    GitHubActions,
    /// Docker Compose files
    DockerCompose,
    /// Travis CI configuration files
    TravisCi,
    /// CMake CMakeLists.txt files
    CMake,
}

/// Different types of task runners supported by dela.
/// Each variant represents a specific task runner that can execute tasks.
/// The runner is selected based on the task definition file type and available commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskRunner {
    /// Make tasks from Makefile
    /// Used when a Makefile is present in the project root
    Make,
    /// Node.js tasks using npm
    /// Selected when package.json is present with package-lock.json, or npm is the only available runner
    NodeNpm,
    /// Node.js tasks using yarn
    /// Selected when yarn.lock is present, or yarn is the preferred available runner
    NodeYarn,
    /// Node.js tasks using pnpm
    /// Selected when pnpm-lock.yaml is present, or pnpm is the preferred available runner
    NodePnpm,
    /// Node.js tasks using bun
    /// Selected when bun.lockb is present, or bun is the preferred available runner
    NodeBun,
    /// Python tasks using uv
    /// Selected when .venv directory is present, or uv is the preferred available runner
    PythonUv,
    /// Python tasks using poetry
    /// Selected when poetry.lock is present, or poetry is the preferred available runner
    PythonPoetry,
    /// Python tasks using poethepoet
    /// Selected when poe is available and no other Python runner is preferred
    PythonPoe,
    /// Shell script tasks
    /// Used for direct execution of shell scripts
    ShellScript,
    /// Task runner for Taskfile.yml
    /// Used when Taskfile.yml is present
    Task,
    /// Maven tasks runner
    /// Used when pom.xml is present
    Maven,
    /// Gradle tasks runner
    /// Used when build.gradle or build.gradle.kts is present
    Gradle,
    /// Act task runner for GitHub Actions
    /// Used for running GitHub Actions workflows locally
    Act,
    /// Docker Compose task runner
    /// Used when docker-compose.yml is present
    DockerCompose,
    /// Travis CI task runner
    /// Used when .travis.yml is present (note: tasks are listed but not executable)
    TravisCi,
    /// CMake task runner
    /// Used when CMakeLists.txt is present
    CMake,
}

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
    /// Type of the task definition file
    pub definition_type: TaskDefinitionType,
    /// Status of the file
    pub status: TaskFileStatus,
}

/// Collection of discovered task definition files
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct DiscoveredTaskDefinitions {
    /// Makefile if found
    pub makefile: Option<TaskDefinitionFile>,
    /// package.json if found
    pub package_json: Option<TaskDefinitionFile>,
    /// pyproject.toml if found
    pub pyproject_toml: Option<TaskDefinitionFile>,
    /// Taskfile.yml if found
    pub taskfile: Option<TaskDefinitionFile>,
    /// Maven pom.xml if found
    pub maven_pom: Option<TaskDefinitionFile>,
    /// Gradle build files (build.gradle, build.gradle.kts) if found
    pub gradle: Option<TaskDefinitionFile>,
    /// GitHub Actions workflow files if found
    pub github_actions: Option<TaskDefinitionFile>,
    /// Docker Compose files if found
    pub docker_compose: Option<TaskDefinitionFile>,
}

/// Represents a discovered task that can be executed
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Name of the task (e.g., "build", "test", "start")
    pub name: String,
    /// Path to the file containing this task
    pub file_path: PathBuf,
    /// The type of definition file this task came from
    pub definition_type: TaskDefinitionType,
    /// The type of runner needed for this task
    pub runner: TaskRunner,
    /// Original task name in the source file (might be different from name)
    pub source_name: String,
    /// Description of the task if available
    pub description: Option<String>,
    /// Information about what shadows this task, if anything
    pub shadowed_by: Option<ShadowType>,
    /// Disambiguated task name if the task name is ambiguous
    pub disambiguated_name: Option<String>,
}

impl TaskRunner {
    /// Get the command to run a task with this runner
    pub fn get_command(&self, task: &Task) -> String {
        match self {
            TaskRunner::Make => format!("make {}", task.source_name),
            TaskRunner::NodeNpm => format!("npm run {}", task.source_name),
            TaskRunner::NodeYarn => format!("yarn run {}", task.source_name),
            TaskRunner::NodePnpm => format!("pnpm run {}", task.source_name),
            TaskRunner::NodeBun => format!("bun run {}", task.source_name),
            TaskRunner::PythonUv => format!("uv run {}", task.source_name),
            TaskRunner::PythonPoetry => format!("poetry run {}", task.source_name),
            TaskRunner::PythonPoe => format!("poe {}", task.source_name),
            TaskRunner::ShellScript => format!("./{}", task.source_name),
            TaskRunner::Task => format!("task {}", task.source_name),
            TaskRunner::Maven => format!("mvn {}", task.source_name),
            TaskRunner::Gradle => format!("gradle {}", task.source_name),
            TaskRunner::Act => format!("act -W {}", task.file_path.display()),
            TaskRunner::DockerCompose => {
                if task.source_name == "up" {
                    "docker compose up".to_string()
                } else if task.source_name == "down" {
                    "docker compose down".to_string()
                } else {
                    format!("docker compose run {}", task.source_name)
                }
            }
            TaskRunner::TravisCi => {
                // Travis CI tasks are not executable locally
                format!(
                    "# Travis CI task '{}' - not executable locally",
                    task.source_name
                )
            }
            TaskRunner::CMake => {
                format!(
                    "cmake -S . -B build && cmake --build build --target {}",
                    task.source_name
                )
            }
        }
    }

    /// Returns a short name for the runner used in the list format
    pub fn short_name(&self) -> &'static str {
        match self {
            TaskRunner::Make => "make",
            TaskRunner::NodeNpm => "npm",
            TaskRunner::NodeYarn => "yarn",
            TaskRunner::NodePnpm => "pnpm",
            TaskRunner::NodeBun => "bun",
            TaskRunner::PythonUv => "uv",
            TaskRunner::PythonPoetry => "poetry",
            TaskRunner::PythonPoe => "poe",
            TaskRunner::ShellScript => "sh",
            TaskRunner::Task => "task",
            TaskRunner::Maven => "mvn",
            TaskRunner::Gradle => "gradle",
            TaskRunner::Act => "act",
            TaskRunner::DockerCompose => "docker compose",
            TaskRunner::TravisCi => "travis",
            TaskRunner::CMake => "cmake",
        }
    }
}

/// Result of task discovery in a directory
#[derive(Debug, Default)]
#[allow(dead_code)]
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
    #[serde(
        serialize_with = "serialize_path",
        deserialize_with = "deserialize_path"
    )]
    pub path: PathBuf,
    /// The scope of the user's decision
    pub scope: AllowScope,
    /// If scope is Task, hold the list of allowed tasks
    pub tasks: Option<Vec<String>>,
}

fn serialize_path<S>(path: &PathBuf, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&path.to_string_lossy())
}

fn deserialize_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(PathBuf::from(s))
}

/// The full allowlist with multiple entries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Allowlist {
    #[serde(default)]
    pub entries: Vec<AllowlistEntry>,
}
