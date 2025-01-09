use std::path::Path;
use crate::types::{DiscoveredTasks, TaskFileStatus, TaskDefinitionFile, TaskRunner};

use crate::parse_makefile;

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

    discovered.definitions.package_json = Some(TaskDefinitionFile {
        path: package_json,
        runner: TaskRunner::Npm,
        status: TaskFileStatus::NotImplemented,
    });

    // TODO(DTKT-5): Implement package.json parser
    Ok(())
}

fn discover_python_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let pyproject_toml = dir.join("pyproject.toml");
    
    if !pyproject_toml.exists() {
        discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
            path: pyproject_toml,
            runner: TaskRunner::Python,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
        path: pyproject_toml,
        runner: TaskRunner::Python,
        status: TaskFileStatus::NotImplemented,
    });

    // TODO(DTKT-6): Implement pyproject.toml parser
    Ok(())
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
        let content = "invalid:\n\tthis is not a valid makefile\n\tno proper targets or rules\n\tjust random text";
        create_test_makefile(temp_dir.path(), content);
        
        let discovered = discover_tasks(temp_dir.path());
        
        assert!(discovered.tasks.is_empty(), "Expected no tasks, found: {:?}", discovered.tasks);
        assert!(!discovered.errors.is_empty(), "Expected errors, found none");
        
        // Check Makefile status
        match &discovered.definitions.makefile.unwrap().status {
            TaskFileStatus::ParseError(_) => (),
            status => panic!("Expected ParseError, got {:?}", status),
        }
    }

    #[test]
    fn test_discover_tasks_with_unimplemented_parsers() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create package.json
        let mut file = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(file, r#"{{ "scripts": {{ "test": "jest" }} }}"#).unwrap();
        
        // Create pyproject.toml
        let mut file = File::create(temp_dir.path().join("pyproject.toml")).unwrap();
        write!(file, r#"[tool.poetry.scripts]
test = "pytest""#).unwrap();
        
        let discovered = discover_tasks(temp_dir.path());
        
        // Check package.json status
        assert!(matches!(
            discovered.definitions.package_json.unwrap().status,
            TaskFileStatus::NotImplemented
        ));
        
        // Check pyproject.toml status
        assert!(matches!(
            discovered.definitions.pyproject_toml.unwrap().status,
            TaskFileStatus::NotImplemented
        ));
    }
} 