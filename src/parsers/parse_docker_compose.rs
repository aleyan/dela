use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
struct DockerComposeService {
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    build: Option<serde_yaml::Value>,
    #[serde(default)]
    command: Option<serde_yaml::Value>,
    #[serde(default)]
    entrypoint: Option<serde_yaml::Value>,
    #[serde(default)]
    environment: Option<serde_yaml::Value>,
    #[serde(default)]
    ports: Option<serde_yaml::Value>,
    #[serde(default)]
    volumes: Option<serde_yaml::Value>,
    #[serde(default)]
    depends_on: Option<serde_yaml::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DockerCompose {
    version: Option<String>,
    services: HashMap<String, DockerComposeService>,
}

/// Parse a docker-compose.yml file at the given path and extract services as tasks
pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("docker-compose.yml");

    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", file_name, e))?;

    let docker_compose: DockerCompose = serde_yaml::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", file_name, e))?;

    let mut tasks = Vec::new();

    // Add "up" task to bring up all services
    tasks.push(Task {
        name: "up".to_string(),
        file_path: path.clone(),
        definition_type: TaskDefinitionType::DockerCompose,
        runner: TaskRunner::DockerCompose,
        source_name: "up".to_string(),
        description: Some("Bring up all Docker Compose services".to_string()),
        shadowed_by: None,
        disambiguated_name: None,
    });

    // Add "down" task to bring down all services
    tasks.push(Task {
        name: "down".to_string(),
        file_path: path.clone(),
        definition_type: TaskDefinitionType::DockerCompose,
        runner: TaskRunner::DockerCompose,
        source_name: "down".to_string(),
        description: Some("Bring down all Docker Compose services".to_string()),
        shadowed_by: None,
        disambiguated_name: None,
    });

    for (service_name, service) in docker_compose.services {
        // Create a description based on the service configuration
        let description = if let Some(image) = &service.image {
            Some(format!("Docker service using image: {}", image))
        } else if service.build.is_some() {
            Some("Docker service with custom build".to_string())
        } else {
            Some("Docker service".to_string())
        };

        tasks.push(Task {
            name: service_name.clone(),
            file_path: path.clone(),
            definition_type: TaskDefinitionType::DockerCompose,
            runner: TaskRunner::DockerCompose,
            source_name: service_name,
            description,
            shadowed_by: None,
            disambiguated_name: None,
        });
    }

    Ok(tasks)
}

/// Find Docker Compose files in the given directory, including profile files
pub fn find_docker_compose_files(dir: &Path) -> Vec<PathBuf> {
    let mut found_files = Vec::new();

    // 1. Add base files first (highest priority)
    let base_files = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];
    for filename in &base_files {
        let path = dir.join(filename);
        if path.exists() {
            found_files.push(path);
        }
    }

    // 2. Add docker-compose.<profile>.yml and .yaml files (excluding the base files)
    if let Ok(entries) = fs::read_dir(dir) {
        let mut profile_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                    fname.starts_with("docker-compose.")
                        && (fname.ends_with(".yml") || fname.ends_with(".yaml"))
                        && fname != "docker-compose.yml"
                        && fname != "docker-compose.yaml"
                } else {
                    false
                }
            })
            .collect();
        profile_files.sort(); // Lexicographical order
        found_files.extend(profile_files);
    }

    found_files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_test_docker_compose(dir: &Path, content: &str) {
        std::fs::write(dir.join("docker-compose.yml"), content).unwrap();
    }

    #[test]
    fn test_parse_docker_compose_with_services() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
  db:
    image: postgres:13
    environment:
      POSTGRES_DB: myapp
      POSTGRES_USER: user
      POSTGRES_PASSWORD: password
  app:
    build: .
    depends_on:
      - db
"#;
        create_test_docker_compose(temp_dir.path(), content);

        let result = parse(&temp_dir.path().join("docker-compose.yml"));
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 5); // 3 services + "up" + "down" tasks

        // Check that all services are found
        let service_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"web"));
        assert!(service_names.contains(&"db"));
        assert!(service_names.contains(&"app"));

        // Check task properties
        for task in &tasks {
            assert_eq!(task.definition_type, TaskDefinitionType::DockerCompose);
            assert_eq!(task.runner, TaskRunner::DockerCompose);
            assert_eq!(task.file_path, temp_dir.path().join("docker-compose.yml"));
            assert!(task.description.is_some());
        }

        // Check that "up" task has correct description
        let up_task = tasks.iter().find(|t| t.name == "up").unwrap();
        assert_eq!(
            up_task.description.as_ref().unwrap(),
            "Bring up all Docker Compose services"
        );
        // Check that "down" task has correct description
        let down_task = tasks.iter().find(|t| t.name == "down").unwrap();
        assert_eq!(
            down_task.description.as_ref().unwrap(),
            "Bring down all Docker Compose services"
        );

        // Check specific task descriptions
        let web_task = tasks.iter().find(|t| t.name == "web").unwrap();
        assert!(
            web_task
                .description
                .as_ref()
                .unwrap()
                .contains("nginx:alpine")
        );

        let app_task = tasks.iter().find(|t| t.name == "app").unwrap();
        assert!(app_task.description.as_ref().unwrap().contains("build"));
    }

    #[test]
    fn test_parse_docker_compose_empty() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
version: '3.8'
services: {}
"#;
        create_test_docker_compose(temp_dir.path(), content);

        let result = parse(&temp_dir.path().join("docker-compose.yml"));
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 2); // "up" and "down" tasks

        // Check that "up" and "down" tasks are present
        let service_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
    }

    #[test]
    fn test_parse_docker_compose_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
  db:
    image: postgres:13
    environment:
      POSTGRES_DB: myapp
      POSTGRES_USER: user
      POSTGRES_PASSWORD: password
  app:
    build: .
    depends_on:
      - db
invalid: yaml: here
"#;
        create_test_docker_compose(temp_dir.path(), content);

        let result = parse(&temp_dir.path().join("docker-compose.yml"));
        assert!(result.is_err()); // YAML should fail to parse with invalid structure
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    #[test]
    fn test_parse_docker_compose_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let result = parse(&temp_dir.path().join("docker-compose.yml"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read"));
    }

    #[test]
    fn test_parse_docker_compose_with_build_context() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
version: '3.8'
services:
  app:
    build:
      context: .
      dockerfile: Dockerfile.dev
    ports:
      - "3000:3000"
  api:
    build: ./api
    ports:
      - "8000:8000"
"#;
        create_test_docker_compose(temp_dir.path(), content);

        let result = parse(&temp_dir.path().join("docker-compose.yml"));
        assert!(result.is_ok());

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 4); // 2 services + "up" + "down" tasks

        let service_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"app"));
        assert!(service_names.contains(&"api"));

        // Check that build services have appropriate descriptions
        for task in &tasks {
            if task.name != "up" && task.name != "down" {
                assert!(task.description.as_ref().unwrap().contains("build"));
            }
        }
    }

    #[test]
    fn test_find_docker_compose_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple Docker Compose files
        std::fs::write(
            temp_dir.path().join("docker-compose.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("docker-compose.yaml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("compose.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("compose.yaml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();

        let found_files = find_docker_compose_files(temp_dir.path());
        assert_eq!(found_files.len(), 4);

        // Check that files are found in priority order
        let file_names: Vec<String> = found_files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(file_names.contains(&"docker-compose.yml".to_string()));
        assert!(file_names.contains(&"docker-compose.yaml".to_string()));
        assert!(file_names.contains(&"compose.yml".to_string()));
        assert!(file_names.contains(&"compose.yaml".to_string()));
    }

    #[test]
    fn test_find_docker_compose_files_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let found_files = find_docker_compose_files(temp_dir.path());
        assert_eq!(found_files.len(), 0);
    }

    #[test]
    fn test_find_docker_compose_files_priority_order() {
        let temp_dir = TempDir::new().unwrap();

        // Create files in reverse priority order
        std::fs::write(
            temp_dir.path().join("compose.yaml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("compose.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("docker-compose.yaml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("docker-compose.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();

        let found_files = find_docker_compose_files(temp_dir.path());
        assert_eq!(found_files.len(), 4);

        // The first file should be docker-compose.yml (highest priority)
        let first_file = found_files[0].file_name().unwrap().to_string_lossy();
        assert_eq!(first_file, "docker-compose.yml");
    }

    #[test]
    fn test_find_docker_compose_profile_files() {
        let temp_dir = TempDir::new().unwrap();
        // Create base and profile files
        std::fs::write(
            temp_dir.path().join("docker-compose.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("docker-compose.dev.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("docker-compose.prod.yaml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("docker-compose.test.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("compose.yml"),
            "version: '3.8'\nservices: {}",
        )
        .unwrap();

        let found_files = find_docker_compose_files(temp_dir.path());
        let file_names: Vec<String> = found_files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        // Base files first
        assert_eq!(file_names[0], "docker-compose.yml");
        assert_eq!(file_names[1], "compose.yml");
        // Profile files in lex order
        assert_eq!(file_names[2], "docker-compose.dev.yml");
        assert_eq!(file_names[3], "docker-compose.prod.yaml");
        assert_eq!(file_names[4], "docker-compose.test.yml");
    }
}
