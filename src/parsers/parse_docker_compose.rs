use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
        assert_eq!(tasks.len(), 3);

        // Check that all services are found
        let service_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
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
        assert_eq!(tasks.len(), 0);
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
        assert_eq!(tasks.len(), 2);

        let service_names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"app"));
        assert!(service_names.contains(&"api"));

        // Check that build services have appropriate descriptions
        for task in &tasks {
            assert!(task.description.as_ref().unwrap().contains("build"));
        }
    }
}
