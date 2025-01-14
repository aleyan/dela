use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde_json::Value;
use crate::types::{Task, TaskRunner};

pub fn parse_package_json(path: &Path) -> Result<Vec<Task>, String> {
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
                description: format!("npm script: {}", cmd),
                runner: TaskRunner::Npm {
                    script: name.clone(),
                },
                source_file: path.to_path_buf(),
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

        let tasks = parse_package_json(&package_json).unwrap();
        
        assert_eq!(tasks.len(), 3);
        assert!(tasks.iter().any(|t| t.name == "test"));
        assert!(tasks.iter().any(|t| t.name == "build"));
        assert!(tasks.iter().any(|t| t.name == "start"));
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

        let tasks = parse_package_json(&package_json).unwrap();
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

        let result = parse_package_json(&package_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("'scripts' must be an object"));
    }
} 