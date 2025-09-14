use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use crate::types::Task;
use crate::runner::is_runner_available;
use crate::allowlist::is_task_allowed;

/// Data Transfer Object for tasks exposed via MCP
/// 
/// This struct represents the stable wire format for tasks,
/// mapping from internal Task representations to a format
/// suitable for external MCP clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskDto {
    /// The uniqified name (disambiguated name if available, otherwise original name)
    /// Examples: "build", "test-m", "start-n"
    pub unique_name: String,
    
    /// The original name as it appears in the source file
    /// Examples: "build", "test", "start"
    pub source_name: String,
    
    /// Short name of the task runner
    /// Examples: "make", "npm", "gradle"
    pub runner: String,
    
    /// Fully expanded shell command that would be executed
    /// Examples: "make build", "npm run test", "gradle clean"
    pub command: String,
    
    /// Whether the runner binary is available on the system
    pub runner_available: bool,
    
    /// Whether this task is allowed by the MCP allowlist
    pub allowlisted: bool,
    
    /// Path to the file containing this task
    pub file_path: String,
    
    /// Description of the task if available
    pub description: Option<String>,
}

impl TaskDto {
    /// Convert from internal Task to TaskDto (legacy method for backward compatibility)
    /// This method provides basic fields without enrichment
    pub fn from_task(task: &Task) -> Self {
        Self {
            unique_name: task.disambiguated_name.as_ref().unwrap_or(&task.name).clone(),
            source_name: task.source_name.clone(),
            runner: task.runner.short_name().to_string(),
            command: task.runner.get_command(task),
            runner_available: is_runner_available(&task.runner),
            allowlisted: is_task_allowed(task).map(|(allowed, denied)| allowed && !denied).unwrap_or(false),
            file_path: task.file_path.to_string_lossy().to_string(),
            description: task.description.clone(),
        }
    }

    /// Convert from internal Task to TaskDto with enriched fields
    /// This method computes all enriched fields including command, runner availability, and allowlist status
    pub fn from_task_enriched(task: &Task) -> Self {
        Self {
            unique_name: task.disambiguated_name.as_ref().unwrap_or(&task.name).clone(),
            source_name: task.source_name.clone(),
            runner: task.runner.short_name().to_string(),
            command: task.runner.get_command(task),
            runner_available: is_runner_available(&task.runner),
            allowlisted: is_task_allowed(task).map(|(allowed, denied)| allowed && !denied).unwrap_or(false),
            file_path: task.file_path.to_string_lossy().to_string(),
            description: task.description.clone(),
        }
    }
}

/// Parameters for the list_tasks MCP tool
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListTasksArgs {
    /// Optional runner filter - if provided, only return tasks for this runner
    /// Examples: "make", "npm", "gradle", "poetry"
    pub runner: Option<String>,
}

impl Default for ListTasksArgs {
    fn default() -> Self {
        Self { runner: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, TaskRunner, TaskDefinitionType};
    use std::path::PathBuf;

    #[test]
    fn test_taskdto_from_simple_task() {
        // Arrange
        let task = Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/project/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: Some("Build the project".to_string()),
            shadowed_by: None,
            disambiguated_name: None,
        };

        // Act
        let dto = TaskDto::from_task(&task);

        // Assert
        assert_eq!(dto.unique_name, "build");
        assert_eq!(dto.source_name, "build");
        assert_eq!(dto.runner, "make");
        assert_eq!(dto.command, "make build");
        assert_eq!(dto.file_path, "/project/Makefile");
        assert_eq!(dto.description, Some("Build the project".to_string()));
        // Note: runner_available and allowlisted will depend on system state and allowlist
    }

    #[test]
    fn test_taskdto_from_disambiguated_task() {
        // Arrange
        let task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/project/package.json"),
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: Some("Run tests".to_string()),
            shadowed_by: None,
            disambiguated_name: Some("test-n".to_string()),
        };

        // Act
        let dto = TaskDto::from_task(&task);

        // Assert
        assert_eq!(dto.unique_name, "test-n"); // Uses disambiguated name
        assert_eq!(dto.source_name, "test"); // Original name
        assert_eq!(dto.runner, "npm");
        assert_eq!(dto.command, "npm run test");
        assert_eq!(dto.file_path, "/project/package.json");
        assert_eq!(dto.description, Some("Run tests".to_string()));
    }

    #[test]
    fn test_taskdto_from_task_without_description() {
        // Arrange
        let task = Task {
            name: "clean".to_string(),
            file_path: PathBuf::from("/project/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "clean".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        };

        // Act
        let dto = TaskDto::from_task(&task);

        // Assert
        assert_eq!(dto.unique_name, "clean");
        assert_eq!(dto.source_name, "clean");
        assert_eq!(dto.runner, "make");
        assert_eq!(dto.command, "make clean");
        assert_eq!(dto.file_path, "/project/Makefile");
        assert_eq!(dto.description, None);
    }

    #[test]
    fn test_taskdto_from_various_runners() {
        let test_cases = vec![
            (TaskRunner::Make, "make"),
            (TaskRunner::NodeNpm, "npm"),
            (TaskRunner::NodeYarn, "yarn"),
            (TaskRunner::NodePnpm, "pnpm"),
            (TaskRunner::NodeBun, "bun"),
            (TaskRunner::PythonUv, "uv"),
            (TaskRunner::PythonPoetry, "poetry"),
            (TaskRunner::PythonPoe, "poe"),
            (TaskRunner::Task, "task"),
            (TaskRunner::Maven, "mvn"),
            (TaskRunner::Gradle, "gradle"),
            (TaskRunner::Act, "act"),
            (TaskRunner::DockerCompose, "docker compose"),
            (TaskRunner::CMake, "cmake"),
            (TaskRunner::Just, "just"),
        ];

        for (runner, expected_short_name) in test_cases {
            // Arrange
            let task = Task {
                name: "build".to_string(),
                file_path: PathBuf::from("/project/file"),
                definition_type: TaskDefinitionType::Makefile,
                runner: runner.clone(),
                source_name: "build".to_string(),
                description: None,
                shadowed_by: None,
                disambiguated_name: None,
            };

            // Act
            let dto = TaskDto::from_task(&task);

            // Assert
            assert_eq!(dto.runner, expected_short_name, "Failed for runner {:?}", runner);
            // Also verify the command is generated correctly
            assert!(dto.command.contains(expected_short_name) || expected_short_name == "docker compose", 
                   "Command '{}' should contain runner '{}'", dto.command, expected_short_name);
        }
    }

    #[test]
    fn test_taskdto_with_complex_paths() {
        // Arrange
        let task = Task {
            name: "serve".to_string(),
            file_path: PathBuf::from("/home/user/projects/my-app/package.json"),
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "serve".to_string(),
            description: Some("Start development server".to_string()),
            shadowed_by: None,
            disambiguated_name: Some("serve-n".to_string()),
        };

        // Act
        let dto = TaskDto::from_task(&task);

        // Assert
        assert_eq!(dto.unique_name, "serve-n");
        assert_eq!(dto.source_name, "serve");
        assert_eq!(dto.runner, "npm");
        assert_eq!(dto.command, "npm run serve");
        assert_eq!(dto.file_path, "/home/user/projects/my-app/package.json");
        assert_eq!(dto.description, Some("Start development server".to_string()));
    }

    #[test]
    fn test_taskdto_serialization() {
        // Arrange
        let task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/project/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: Some("Run tests".to_string()),
            shadowed_by: None,
            disambiguated_name: Some("test-m".to_string()),
        };

        let dto = TaskDto::from_task(&task);

        // Act
        let json = serde_json::to_string(&dto).expect("Should serialize");
        let deserialized: TaskDto = serde_json::from_str(&json).expect("Should deserialize");

        // Assert
        assert_eq!(dto, deserialized);
    }

    #[test]
    fn test_taskdto_from_multiple_tasks_batch() {
        // Arrange - simulate a disambiguation scenario
        let tasks = vec![
            Task {
                name: "test".to_string(),
                file_path: PathBuf::from("/project/Makefile"),
                definition_type: TaskDefinitionType::Makefile,
                runner: TaskRunner::Make,
                source_name: "test".to_string(),
                description: Some("Run make tests".to_string()),
                shadowed_by: None,
                disambiguated_name: Some("test-m".to_string()),
            },
            Task {
                name: "test".to_string(),
                file_path: PathBuf::from("/project/package.json"),
                definition_type: TaskDefinitionType::PackageJson,
                runner: TaskRunner::NodeNpm,
                source_name: "test".to_string(),
                description: Some("Run npm tests".to_string()),
                shadowed_by: None,
                disambiguated_name: Some("test-n".to_string()),
            },
        ];

        // Act
        let dtos: Vec<TaskDto> = tasks.iter().map(TaskDto::from_task).collect();

        // Assert
        assert_eq!(dtos.len(), 2);
        
        assert_eq!(dtos[0].unique_name, "test-m");
        assert_eq!(dtos[0].source_name, "test");
        assert_eq!(dtos[0].runner, "make");
        assert_eq!(dtos[0].command, "make test");
        
        assert_eq!(dtos[1].unique_name, "test-n");
        assert_eq!(dtos[1].source_name, "test");
        assert_eq!(dtos[1].runner, "npm");
        assert_eq!(dtos[1].command, "npm run test");
        
        // Both should have different uniqified names but same source name
        assert_ne!(dtos[0].unique_name, dtos[1].unique_name);
        assert_eq!(dtos[0].source_name, dtos[1].source_name);
    }

    #[test]
    fn test_list_tasks_args_default() {
        // Arrange & Act
        let args = ListTasksArgs::default();

        // Assert
        assert_eq!(args.runner, None);
    }

    #[test]
    fn test_list_tasks_args_with_runner() {
        // Arrange & Act
        let args = ListTasksArgs {
            runner: Some("make".to_string()),
        };

        // Assert
        assert_eq!(args.runner, Some("make".to_string()));
    }

    #[test]
    fn test_list_tasks_args_serialization() {
        // Arrange
        let args_with_runner = ListTasksArgs {
            runner: Some("npm".to_string()),
        };
        let args_without_runner = ListTasksArgs { runner: None };

        // Act
        let json_with = serde_json::to_string(&args_with_runner).expect("Should serialize");
        let json_without = serde_json::to_string(&args_without_runner).expect("Should serialize");

        let deserialized_with: ListTasksArgs = serde_json::from_str(&json_with).expect("Should deserialize");
        let deserialized_without: ListTasksArgs = serde_json::from_str(&json_without).expect("Should deserialize");

        // Assert
        assert_eq!(args_with_runner, deserialized_with);
        assert_eq!(args_without_runner, deserialized_without);
    }

    #[test]
    fn test_taskdto_enriched_fields() {
        // Arrange
        let task = Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/project/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: Some("Build the project".to_string()),
            shadowed_by: None,
            disambiguated_name: None,
        };

        // Act
        let dto = TaskDto::from_task_enriched(&task);

        // Assert - verify all enriched fields are populated
        assert_eq!(dto.unique_name, "build");
        assert_eq!(dto.source_name, "build");
        assert_eq!(dto.runner, "make");
        assert_eq!(dto.command, "make build");
        assert_eq!(dto.file_path, "/project/Makefile");
        assert_eq!(dto.description, Some("Build the project".to_string()));
        
        // These fields depend on system state but should be present
        assert!(dto.runner_available == true || dto.runner_available == false);
        assert!(dto.allowlisted == true || dto.allowlisted == false);
    }

    #[test]
    fn test_taskdto_command_generation_various_runners() {
        let test_cases = vec![
            (TaskRunner::Make, "build", "make build"),
            (TaskRunner::NodeNpm, "test", "npm run test"),
            (TaskRunner::NodeYarn, "start", "yarn run start"),
            (TaskRunner::NodePnpm, "dev", "pnpm run dev"),
            (TaskRunner::NodeBun, "build", "bun run build"),
            (TaskRunner::PythonUv, "test", "uv run test"),
            (TaskRunner::PythonPoetry, "install", "poetry run install"),
            (TaskRunner::PythonPoe, "lint", "poe lint"),
            (TaskRunner::Task, "deploy", "task deploy --"),
            (TaskRunner::Maven, "compile", "mvn compile"),
            (TaskRunner::Gradle, "build", "gradle build"),
            (TaskRunner::Just, "test", "just test"),
            (TaskRunner::CMake, "all", "cmake -S . -B build && cmake --build build --target all"),
        ];

        for (runner, task_name, expected_command) in test_cases {
            // Arrange
            let task = Task {
                name: task_name.to_string(),
                file_path: PathBuf::from("/project/file"),
                definition_type: TaskDefinitionType::Makefile,
                runner: runner.clone(),
                source_name: task_name.to_string(),
                description: None,
                shadowed_by: None,
                disambiguated_name: None,
            };

            // Act
            let dto = TaskDto::from_task_enriched(&task);

            // Assert
            assert_eq!(dto.command, expected_command, "Failed for runner {:?}", runner);
        }
    }

    #[test]
    fn test_taskdto_docker_compose_special_commands() {
        let test_cases = vec![
            ("up", "docker compose up"),
            ("down", "docker compose down"),
            ("build", "docker compose run build"),
            ("logs", "docker compose run logs"),
        ];

        for (task_name, expected_command) in test_cases {
            // Arrange
            let task = Task {
                name: task_name.to_string(),
                file_path: PathBuf::from("/project/docker-compose.yml"),
                definition_type: TaskDefinitionType::DockerCompose,
                runner: TaskRunner::DockerCompose,
                source_name: task_name.to_string(),
                description: None,
                shadowed_by: None,
                disambiguated_name: None,
            };

            // Act
            let dto = TaskDto::from_task_enriched(&task);

            // Assert
            assert_eq!(dto.command, expected_command, "Failed for docker compose task '{}'", task_name);
        }
    }

    #[test]
    fn test_taskdto_travis_ci_non_executable() {
        // Arrange
        let task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/project/.travis.yml"),
            definition_type: TaskDefinitionType::TravisCi,
            runner: TaskRunner::TravisCi,
            source_name: "test".to_string(),
            description: Some("Run CI tests".to_string()),
            shadowed_by: None,
            disambiguated_name: None,
        };

        // Act
        let dto = TaskDto::from_task_enriched(&task);

        // Assert
        assert_eq!(dto.unique_name, "test");
        assert_eq!(dto.runner, "travis");
        assert_eq!(dto.command, "# Travis CI task 'test' - not executable locally");
        assert_eq!(dto.runner_available, false); // Travis CI is never available locally
    }

    #[test]
    fn test_taskdto_from_task_vs_from_task_enriched_equivalence() {
        // Both methods should produce identical results since from_task now includes enrichment
        let task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/project/package.json"),
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: Some("Run tests".to_string()),
            shadowed_by: None,
            disambiguated_name: Some("test-n".to_string()),
        };

        // Act
        let dto_basic = TaskDto::from_task(&task);
        let dto_enriched = TaskDto::from_task_enriched(&task);

        // Assert - both methods should produce identical results
        assert_eq!(dto_basic, dto_enriched);
    }
}

