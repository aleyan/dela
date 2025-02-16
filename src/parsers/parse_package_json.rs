use crate::package_manager;
use crate::types::{Task, TaskDefinitionFile, TaskFileStatus, TaskRunner};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Parse a package.json file at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    // Read and parse the package.json file
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read package.json: {}", e))?;

    let json: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;

    // Extract the scripts section
    let scripts = match json.get("scripts") {
        Some(Value::Object(scripts)) => scripts,
        Some(_) => return Err("'scripts' must be an object".to_string()),
        None => return Ok(vec![]), // No scripts defined
    };

    // Detect available package manager
    let pkg_mgr = package_manager::detect_package_manager();

    // Convert scripts to Tasks
    let mut tasks = Vec::new();
    for (name, cmd) in scripts {
        if let Value::String(cmd) = cmd {
            tasks.push(Task {
                name: name.clone(),
                file_path: path.to_path_buf(),
                runner: TaskRunner::Node(pkg_mgr.clone()),
                source_name: name.clone(),
                description: Some(format!("node script: {}", cmd)),
                shadowed_by: None, // This will be filled in by task_discovery
            });
        }
    }

    Ok(tasks)
}

/// Create a TaskDefinitionFile for a package.json
pub fn create_definition(path: &Path, status: TaskFileStatus) -> TaskDefinitionFile {
    TaskDefinitionFile {
        path: path.to_path_buf(),
        runner: TaskRunner::Node(package_manager::detect_package_manager()),
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
