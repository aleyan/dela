use std::path::Path;
use crate::types::{DiscoveredTasks, TaskFileStatus, TaskDefinitionFile, TaskRunner};

use crate::parse_makefile;
use crate::parse_package_json;
use crate::parse_pyproject_toml;

/// Discovers tasks in the given directory
pub fn discover_tasks(dir: &Path) -> DiscoveredTasks {
    let mut discovered = DiscoveredTasks::default();
    
    if let Err(e) = discover_makefile_tasks(dir, &mut discovered) {
        discovered.errors.push(format!("Error parsing Makefile: {}", e));
        if let Some(makefile) = &mut discovered.definitions.makefile {
            makefile.status = TaskFileStatus::ParseError(e);
        }
    }

    // TODO(DTKT-5): Implement package.json parser
    if let Err(e) = discover_npm_tasks(dir, &mut discovered) {
        discovered.errors.push(format!("Error parsing package.json: {}", e));
        if let Some(package_json) = &mut discovered.definitions.package_json {
            package_json.status = TaskFileStatus::ParseError(e);
        }
    }

    // TODO(DTKT-6): Implement pyproject.toml parser
    if let Err(e) = discover_python_tasks(dir, &mut discovered) {
        discovered.errors.push(format!("Error parsing pyproject.toml: {}", e));
        if let Some(pyproject) = &mut discovered.definitions.pyproject_toml {
            pyproject.status = TaskFileStatus::ParseError(e);
        }
    }

    discovered
}

fn discover_makefile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let makefile_path = dir.join("Makefile");
    
    if !makefile_path.exists() {
        discovered.definitions.makefile = Some(parse_makefile::create_definition(
            &makefile_path,
            TaskFileStatus::NotFound,
        ));
        return Ok(());
    }

    match parse_makefile::parse(&makefile_path) {
        Ok(tasks) => {
            discovered.definitions.makefile = Some(parse_makefile::create_definition(
                &makefile_path,
                TaskFileStatus::Parsed,
            ));
            discovered.tasks.extend(tasks);
            Ok(())
        }
        Err(e) => {
            discovered.definitions.makefile = Some(parse_makefile::create_definition(
                &makefile_path,
                TaskFileStatus::ParseError(e.clone()),
            ));
            // Don't add any tasks if parsing failed
            discovered.tasks.clear();
            Err(e)
        }
    }
}

fn discover_npm_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let package_json = dir.join("package.json");
    
    if !package_json.exists() {
        discovered.definitions.package_json = Some(TaskDefinitionFile {
            path: package_json,
            runner: TaskRunner::Npm,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    match parse_package_json::parse(&package_json) {
        Ok(tasks) => {
            discovered.definitions.package_json = Some(TaskDefinitionFile {
                path: package_json.clone(),
                runner: TaskRunner::Npm,
                status: TaskFileStatus::Parsed,
            });
            discovered.tasks.extend(tasks);
            Ok(())
        }
        Err(e) => {
            discovered.definitions.package_json = Some(TaskDefinitionFile {
                path: package_json,
                runner: TaskRunner::Npm,
                status: TaskFileStatus::ParseError(e.clone()),
            });
            Err(e)
        }
    }
}

fn discover_python_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let pyproject_toml = dir.join("pyproject.toml");
    
    if !pyproject_toml.exists() {
        discovered.definitions.pyproject_toml = Some(
            parse_pyproject_toml::create_definition(
                &pyproject_toml,
                TaskFileStatus::NotFound,
                TaskRunner::PythonUv
            )
        );
        return Ok(());
    }

    match parse_pyproject_toml::parse(&pyproject_toml) {
        Ok((tasks, runner)) => {
            discovered.definitions.pyproject_toml = Some(
                parse_pyproject_toml::create_definition(
                    &pyproject_toml,
                    TaskFileStatus::Parsed,
                    runner.clone()
                )
            );
            discovered.tasks.extend(tasks);
            Ok(())
        }
        Err(e) => {
            discovered.definitions.pyproject_toml = Some(
                parse_pyproject_toml::create_definition(
                    &pyproject_toml,
                    TaskFileStatus::ParseError(e.clone()),
                    TaskRunner::PythonUv
                )
            );
            Err(e)
        }
    }
}

// TODO(DTKT-52): Add trait for plugin-based task discovery 

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_makefile(dir: &Path, content: &str) {
        let mut file = File::create(dir.join("Makefile")).unwrap();
        writeln!(file, "{}", content).unwrap();
    }

    #[test]
    fn test_discover_tasks_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let discovered = discover_tasks(temp_dir.path());
        
        assert!(discovered.tasks.is_empty());
        assert!(discovered.errors.is_empty());
        
        // Check Makefile status
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::NotFound
        ));
        
        // Check package.json status
        assert!(matches!(
            discovered.definitions.package_json.unwrap().status,
            TaskFileStatus::NotFound
        ));
        
        // Check pyproject.toml status
        assert!(matches!(
            discovered.definitions.pyproject_toml.unwrap().status,
            TaskFileStatus::NotFound
        ));
    }

    #[test]
    fn test_discover_tasks_with_makefile() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#".PHONY: build test

build:
	@echo "Building the project"
	cargo build

test:
	@echo "Running tests"
	cargo test"#;
        create_test_makefile(temp_dir.path(), content);
        
        let discovered = discover_tasks(temp_dir.path());
        
        assert_eq!(discovered.tasks.len(), 2);
        assert!(discovered.errors.is_empty());
        
        // Check Makefile status
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        
        // Verify tasks
        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Make);
        assert_eq!(build_task.description, Some("Building the project".to_string()));
        
        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Make);
        assert_eq!(test_task.description, Some("Running tests".to_string()));
    }

    #[test]
    fn test_discover_tasks_with_invalid_makefile() {
        let temp_dir = TempDir::new().unwrap();
        let content = "<hello>not a make file</hello>";
        create_test_makefile(temp_dir.path(), content);
        
        let discovered = discover_tasks(temp_dir.path());
        
        // Because makefile_lossless doesn't throw an error for unrecognized lines,
        // we expect zero tasks without any parse error:
        assert!(discovered.tasks.is_empty(), "Expected no tasks, found: {:?}", discovered.tasks);
        assert!(discovered.errors.is_empty(), "Expected no errors, found some");
        
        // The status is considered Parsed (no recognized tasks, but no parse error):
        match &discovered.definitions.makefile.unwrap().status {
            TaskFileStatus::Parsed => (),
            status => panic!("Expected Parsed, got {:?}", status),
        }
    }

    #[test]
    fn test_discover_tasks_with_unimplemented_parsers() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create an invalid pyproject.toml to trigger a parse error
        let mut file = File::create(temp_dir.path().join("pyproject.toml")).unwrap();
        write!(file, "invalid toml content").unwrap();
        
        let discovered = discover_tasks(temp_dir.path());
        
        // Check pyproject.toml status - should be ParseError now that we've implemented it
        assert!(matches!(
            discovered.definitions.pyproject_toml.unwrap().status,
            TaskFileStatus::ParseError(_)
        ));
    }

    #[test]
    fn test_discover_npm_tasks() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create package.json with scripts
        let content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest",
                "build": "tsc"
            }
        }"#;
        
        let mut file = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(file, "{}", content).unwrap();
        
        let discovered = discover_tasks(temp_dir.path());
        
        // Check package.json status
        let package_json_def = discovered.definitions.package_json.unwrap();
        assert_eq!(package_json_def.status, TaskFileStatus::Parsed);
        
        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 2);
        
        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Npm);
        assert_eq!(test_task.description, Some("npm script: jest".to_string()));
        
        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Npm);
        assert_eq!(build_task.description, Some("npm script: tsc".to_string()));
    }

    #[test]
    fn test_discover_npm_tasks_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create invalid package.json
        let content = r#"{ invalid json }"#;
        let mut file = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(file, "{}", content).unwrap();
        
        let discovered = discover_tasks(temp_dir.path());
        
        // Check package.json status shows parse error
        let package_json_def = discovered.definitions.package_json.unwrap();
        assert!(matches!(package_json_def.status, TaskFileStatus::ParseError(_)));
        
        // Verify no tasks were discovered
        assert!(discovered.tasks.is_empty());
    }

    #[test]
    fn test_discover_python_tasks() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create pyproject.toml with UV scripts
        let content = r#"
[project]
name = "test-project"

[project.scripts]
serve = "uvicorn main:app --reload"
"#;
        
        let pyproject_path = temp_dir.path().join("pyproject.toml");
        let mut file = File::create(&pyproject_path).unwrap();
        write!(file, "{}", content).unwrap();
        
        let discovered = discover_tasks(temp_dir.path());
        
        // Check pyproject.toml status
        let pyproject_def = discovered.definitions.pyproject_toml.unwrap();
        assert_eq!(pyproject_def.status, TaskFileStatus::Parsed);
        
        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 1);
        
        let serve_task = discovered.tasks.iter().find(|t| t.name == "serve").unwrap();
        assert_eq!(serve_task.runner, TaskRunner::PythonUv);
        assert_eq!(serve_task.description, Some("python script: uvicorn main:app --reload".to_string()));
    }
} 