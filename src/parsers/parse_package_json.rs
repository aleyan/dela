use crate::package_manager::{self, PackageManager};
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus, TaskRunner};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// Parse a package.json file at the given path and extract tasks
pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read package.json: {}", e))?;

    let json: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;

    let mut tasks = Vec::new();

    // Get the package manager to use
    let pkg_mgr = package_manager::detect_package_manager()
        .ok_or_else(|| "No package manager found".to_string())?;

    if let Some(scripts) = json.get("scripts") {
        if let Some(scripts_obj) = scripts.as_object() {
            for (name, cmd) in scripts_obj {
                tasks.push(Task {
                    name: name.clone(),
                    file_path: path.clone(),
                    definition_type: TaskDefinitionType::PackageJson,
                    runner: TaskRunner::Node(pkg_mgr.clone()),
                    source_name: name.clone(),
                    description: cmd.as_str().map(|s| format!("node script: {}", s)),
                    shadowed_by: None,
                });
            }
        }
    }

    Ok(tasks)
}

/// Create a TaskDefinitionFile for a package.json
pub fn create_definition(path: &PathBuf, status: TaskFileStatus) -> TaskDefinitionFile {
    TaskDefinitionFile {
        path: path.clone(),
        definition_type: TaskDefinitionType::PackageJson,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        let content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest",
                "build": "tsc"
            }
        }"#;

        File::create(&package_json_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&package_json_path).unwrap();

        assert_eq!(tasks.len(), 2);

        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        match &test_task.runner {
            TaskRunner::Node(_) => (),
            _ => panic!("Expected Node task runner"),
        }
        assert_eq!(test_task.description, Some("node script: jest".to_string()));

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        match &build_task.runner {
            TaskRunner::Node(_) => (),
            _ => panic!("Expected Node task runner"),
        }
        assert_eq!(build_task.description, Some("node script: tsc".to_string()));
    }

    #[test]
    fn test_parse_invalid_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        let content = r#"{ invalid json }"#;
        File::create(&package_json_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let result = parse(&package_json_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_package_json_no_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        let content = r#"{
            "name": "test-package"
        }"#;

        File::create(&package_json_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&package_json_path).unwrap();
        assert!(tasks.is_empty());
    }
}
