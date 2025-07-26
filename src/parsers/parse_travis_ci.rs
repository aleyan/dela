use crate::types::{Task, TaskDefinitionType, TaskRunner};
use serde_yaml::Value;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Parse Travis CI configuration file and extract jobs as tasks
///
/// This function parses a .travis.yml file and extracts each job as a separate task.
/// Note: Travis CI tasks are listed for discovery but cannot be executed locally.
pub fn parse(file_path: &Path) -> Result<Vec<Task>, String> {
    let mut file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    parse_travis_string(&contents, file_path)
}

/// Parse Travis CI configuration content from a string
fn parse_travis_string(content: &str, file_path: &Path) -> Result<Vec<Task>, String> {
    let config: Value = if content.trim().is_empty() {
        Value::Mapping(serde_yaml::Mapping::new())
    } else {
        serde_yaml::from_str(content)
            .map_err(|e| format!("Failed to parse Travis CI YAML: {}", e))?
    };

    let config_map = match config {
        Value::Mapping(map) => map,
        _ => return Err("Travis CI YAML is not a mapping".to_string()),
    };

    let mut tasks = Vec::new();

    // Extract jobs from the configuration
    if let Some(Value::Mapping(jobs_map)) = config_map.get(&Value::String("jobs".to_string())) {
        // Parse jobs section
        for (job_key, job_value) in jobs_map {
            if let Value::String(job_name) = job_key {
                let description = extract_job_description(job_value);

                let task = Task {
                    name: job_name.clone(),
                    file_path: file_path.to_path_buf(),
                    definition_type: TaskDefinitionType::TravisCi,
                    runner: TaskRunner::TravisCi,
                    source_name: job_name.clone(),
                    description,
                    shadowed_by: None,
                    disambiguated_name: None,
                };

                tasks.push(task);
            }
        }
    } else {
        // If no jobs section, look for matrix or other job definitions
        if let Some(Value::Mapping(matrix_map)) =
            config_map.get(&Value::String("matrix".to_string()))
        {
            // Handle matrix configuration
            if let Some(Value::Sequence(include_list)) =
                matrix_map.get(&Value::String("include".to_string()))
            {
                for (i, include_item) in include_list.iter().enumerate() {
                    if let Value::Mapping(include_map) = include_item {
                        if let Some(Value::String(job_name)) =
                            include_map.get(&Value::String("name".to_string()))
                        {
                            let description = extract_job_description(include_item);

                            let task = Task {
                                name: job_name.clone(),
                                file_path: file_path.to_path_buf(),
                                definition_type: TaskDefinitionType::TravisCi,
                                runner: TaskRunner::TravisCi,
                                source_name: job_name.clone(),
                                description,
                                shadowed_by: None,
                                disambiguated_name: None,
                            };

                            tasks.push(task);
                        } else {
                            // If no name, use index
                            let job_name = format!("matrix-job-{}", i);
                            let description = Some("Matrix job from Travis CI".to_string());

                            let task = Task {
                                name: job_name.clone(),
                                file_path: file_path.to_path_buf(),
                                definition_type: TaskDefinitionType::TravisCi,
                                runner: TaskRunner::TravisCi,
                                source_name: job_name.clone(),
                                description,
                                shadowed_by: None,
                                disambiguated_name: None,
                            };

                            tasks.push(task);
                        }
                    }
                }
            }
        }

        // If still no tasks found, create a default task for the entire configuration
        if tasks.is_empty() {
            let task = Task {
                name: "travis".to_string(),
                file_path: file_path.to_path_buf(),
                definition_type: TaskDefinitionType::TravisCi,
                runner: TaskRunner::TravisCi,
                source_name: "travis".to_string(),
                description: Some("Travis CI configuration".to_string()),
                shadowed_by: None,
                disambiguated_name: None,
            };

            tasks.push(task);
        }
    }

    Ok(tasks)
}

/// Extract description from a job configuration
fn extract_job_description(job_value: &Value) -> Option<String> {
    match job_value {
        Value::Mapping(job_map) => {
            // Try to get name first
            if let Some(Value::String(name)) = job_map.get(&Value::String("name".to_string())) {
                return Some(format!("Travis CI job: {}", name));
            }

            // Try to get stage
            if let Some(Value::String(stage)) = job_map.get(&Value::String("stage".to_string())) {
                return Some(format!("Travis CI job in stage: {}", stage));
            }

            // Try to get language
            if let Some(Value::String(language)) =
                job_map.get(&Value::String("language".to_string()))
            {
                return Some(format!("Travis CI {} job", language));
            }

            Some("Travis CI job".to_string())
        }
        Value::String(job_name) => Some(format!("Travis CI job: {}", job_name)),
        _ => Some("Travis CI job".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_travis_config(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let file_path = dir.join(filename);
        fs::write(&file_path, content).expect("Failed to write test Travis CI file");
        file_path
    }

    #[test]
    fn test_parse_simple_travis_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let travis_content = r#"
language: node_js
node_js:
  - "18"
  - "20"

jobs:
  test:
    name: "Test"
    stage: test
  build:
    name: "Build"
    stage: build
"#;

        let file_path = create_test_travis_config(&temp_dir.path(), ".travis.yml", travis_content);

        let tasks = parse(&file_path).expect("Failed to parse Travis CI config");

        assert_eq!(tasks.len(), 2, "Should have two tasks");

        let test_task = tasks
            .iter()
            .find(|t| t.name == "test")
            .expect("Should find test task");
        assert_eq!(test_task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(test_task.runner, TaskRunner::TravisCi);
        assert_eq!(
            test_task.description,
            Some("Travis CI job: Test".to_string())
        );

        let build_task = tasks
            .iter()
            .find(|t| t.name == "build")
            .expect("Should find build task");
        assert_eq!(build_task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(build_task.runner, TaskRunner::TravisCi);
        assert_eq!(
            build_task.description,
            Some("Travis CI job: Build".to_string())
        );
    }

    #[test]
    fn test_parse_matrix_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let travis_content = r#"
language: python

matrix:
  include:
    - name: "Python 3.8"
      python: "3.8"
    - name: "Python 3.9"
      python: "3.9"
    - name: "Python 3.10"
      python: "3.10"
"#;

        let file_path = create_test_travis_config(&temp_dir.path(), ".travis.yml", travis_content);

        let tasks = parse(&file_path).expect("Failed to parse Travis CI config");

        assert_eq!(tasks.len(), 3, "Should have three tasks");

        for task in &tasks {
            assert_eq!(task.definition_type, TaskDefinitionType::TravisCi);
            assert_eq!(task.runner, TaskRunner::TravisCi);
            assert!(
                task.description
                    .as_ref()
                    .unwrap()
                    .contains("Travis CI job:")
            );
        }
    }

    #[test]
    fn test_parse_basic_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let travis_content = r#"
language: ruby
rvm:
  - 2.7
  - 3.0
  - 3.1

script:
  - bundle install
  - bundle exec rspec
"#;

        let file_path = create_test_travis_config(&temp_dir.path(), ".travis.yml", travis_content);

        let tasks = parse(&file_path).expect("Failed to parse Travis CI config");

        assert_eq!(tasks.len(), 1, "Should have one default task");

        let task = &tasks[0];
        assert_eq!(task.name, "travis");
        assert_eq!(task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(task.runner, TaskRunner::TravisCi);
        assert_eq!(
            task.description,
            Some("Travis CI configuration".to_string())
        );
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let invalid_content = r#"
language: node_js
node_js:
  - "18"
  - "20"

jobs:
  test:
    name: "Test"
    stage: test
  build:
    name: "Build"
    stage: build
    invalid: yaml: content
"#;

        let file_path = create_test_travis_config(&temp_dir.path(), ".travis.yml", invalid_content);

        let result = parse(&file_path);
        assert!(result.is_err(), "Should fail to parse invalid YAML");
    }

    #[test]
    fn test_parse_empty_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let file_path = create_test_travis_config(&temp_dir.path(), ".travis.yml", "");

        let tasks = parse(&file_path).expect("Failed to parse empty Travis CI config");

        assert_eq!(
            tasks.len(),
            1,
            "Should have one default task for empty config"
        );

        let task = &tasks[0];
        assert_eq!(task.name, "travis");
        assert_eq!(task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(task.runner, TaskRunner::TravisCi);
    }
}
