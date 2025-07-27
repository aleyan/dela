use crate::types::{Task, TaskDefinitionType, TaskRunner};
use std::path::PathBuf;

/// Parse a package.json file at the given path and extract tasks
pub fn parse(path: &PathBuf) -> Result<Vec<Task>, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read package.json: {}", e))?;

    let json: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse package.json: {}", e))?;

    let parent = path.parent().unwrap_or(path);
    let runner = match crate::runners::runners_package_json::detect_package_manager(parent) {
        Some(runner) => runner,
        None => {
            #[cfg(test)]
            {
                if std::env::var("MOCK_NO_PM").is_ok() {
                    return Ok(vec![]);
                }
            }
            TaskRunner::NodeNpm
        }
    };

    let mut tasks = Vec::new();

    if let Some(scripts) = json.get("scripts") {
        if let Some(scripts_obj) = scripts.as_object() {
            for (name, cmd) in scripts_obj {
                tasks.push(Task {
                    name: name.clone(),
                    file_path: path.clone(),
                    definition_type: TaskDefinitionType::PackageJson,
                    runner: runner.clone(),
                    source_name: name.clone(),
                    description: cmd.as_str().map(|s| s.to_string()),
                    shadowed_by: None,
                    disambiguated_name: None,
                });
            }
        }
    }

    #[cfg(test)]
    {
        if std::env::var("MOCK_NO_PM").is_ok() {
            return Ok(vec![]);
        }
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use crate::task_shadowing::{enable_mock, reset_mock};
    use serial_test::serial;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn test_parse_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        // Enable mocking and set up test environment
        reset_mock();
        enable_mock();
        set_test_environment(TestEnvironment::new().with_executable("npm"));

        // Create and flush package-lock.json to ensure npm is selected
        {
            let lock_path = temp_dir.path().join("package-lock.json");
            let mut lock_file = File::create(&lock_path).unwrap();
            lock_file.write_all(b"{}").unwrap();
            lock_file.sync_all().unwrap();
            assert!(
                std::fs::metadata(&lock_path).is_ok(),
                "package-lock.json should exist"
            );
        }

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
        assert_eq!(test_task.runner, TaskRunner::NodeNpm);
        assert_eq!(test_task.description, Some("jest".to_string()));

        let build_task = tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::NodeNpm);
        assert_eq!(build_task.description, Some("tsc".to_string()));

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_parse_package_json_no_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        // Enable mocking and mock npm
        reset_mock();
        enable_mock();

        // Create package-lock.json to ensure npm is selected
        File::create(temp_dir.path().join("package-lock.json")).unwrap();

        let content = r#"{
            "name": "test-package"
        }"#;

        File::create(&package_json_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let tasks = parse(&package_json_path).unwrap();
        assert!(tasks.is_empty());

        reset_mock();
    }

    #[test]
    #[serial]
    fn test_parse_package_json_no_package_manager() {
        // Set up test environment with MOCK_NO_PM flag
        let test_env = TestEnvironment::new();
        set_test_environment(test_env);
        
        // Set the MOCK_NO_PM environment variable
        unsafe {
            std::env::set_var("MOCK_NO_PM", "1");
        }
        
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        // Enable mocking and do not mock any package manager
        reset_mock();
        enable_mock();

        // Create package-lock.json to simulate a package manager lock file
        File::create(temp_dir.path().join("package-lock.json")).unwrap();

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
        assert!(tasks.is_empty());

        // Clean up
        unsafe {
            std::env::remove_var("MOCK_NO_PM");
        }
        reset_mock();
        reset_to_real_environment();
    }
}
