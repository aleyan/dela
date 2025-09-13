use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use crate::types::Task;

/// Data Transfer Object for tasks exposed via MCP
/// 
/// This struct represents the stable wire format for tasks,
/// mapping from internal Task representations to a format
/// suitable for external MCP clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskDto {
    /// The uniqified name (disambiguated name if available, otherwise original name)
    /// Examples: "build", "test-m", "start-n"
    pub name: String,
    
    /// The original name as it appears in the source file
    /// Examples: "build", "test", "start"
    pub source_name: String,
    
    /// Short name of the task runner
    /// Examples: "make", "npm", "gradle"
    pub runner: String,
    
    /// Path to the file containing this task
    pub file_path: String,
    
    /// Description of the task if available
    pub description: Option<String>,
}

impl TaskDto {
    /// Convert from internal Task to TaskDto
    pub fn from_task(task: &Task) -> Self {
        Self {
            name: task.disambiguated_name.as_ref().unwrap_or(&task.name).clone(),
            source_name: task.source_name.clone(),
            runner: task.runner.short_name().to_string(),
            file_path: task.file_path.to_string_lossy().to_string(),
            description: task.description.clone(),
        }
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
        assert_eq!(dto.name, "build");
        assert_eq!(dto.source_name, "build");
        assert_eq!(dto.runner, "make");
        assert_eq!(dto.file_path, "/project/Makefile");
        assert_eq!(dto.description, Some("Build the project".to_string()));
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
        assert_eq!(dto.name, "test-n"); // Uses disambiguated name
        assert_eq!(dto.source_name, "test"); // Original name
        assert_eq!(dto.runner, "npm");
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
        assert_eq!(dto.name, "clean");
        assert_eq!(dto.source_name, "clean");
        assert_eq!(dto.runner, "make");
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
        assert_eq!(dto.name, "serve-n");
        assert_eq!(dto.source_name, "serve");
        assert_eq!(dto.runner, "npm");
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
        
        assert_eq!(dtos[0].name, "test-m");
        assert_eq!(dtos[0].source_name, "test");
        assert_eq!(dtos[0].runner, "make");
        
        assert_eq!(dtos[1].name, "test-n");
        assert_eq!(dtos[1].source_name, "test");
        assert_eq!(dtos[1].runner, "npm");
        
        // Both should have different uniqified names but same source name
        assert_ne!(dtos[0].name, dtos[1].name);
        assert_eq!(dtos[0].source_name, dtos[1].source_name);
    }
}

