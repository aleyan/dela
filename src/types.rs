use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Information about what shadows a task name
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowType {
    /// Task is shadowed by a shell builtin
    ShellBuiltin(String), // shell name
    /// Task is shadowed by an executable in PATH
    PathExecutable(String), // full path
}

/// Different types of task definition files supported by dela
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
    /// turbo.json
    TurboJson,
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
    /// Justfile
    Justfile,
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
    /// Turborepo tasks from turbo.json
    /// Used when turbo.json is present at the repository root
    Turbo,
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
    /// Just task runner
    /// Used when Justfile is present
    Just,
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
#[derive(Debug, Clone, Default)]
pub struct DiscoveredTaskDefinitions {
    files: std::collections::BTreeMap<TaskDefinitionType, Vec<TaskDefinitionFile>>,
}

impl DiscoveredTaskDefinitions {
    pub fn insert(&mut self, definition: TaskDefinitionFile) {
        self.files
            .entry(definition.definition_type.clone())
            .or_default()
            .push(definition);
    }

    /// Returns the first definition file for a given type, if any.
    #[allow(dead_code)]
    pub fn get_first(&self, definition_type: &TaskDefinitionType) -> Option<&TaskDefinitionFile> {
        self.files.get(definition_type).and_then(|v| v.first())
    }

    /// Returns all definition files for a given type, if any.
    #[allow(dead_code)]
    pub fn get_all(&self, definition_type: &TaskDefinitionType) -> Option<&[TaskDefinitionFile]> {
        self.files.get(definition_type).map(|v| v.as_slice())
    }

    /// Iterates over all (type, files) entries.
    pub fn iter(&self) -> impl Iterator<Item = (&TaskDefinitionType, &[TaskDefinitionFile])> {
        self.files.iter().map(|(k, v)| (k, v.as_slice()))
    }
}

/// Represents a discovered task that can be executed
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Name of the task (e.g., "build", "test", "start")
    pub name: String,
    /// Path the runner should use for execution context.
    /// For simple task definitions this is the same as the defining file.
    /// For composed definitions this may point at the root file or directory
    /// that the runner executes against.
    pub file_path: PathBuf,
    /// Path to the file that actually defines this task when it differs from
    /// the runner path. For simple task definitions this is None.
    pub definition_path: Option<PathBuf>,
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

impl Task {
    /// Return the file that actually defines this task.
    pub fn definition_path(&self) -> &Path {
        self.definition_path.as_deref().unwrap_or(&self.file_path)
    }

    /// Return the path used for allowlist matching and user-facing source attribution.
    pub fn allowlist_path(&self) -> &Path {
        self.definition_path()
    }
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
            TaskRunner::Task => format!("task {} --", task.source_name),
            TaskRunner::Turbo => format!("turbo run {}", task.source_name),
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
            TaskRunner::Just => format!("just {}", task.source_name),
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
            TaskRunner::Turbo => "turbo",
            TaskRunner::Maven => "mvn",
            TaskRunner::Gradle => "gradle",
            TaskRunner::Act => "act",
            TaskRunner::DockerCompose => "docker compose",
            TaskRunner::TravisCi => "travis",
            TaskRunner::CMake => "cmake",
            TaskRunner::Just => "just",
        }
    }
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

fn serialize_path<S>(path: &std::path::Path, serializer: S) -> Result<S::Ok, S::Error>
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_task_definitions_get_all() {
        let mut defs = DiscoveredTaskDefinitions::default();

        // 1. Assert get_all returns None on empty
        assert!(defs.get_all(&TaskDefinitionType::Makefile).is_none());

        // 2. Add multiple definitions sharing the same key, and some unique ones
        let makefile1 = TaskDefinitionFile {
            path: PathBuf::from("path/to/Makefile1"),
            definition_type: TaskDefinitionType::Makefile,
            status: TaskFileStatus::Parsed,
        };
        let makefile2 = TaskDefinitionFile {
            path: PathBuf::from("path/to/Makefile2"),
            definition_type: TaskDefinitionType::Makefile,
            status: TaskFileStatus::NotFound,
        };
        let package_json = TaskDefinitionFile {
            path: PathBuf::from("path/to/package.json"),
            definition_type: TaskDefinitionType::PackageJson,
            status: TaskFileStatus::NotImplemented,
        };

        defs.insert(makefile1.clone());
        defs.insert(makefile2.clone());
        defs.insert(package_json.clone());

        // 3. Assert get_all returns the multiple files under the same key
        let makefiles = defs.get_all(&TaskDefinitionType::Makefile).unwrap();
        assert_eq!(makefiles.len(), 2);
        assert_eq!(makefiles[0].path, makefile1.path);
        assert_eq!(makefiles[0].status, makefile1.status);
        assert_eq!(makefiles[1].path, makefile2.path);
        assert_eq!(makefiles[1].status, makefile2.status);

        // 4. Assert get_all returns the single file under its key
        let package_jsons = defs.get_all(&TaskDefinitionType::PackageJson).unwrap();
        assert_eq!(package_jsons.len(), 1);
        assert_eq!(package_jsons[0].path, package_json.path);
        assert_eq!(package_jsons[0].status, package_json.status);

        // 5. Assert get_all returns None for query on non-inserted key
        assert!(defs.get_all(&TaskDefinitionType::PyprojectToml).is_none());
    }
}
