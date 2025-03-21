use crate::parsers::{parse_makefile, parse_package_json, parse_pom_xml, parse_pyproject_toml, parse_taskfile};
use crate::task_shadowing::check_shadowing;
use crate::types::{
    DiscoveredTaskDefinitions, Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus,
    TaskRunner,
};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

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
    let _ = discover_taskfile_tasks(dir, &mut discovered);
    let _ = discover_maven_tasks(dir, &mut discovered);
    discover_shell_script_tasks(dir, &mut discovered);

    discovered
}

/// Helper function to set task definition based on type
fn set_definition(discovered: &mut DiscoveredTasks, definition: TaskDefinitionFile) {
    match definition.definition_type {
        TaskDefinitionType::Makefile => discovered.definitions.makefile = Some(definition),
        TaskDefinitionType::PackageJson => discovered.definitions.package_json = Some(definition),
        TaskDefinitionType::PyprojectToml => {
            discovered.definitions.pyproject_toml = Some(definition)
        }
        TaskDefinitionType::Taskfile => discovered.definitions.taskfile = Some(definition),
        TaskDefinitionType::MavenPom => discovered.definitions.maven_pom = Some(definition),
        _ => {}
    }
}

/// Helper function to handle task file discovery errors
fn handle_discovery_error(
    error: String,
    file_path: PathBuf,
    definition_type: TaskDefinitionType,
    discovered: &mut DiscoveredTasks,
) {
    discovered.errors.push(format!(
        "Failed to parse {}: {}",
        file_path.display(),
        error
    ));
    let definition = TaskDefinitionFile {
        path: file_path,
        definition_type,
        status: TaskFileStatus::ParseError(error),
    };
    set_definition(discovered, definition);
}

/// Helper function to handle successful task discovery
fn handle_discovery_success(
    mut tasks: Vec<Task>,
    file_path: PathBuf,
    definition_type: TaskDefinitionType,
    discovered: &mut DiscoveredTasks,
) {
    // Add shadow information
    for task in &mut tasks {
        task.shadowed_by = check_shadowing(&task.name);
    }
    let definition = TaskDefinitionFile {
        path: file_path,
        definition_type,
        status: TaskFileStatus::Parsed,
    };
    set_definition(discovered, definition);
    discovered.tasks.extend(tasks);
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
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                makefile_path,
                TaskDefinitionType::Makefile,
                discovered,
            );
        }
        Err(e) => {
            handle_discovery_error(e, makefile_path, TaskDefinitionType::Makefile, discovered);
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
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                package_json,
                TaskDefinitionType::PackageJson,
                discovered,
            );
        }
        Err(e) => {
            handle_discovery_error(e, package_json, TaskDefinitionType::PackageJson, discovered);
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
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                pyproject_toml,
                TaskDefinitionType::PyprojectToml,
                discovered,
            );
        }
        Err(e) => {
            handle_discovery_error(
                e,
                pyproject_toml,
                TaskDefinitionType::PyprojectToml,
                discovered,
            );
        }
    }

    Ok(())
}

fn discover_taskfile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let taskfile_path = dir.join("Taskfile.yml");
    if taskfile_path.exists() {
        let mut definition = TaskDefinitionFile {
            path: taskfile_path.clone(),
            definition_type: TaskDefinitionType::Taskfile,
            status: TaskFileStatus::NotImplemented,
        };

        match parse_taskfile::parse(&taskfile_path) {
            Ok(tasks) => {
                definition.status = TaskFileStatus::Parsed;
                discovered.tasks.extend(tasks);
            }
            Err(e) => {
                definition.status = TaskFileStatus::ParseError(e.clone());
                discovered
                    .errors
                    .push(format!("Error parsing Taskfile.yml: {}", e));
            }
        }

        set_definition(discovered, definition);
    }
    Ok(())
}

fn discover_maven_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let pom_path = dir.join("pom.xml");
    if !pom_path.exists() {
        return Ok(());
    }

    match parse_pom_xml(&pom_path) {
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                pom_path.clone(),
                TaskDefinitionType::MavenPom,
                discovered,
            );
            Ok(())
        }
        Err(e) => {
            handle_discovery_error(e, pom_path, TaskDefinitionType::MavenPom, discovered);
            Err("Error parsing pom.xml".to_string())
        }
    }
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
                            shadowed_by: check_shadowing(&name),
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
    use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
    use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
    use crate::types::ShadowType;
    use serial_test::serial;
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

        // The status should be ParseError, as the makefile contains invalid content:
        assert!(matches!(
            discovered.definitions.makefile.unwrap().status,
            TaskFileStatus::ParseError(_)
        ));
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

        // Mock UV being installed
        reset_mock();
        enable_mock();
        mock_executable("uv");

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

        reset_mock();
    }

    #[test]
    fn test_discover_python_poetry_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Mock Poetry being installed
        reset_mock();
        enable_mock();
        mock_executable("poetry");

        // Create poetry.lock to ensure Poetry is selected
        File::create(temp_dir.path().join("poetry.lock")).unwrap();

        // Create pyproject.toml with Poetry scripts
        let content = r#"
[tool.poetry]
name = "test-project"

[tool.poetry.scripts]
serve = "python -m http.server"
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
            Some("python script: python -m http.server".to_string())
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

        reset_mock();
    }

    #[test]
    fn test_discover_tasks_multiple_files() {
        let temp_dir = TempDir::new().unwrap();

        // Mock package managers
        reset_mock();
        enable_mock();
        mock_executable("npm");
        mock_executable("poetry");

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

        // Create pyproject.toml with Poetry scripts
        let pyproject_content = r#"
[tool.poetry]
name = "test-project"

[tool.poetry.scripts]
serve = "python -m http.server"
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
            .filter(|t| matches!(t.runner, TaskRunner::PythonPoetry))
            .collect();
        assert_eq!(python_tasks.len(), 1);

        reset_mock();
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
    #[serial]
    fn test_discover_tasks_with_shadowing() {
        let temp_dir = TempDir::new().unwrap();
        let makefile_path = temp_dir.path().join("Makefile");

        // Set up test environment with zsh shell
        let env = TestEnvironment::new().with_shell("/bin/zsh");
        set_test_environment(env);

        let content = ".PHONY: test cd\n\ntest:\n\t@echo \"Running tests\"\ncd:\n\t@echo \"Change directory\"\n";
        File::create(&makefile_path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Verify that the cd task is marked as shadowed
        let cd_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "cd")
            .expect("cd task not found");

        assert!(matches!(
            cd_task.shadowed_by,
            Some(ShadowType::ShellBuiltin(_))
        ));

        reset_to_real_environment();
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

    #[test]
    fn test_discover_taskfile_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Mock task being installed
        reset_mock();
        enable_mock();
        mock_executable("task");

        // Create Taskfile.yml with tasks
        let content = r#"version: '3'

tasks:
  test:
    desc: Test task
    cmds:
      - echo "Running tests"
  build:
    desc: Build task
    cmds:
      - echo "Building project"
  deps:
    desc: Task with dependencies
    deps:
      - test
    cmds:
      - echo "Running dependent task""#;

        let taskfile_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_path).unwrap();
        write!(file, "{}", content).unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Check Taskfile.yml status
        let taskfile_def = discovered.definitions.taskfile.unwrap();
        assert_eq!(taskfile_def.status, TaskFileStatus::Parsed);

        // Verify tasks were discovered
        assert_eq!(discovered.tasks.len(), 3);

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.runner, TaskRunner::Task);
        assert_eq!(test_task.description, Some("Test task".to_string()));

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.runner, TaskRunner::Task);
        assert_eq!(build_task.description, Some("Build task".to_string()));

        let deps_task = discovered.tasks.iter().find(|t| t.name == "deps").unwrap();
        assert_eq!(deps_task.runner, TaskRunner::Task);
        assert_eq!(
            deps_task.description,
            Some("Task with dependencies".to_string())
        );

        reset_mock();
    }

    #[test]
    fn test_discover_maven_tasks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();
        
        // Create a sample pom.xml
        let pom_xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    
    <groupId>com.example</groupId>
    <artifactId>sample-project</artifactId>
    <version>1.0-SNAPSHOT</version>
    
    <properties>
        <maven.compiler.source>17</maven.compiler.source>
        <maven.compiler.target>17</maven.compiler.target>
    </properties>
    
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.10.1</version>
                <executions>
                    <execution>
                        <id>compile-java</id>
                        <goals>
                            <goal>compile</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>
            <plugin>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-maven-plugin</artifactId>
                <version>2.7.0</version>
                <executions>
                    <execution>
                        <id>build-info</id>
                        <goals>
                            <goal>build-info</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>
        </plugins>
    </build>
    
    <profiles>
        <profile>
            <id>dev</id>
            <properties>
                <spring.profiles.active>dev</spring.profiles.active>
            </properties>
        </profile>
        <profile>
            <id>prod</id>
            <properties>
                <spring.profiles.active>prod</spring.profiles.active>
            </properties>
        </profile>
    </profiles>
</project>"#;
        
        std::fs::write(dir_path.join("pom.xml"), pom_xml_content).unwrap();
        
        let discovered = discover_tasks(dir_path);
        
        // Check that the definition was found
        assert!(discovered.definitions.maven_pom.is_some());
        assert_eq!(
            discovered.definitions.maven_pom.unwrap().status,
            TaskFileStatus::Parsed
        );
        
        // Check that default Maven lifecycle tasks are discovered
        assert!(discovered.tasks.iter().any(|t| t.name == "clean"));
        assert!(discovered.tasks.iter().any(|t| t.name == "compile"));
        assert!(discovered.tasks.iter().any(|t| t.name == "test"));
        assert!(discovered.tasks.iter().any(|t| t.name == "package"));
        assert!(discovered.tasks.iter().any(|t| t.name == "install"));
        
        // Check that profile tasks are discovered
        assert!(discovered.tasks.iter().any(|t| t.name == "profile:dev"));
        assert!(discovered.tasks.iter().any(|t| t.name == "profile:prod"));
        
        // Check that plugin goals are discovered
        assert!(discovered.tasks.iter().any(|t| t.name == "maven-compiler-plugin:compile"));
        assert!(discovered.tasks.iter().any(|t| t.name == "spring-boot-maven-plugin:build-info"));
        
        // Verify task runners
        for task in discovered.tasks {
            if task.definition_type == TaskDefinitionType::MavenPom {
                assert_eq!(task.runner, TaskRunner::Maven);
            }
        }
    }
}
