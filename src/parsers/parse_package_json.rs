use crate::runners::runners_package_json;
use crate::types::{Task, TaskDefinitionType};
use std::path::PathBuf;

/// Parse a package.json file at the given path and extract tasks
pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read package.json: {}", e))?;

    let json: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;

    let mut tasks = Vec::new();

    // Get the package manager to use
    let pkg_mgr = runners_package_json::detect_package_manager(
        path.parent().unwrap_or_else(|| path.as_ref()),
    )
    .ok_or_else(|| "No package manager found".to_string())?;

    if let Some(scripts) = json.get("scripts") {
        if let Some(scripts_obj) = scripts.as_object() {
            for (name, cmd) in scripts_obj {
                tasks.push(Task {
                    name: name.clone(),
                    file_path: path.clone(),
                    definition_type: TaskDefinitionType::PackageJson,
                    runner: pkg_mgr.clone(),
                    source_name: name.clone(),
                    description: cmd.as_str().map(|s| s.to_string()),
                    shadowed_by: None,
                });
            }
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
        assert!(matches!(
            test_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(test_task.description, Some("jest".to_string()));

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert!(matches!(
            build_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(build_task.description, Some("tsc".to_string()));
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
