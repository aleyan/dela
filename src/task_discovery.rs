use crate::parsers::{parse_makefile, parse_package_json, parse_pyproject_toml};
use crate::task_shadowing;
use crate::types::{DiscoveredTasks, TaskFileStatus, TaskRunner};
use std::path::Path;

/// Discovers tasks in the given directory
pub fn discover_tasks(dir: &Path) -> DiscoveredTasks {
    let mut discovered = DiscoveredTasks::default();

    if let Err(e) = discover_makefile_tasks(dir, &mut discovered) {
        discovered
            .errors
            .push(format!("Error parsing Makefile: {}", e));
        if let Some(makefile) = &mut discovered.definitions.makefile {
            makefile.status = TaskFileStatus::ParseError(e);
        }
    }

    // TODO(DTKT-5): Implement package.json parser
    if let Err(e) = discover_npm_tasks(dir, &mut discovered) {
        discovered
            .errors
            .push(format!("Error parsing package.json: {}", e));
        if let Some(package_json) = &mut discovered.definitions.package_json {
            package_json.status = TaskFileStatus::ParseError(e);
        }
    }

    // TODO(DTKT-6): Implement pyproject.toml parser
    if let Err(e) = discover_python_tasks(dir, &mut discovered) {
        discovered
            .errors
            .push(format!("Error parsing pyproject.toml: {}", e));
        if let Some(pyproject) = &mut discovered.definitions.pyproject_toml {
            pyproject.status = TaskFileStatus::ParseError(e);
        }
    }

    // Check for shadowing after discovering all tasks
    for task in &mut discovered.tasks {
        if let Some(shadow_type) = task_shadowing::check_shadowing(&task.name) {
            task.shadowed_by = Some(shadow_type);
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
        discovered.definitions.package_json = Some(parse_package_json::create_definition(
            &package_json,
            TaskFileStatus::NotFound,
        ));
        return Ok(());
    }

    match parse_package_json::parse(&package_json) {
        Ok(tasks) => {
            discovered.definitions.package_json = Some(parse_package_json::create_definition(
                &package_json,
                TaskFileStatus::Parsed,
            ));
            discovered.tasks.extend(tasks);
            Ok(())
        }
        Err(e) => {
            discovered.definitions.package_json = Some(parse_package_json::create_definition(
                &package_json,
                TaskFileStatus::ParseError(e.clone()),
            ));
            Err(e)
        }
    }
}

fn discover_python_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let pyproject_toml = dir.join("pyproject.toml");

    if !pyproject_toml.exists() {
        discovered.definitions.pyproject_toml = Some(parse_pyproject_toml::create_definition(
            &pyproject_toml,
            TaskFileStatus::NotFound,
            TaskRunner::PythonUv,
        ));
        return Ok(());
    }

    match parse_pyproject_toml::parse(&pyproject_toml) {
        Ok((tasks, runner)) => {
            discovered.definitions.pyproject_toml = Some(parse_pyproject_toml::create_definition(
                &pyproject_toml,
                TaskFileStatus::Parsed,
                runner.clone(),
            ));
            discovered.tasks.extend(tasks);
            Ok(())
        }
        Err(e) => {
            discovered.definitions.pyproject_toml = Some(parse_pyproject_toml::create_definition(
                &pyproject_toml,
                TaskFileStatus::ParseError(e.clone()),
                TaskRunner::PythonUv,
            ));
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
        assert_eq!(
            build_task.description,
            Some("Building the project".to_string())
        );

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
        assert!(
            discovered.tasks.is_empty(),
            "Expected no tasks, found: {:?}",
            discovered.tasks
        );
        assert!(
            discovered.errors.is_empty(),
            "Expected no errors, found some"
        );

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
        assert!(matches!(
            package_json_def.status,
            TaskFileStatus::ParseError(_)
        ));

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
        assert_eq!(
            serve_task.description,
            Some("python script: uvicorn main:app --reload".to_string())
        );
    }

    #[test]
    fn test_discover_tasks_multiple_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create Makefile
        let makefile_content = r#".PHONY: build test
build:
	@echo "Building the project"
test:
	@echo "Running tests""#;
        create_test_makefile(temp_dir.path(), makefile_content);

        // Create package.json
        let package_json_content = r#"{
            "name": "test-package",
            "scripts": {
                "start": "node index.js",
                "lint": "eslint ."
            }
        }"#;
        let mut package_json = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(package_json, "{}", package_json_content).unwrap();

        // Create pyproject.toml
        let pyproject_content = r#"
[project]
name = "test-project"

[project.scripts]
serve = "uvicorn main:app --reload"
"#;
        let mut pyproject = File::create(temp_dir.path().join("pyproject.toml")).unwrap();
        write!(pyproject, "{}", pyproject_content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Verify all task files were parsed
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert!(matches!(
            discovered.definitions.package_json.unwrap().status,
            TaskFileStatus::Parsed
        ));
        assert!(matches!(
            discovered.definitions.pyproject_toml.unwrap().status,
            TaskFileStatus::Parsed
        ));

        // Verify all tasks were discovered
        assert_eq!(discovered.tasks.len(), 5);

        // Verify tasks from each file
        let make_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.runner == TaskRunner::Make)
            .collect();
        assert_eq!(make_tasks.len(), 2);

        let npm_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.runner == TaskRunner::Npm)
            .collect();
        assert_eq!(npm_tasks.len(), 2);

        let python_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.runner == TaskRunner::PythonUv)
            .collect();
        assert_eq!(python_tasks.len(), 1);
    }

    #[test]
    fn test_discover_tasks_with_name_collision() {
        let temp_dir = TempDir::new().unwrap();

        // Create Makefile with 'test' task
        let makefile_content = r#".PHONY: test
test:
	@echo "Running make tests""#;
        create_test_makefile(temp_dir.path(), makefile_content);

        // Create package.json with 'test' task
        let package_json_content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest"
            }
        }"#;
        let mut package_json = File::create(temp_dir.path().join("package.json")).unwrap();
        write!(package_json, "{}", package_json_content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Both tasks should be discovered
        assert_eq!(discovered.tasks.len(), 2);

        // Verify both test tasks exist with different runners
        let make_test = discovered
            .tasks
            .iter()
            .find(|t| t.runner == TaskRunner::Make && t.name == "test")
            .unwrap();
        assert_eq!(
            make_test.description,
            Some("Running make tests".to_string())
        );

        let npm_test = discovered
            .tasks
            .iter()
            .find(|t| t.runner == TaskRunner::Npm && t.name == "test")
            .unwrap();
        assert_eq!(npm_test.description, Some("npm script: jest".to_string()));
    }

    #[test]
    fn test_discover_tasks_with_shadowing() {
        let temp_dir = TempDir::new().unwrap();

        // Create Makefile with tasks that shadow shell builtins
        let makefile_content = r#".PHONY: cd ls echo
cd:
	@echo "Change directory"
ls:
	@echo "List files"
echo:
	@echo "Echo text""#;
        create_test_makefile(temp_dir.path(), makefile_content);

        let discovered = discover_tasks(temp_dir.path());

        // All tasks should be discovered
        assert_eq!(discovered.tasks.len(), 3);

        // Verify shadowing information
        for task in &discovered.tasks {
            assert!(
                task.shadowed_by.is_some(),
                "Task {} should be shadowed",
                task.name
            );
            assert!(matches!(
                task.shadowed_by.as_ref().unwrap(),
                task_shadowing::ShadowType::ShellBuiltin(_)
            ));
        }
    }

    #[test]
    fn test_discover_python_poetry_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Create pyproject.toml with Poetry scripts
        let content = r#"
[tool.poetry]
name = "test-project"

[tool.poetry.scripts]
serve = "uvicorn main:app --reload"
test = "pytest"
lint = "flake8"
"#;

        let pyproject_path = temp_dir.path().join("pyproject.toml");
        let mut file = File::create(&pyproject_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check pyproject.toml status
        let pyproject_def = discovered.definitions.pyproject_toml.unwrap();
        assert_eq!(pyproject_def.status, TaskFileStatus::Parsed);

        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 3);

        // Verify all tasks use PythonPoetry runner
        for task in &discovered.tasks {
            assert_eq!(task.runner, TaskRunner::PythonPoetry);
        }

        // Verify specific tasks
        let serve_task = discovered.tasks.iter().find(|t| t.name == "serve").unwrap();
        assert_eq!(
            serve_task.description,
            Some("python script: uvicorn main:app --reload".to_string())
        );

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(
            test_task.description,
            Some("python script: pytest".to_string())
        );

        let lint_task = discovered.tasks.iter().find(|t| t.name == "lint").unwrap();
        assert_eq!(
            lint_task.description,
            Some("python script: flake8".to_string())
        );
    }
}
