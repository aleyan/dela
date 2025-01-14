use std::fs;
use std::path::Path;
use serde_json::Value;
use crate::types::{Task, TaskRunner};

/// Parse a package.json file at the given path and extract tasks
pub fn parse(path: &Path) -> Result<Vec<Task>, String> {
    // Read and parse the package.json file
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read package.json: {}", e))?;
    
    let json: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;
    
    // Extract the scripts section
    let scripts = match json.get("scripts") {
        Some(Value::Object(scripts)) => scripts,
        Some(_) => return Err("'scripts' must be an object".to_string()),
        None => return Ok(vec![]), // No scripts defined
    };

    // Convert scripts to Tasks
    let mut tasks = Vec::new();
    for (name, cmd) in scripts {
        if let Value::String(cmd) = cmd {
            tasks.push(Task {
                name: name.clone(),
                file_path: path.to_path_buf(),
                runner: TaskRunner::Npm,
                source_name: name.clone(),
                description: Some(format!("npm script: {}", cmd)),
            });
        }
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_valid_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_json = temp_dir.path().join("package.json");
        
        let content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest",
                "build": "tsc",
                "start": "node dist/index.js"
            }
        }"#;
        
        File::create(&package_json)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&package_json).unwrap();
        
        assert_eq!(tasks.len(), 3);
        
        let test_task = tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Npm);
        assert_eq!(test_task.source_name, "test");
        assert_eq!(test_task.description, Some("npm script: jest".to_string()));
        
        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Npm);
        assert_eq!(build_task.source_name, "build");
        assert_eq!(build_task.description, Some("npm script: tsc".to_string()));
    }

    #[test]
    fn test_parse_package_json_no_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let package_json = temp_dir.path().join("package.json");
        
        let content = r#"{
            "name": "test-package"
        }"#;
        
        File::create(&package_json)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&package_json).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_invalid_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_json = temp_dir.path().join("package.json");
        
        let content = r#"{
            "name": "test-package",
            "scripts": "invalid"
        }"#;
        
        File::create(&package_json)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let result = parse(&package_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("'scripts' must be an object"));
    }
} 