use crate::parsers::{parse_makefile, parse_package_json, parse_pyproject_toml};
use crate::task_shadowing;
use crate::types::{
    DiscoveredTaskDefinitions, Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus,
    TaskRunner,
};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml;
use walkdir::WalkDir;

/// Result of task discovery
#[derive(Debug, Default)]
pub struct DiscoveredTasks {
    /// Task definition files found
    pub definitions: DiscoveredTaskDefinitions,
    /// Tasks found
    pub tasks: Vec<Task>,
    /// Errors encountered during discovery
    pub errors: Vec<String>,
}

/// Discover tasks in a directory
pub fn discover_tasks(dir: &Path) -> DiscoveredTasks {
    let mut discovered = DiscoveredTasks::default();

    // Discover tasks from each type of definition file
    let _ = discover_makefile_tasks(dir, &mut discovered);
    let _ = discover_npm_tasks(dir, &mut discovered);
    let _ = discover_python_tasks(dir, &mut discovered);
    discover_shell_script_tasks(dir, &mut discovered);

    discovered
}

fn discover_makefile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let makefile_path = dir.join("Makefile");

    if !makefile_path.exists() {
        discovered.definitions.makefile = Some(TaskDefinitionFile {
            path: makefile_path.clone(),
            definition_type: TaskDefinitionType::Makefile,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    match parse_makefile::parse(&makefile_path) {
        Ok(mut tasks) => {
            discovered.definitions.makefile = Some(TaskDefinitionFile {
                path: makefile_path.clone(),
                definition_type: TaskDefinitionType::Makefile,
                status: TaskFileStatus::Parsed,
            });
            // Add shadow information
            for task in &mut tasks {
                task.shadowed_by = task_shadowing::check_shadowing(&task.name);
            }
            discovered.tasks.extend(tasks);
        }
        Err(e) => {
            discovered.definitions.makefile = Some(TaskDefinitionFile {
                path: makefile_path,
                definition_type: TaskDefinitionType::Makefile,
                status: TaskFileStatus::ParseError(e.clone()),
            });
            discovered
                .errors
                .push(format!("Failed to parse Makefile: {}", e));
        }
    }

    Ok(())
}

fn discover_npm_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let package_json = dir.join("package.json");

    if !package_json.exists() {
        discovered.definitions.package_json = Some(TaskDefinitionFile {
            path: package_json.clone(),
            definition_type: TaskDefinitionType::PackageJson,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    match parse_package_json::parse(&package_json) {
        Ok(mut tasks) => {
            discovered.definitions.package_json = Some(TaskDefinitionFile {
                path: package_json.clone(),
                definition_type: TaskDefinitionType::PackageJson,
                status: TaskFileStatus::Parsed,
            });
            // Add shadow information
            for task in &mut tasks {
                task.shadowed_by = task_shadowing::check_shadowing(&task.name);
            }
            discovered.tasks.extend(tasks);
        }
        Err(e) => {
            discovered.definitions.package_json = Some(TaskDefinitionFile {
                path: package_json,
                definition_type: TaskDefinitionType::PackageJson,
                status: TaskFileStatus::ParseError(e.clone()),
            });
            discovered
                .errors
                .push(format!("Failed to parse package.json: {}", e));
        }
    }

    Ok(())
}

fn discover_python_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let pyproject_toml = dir.join("pyproject.toml");

    if !pyproject_toml.exists() {
        discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
            path: pyproject_toml.clone(),
            definition_type: TaskDefinitionType::PyprojectToml,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    match parse_pyproject_toml::parse(&pyproject_toml) {
        Ok(mut tasks) => {
            discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
                path: pyproject_toml.clone(),
                definition_type: TaskDefinitionType::PyprojectToml,
                status: TaskFileStatus::Parsed,
            });
            // Add shadow information
            for task in &mut tasks {
                task.shadowed_by = task_shadowing::check_shadowing(&task.name);
            }
            discovered.tasks.extend(tasks);
        }
        Err(e) => {
            discovered.definitions.pyproject_toml = Some(TaskDefinitionFile {
                path: pyproject_toml,
                definition_type: TaskDefinitionType::PyprojectToml,
                status: TaskFileStatus::ParseError(e.clone()),
            });
            discovered
                .errors
                .push(format!("Failed to parse pyproject.toml: {}", e));
        }
    }

    Ok(())
}

fn discover_shell_script_tasks(dir: &Path, discovered: &mut DiscoveredTasks) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "sh" {
                        let name = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        discovered.tasks.push(Task {
                            name: name.clone(),
                            file_path: path,
                            definition_type: TaskDefinitionType::ShellScript,
                            runner: TaskRunner::ShellScript,
                            source_name: name.clone(),
                            description: None,
                            shadowed_by: task_shadowing::check_shadowing(&name),
                        });
                    }
                }
            }
        }
    }
}

// TODO(DTKT-52): Add trait for plugin-based task discovery

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
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
        assert!(matches!(
            test_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(test_task.description, Some("jest".to_string()));

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert!(matches!(
            build_task.runner,
            TaskRunner::NodeNpm | TaskRunner::NodeYarn | TaskRunner::NodePnpm | TaskRunner::NodeBun
        ));
        assert_eq!(build_task.description, Some("tsc".to_string()));
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
            .filter(|t| matches!(t.runner, TaskRunner::Make))
            .collect();
        assert_eq!(make_tasks.len(), 2);

        let node_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.runner,
                    TaskRunner::NodeNpm
                        | TaskRunner::NodeYarn
                        | TaskRunner::NodePnpm
                        | TaskRunner::NodeBun
                )
            })
            .collect();
        assert_eq!(node_tasks.len(), 2);

        let python_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| matches!(t.runner, TaskRunner::PythonUv))
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
            .find(|t| matches!(t.runner, TaskRunner::Make) && t.name == "test")
            .unwrap();
        assert_eq!(
            make_test.description,
            Some("Running make tests".to_string())
        );

        let node_test = discovered
            .tasks
            .iter()
            .find(|t| {
                matches!(
                    t.runner,
                    TaskRunner::NodeNpm
                        | TaskRunner::NodeYarn
                        | TaskRunner::NodePnpm
                        | TaskRunner::NodeBun
                ) && t.name == "test"
            })
            .unwrap();
        assert_eq!(node_test.description, Some("jest".to_string()));
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

    #[test]
    fn test_discover_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create test files
        let makefile_content = "test:\n\t@echo Testing\n";
        let mut makefile = File::create(project_dir.join("Makefile")).unwrap();
        makefile.write_all(makefile_content.as_bytes()).unwrap();

        let package_json_content = r#"{
            "scripts": {
                "build": "echo Building",
                "test": "echo Testing"
            }
        }"#;
        let mut package_json = File::create(project_dir.join("package.json")).unwrap();
        package_json
            .write_all(package_json_content.as_bytes())
            .unwrap();

        let pyproject_toml_content = r#"
[tool.poetry.scripts]
serve = "echo Serving"
"#;
        let mut pyproject_toml = File::create(project_dir.join("pyproject.toml")).unwrap();
        pyproject_toml
            .write_all(pyproject_toml_content.as_bytes())
            .unwrap();

        let tasks = discover_tasks(project_dir);

        // Verify tasks were discovered correctly
        assert!(tasks
            .tasks
            .iter()
            .any(|t| t.name == "test" && matches!(t.runner, TaskRunner::Make)));
        assert!(tasks
            .tasks
            .iter()
            .any(|t| t.name == "build" && matches!(t.runner, TaskRunner::NodeNpm)));
        assert!(tasks
            .tasks
            .iter()
            .any(|t| t.name == "test" && matches!(t.runner, TaskRunner::NodeNpm)));
        assert!(tasks
            .tasks
            .iter()
            .any(|t| t.name == "serve" && matches!(t.runner, TaskRunner::PythonPoetry)));
    }

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

        let tasks = parse_package_json::parse(&package_json_path).unwrap();

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
}
