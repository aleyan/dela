use crate::parsers::{
    parse_github_actions, parse_gradle, parse_makefile, parse_package_json, parse_pom_xml,
    parse_pyproject_toml, parse_taskfile,
};
use crate::task_shadowing::check_shadowing;
use crate::types::{Task, TaskDefinitionFile, TaskDefinitionType, TaskFileStatus, TaskRunner};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

// Define the DiscoveredTaskDefinitions type directly here
#[derive(Debug, Default)]
pub struct DiscoveredTaskDefinitions {
    pub makefile: Option<TaskDefinitionFile>,
    pub package_json: Option<TaskDefinitionFile>,
    pub pyproject_toml: Option<TaskDefinitionFile>,
    pub taskfile: Option<TaskDefinitionFile>,
    pub maven_pom: Option<TaskDefinitionFile>,
    pub gradle: Option<TaskDefinitionFile>,
    pub github_actions: Option<TaskDefinitionFile>,
}

/// Result of task discovery
#[derive(Debug, Default)]
pub struct DiscoveredTasks {
    /// Task definition files found
    pub definitions: DiscoveredTaskDefinitions,
    /// Tasks found
    pub tasks: Vec<Task>,
    /// Errors encountered during discovery
    pub errors: Vec<String>,
    /// Map of task names to the number of occurrences (for disambiguation)
    pub task_name_counts: HashMap<String, usize>,
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
    let _ = discover_gradle_tasks(dir, &mut discovered);
    let _ = discover_github_actions_tasks(dir, &mut discovered);
    discover_shell_script_tasks(dir, &mut discovered);

    // Process tasks to identify name collisions
    process_task_disambiguation(&mut discovered);

    discovered
}

/// Processes tasks to identify name collisions and populate disambiguated_name fields
pub fn process_task_disambiguation(discovered: &mut DiscoveredTasks) {
    // Step 1: Identify tasks with name collisions
    let mut task_name_counts: HashMap<String, usize> = HashMap::new();
    let mut tasks_by_name: HashMap<String, Vec<usize>> = HashMap::new();

    // Count occurrences of each task name
    for (i, task) in discovered.tasks.iter().enumerate() {
        *task_name_counts.entry(task.name.clone()).or_insert(0) += 1;
        tasks_by_name
            .entry(task.name.clone())
            .or_insert_with(Vec::new)
            .push(i);
    }

    // Save task name counts for reference
    discovered.task_name_counts = task_name_counts.clone();

    // Step 2: Add disambiguated names to tasks with name collisions
    for (name, count) in task_name_counts.iter() {
        if *count > 1 {
            // This task name has collisions
            let task_indices = tasks_by_name.get(name).unwrap();

            // Track which runner prefix suffixes we've used for this task name
            let mut used_prefixes = std::collections::HashSet::new();

            for &idx in task_indices {
                let task = &mut discovered.tasks[idx];
                let runner_prefix = generate_runner_prefix(&task.runner, &used_prefixes);
                used_prefixes.insert(runner_prefix.clone());

                // Add a disambiguated name
                task.disambiguated_name = Some(format!("{}-{}", task.name, runner_prefix));
            }
        }
    }
}

/// Generates a unique prefix for a task runner for disambiguation
fn generate_runner_prefix(
    runner: &TaskRunner,
    used_prefixes: &std::collections::HashSet<String>,
) -> String {
    let short_name = runner.short_name();

    // Try just the first letter first
    let mut prefix = short_name[0..1].to_string();
    if !used_prefixes.contains(&prefix) {
        return prefix;
    }

    // If that's taken, try adding more letters until we have a unique prefix
    for i in 2..=short_name.len() {
        prefix = short_name[0..i].to_string();
        if !used_prefixes.contains(&prefix) {
            return prefix;
        }
    }

    // If we somehow get here, we'll make it unique by adding a number
    let mut i = 1;
    loop {
        let numbered_prefix = format!("{}{}", short_name, i);
        if !used_prefixes.contains(&numbered_prefix) {
            return numbered_prefix;
        }
        i += 1;
    }
}

/// Checks if a task name is ambiguous (has multiple implementations)
pub fn is_task_ambiguous(discovered: &DiscoveredTasks, task_name: &str) -> bool {
    discovered
        .task_name_counts
        .get(task_name)
        .map_or(false, |&count| count > 1)
}

/// Returns all tasks matching a given name (both original and disambiguated)
pub fn get_matching_tasks<'a>(discovered: &'a DiscoveredTasks, task_name: &str) -> Vec<&'a Task> {
    let mut result = Vec::new();

    // Check if this matches a disambiguated name
    if let Some(task) = discovered.tasks.iter().find(|t| {
        t.disambiguated_name
            .as_ref()
            .map_or(false, |dn| dn == task_name)
    }) {
        result.push(task);
        return result;
    }

    // Otherwise, find all tasks with this original name
    result.extend(discovered.tasks.iter().filter(|t| t.name == task_name));
    result
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
        TaskDefinitionType::Gradle => discovered.definitions.gradle = Some(definition),
        TaskDefinitionType::GitHubActions => {
            discovered.definitions.github_actions = Some(definition)
        }
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

/// Discover Gradle tasks from build.gradle or build.gradle.kts
fn discover_gradle_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    // Check for build.gradle first
    let build_gradle_path = dir.join("build.gradle");
    if build_gradle_path.exists() {
        match parse_gradle::parse(&build_gradle_path) {
            Ok(tasks) => {
                handle_discovery_success(
                    tasks,
                    build_gradle_path.clone(),
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                return Ok(());
            }
            Err(e) => {
                handle_discovery_error(
                    e,
                    build_gradle_path,
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                return Err("Error parsing build.gradle".to_string());
            }
        }
    }

    // If no build.gradle, try build.gradle.kts
    let build_gradle_kts_path = dir.join("build.gradle.kts");
    if build_gradle_kts_path.exists() {
        match parse_gradle::parse(&build_gradle_kts_path) {
            Ok(tasks) => {
                handle_discovery_success(
                    tasks,
                    build_gradle_kts_path.clone(),
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                Ok(())
            }
            Err(e) => {
                handle_discovery_error(
                    e,
                    build_gradle_kts_path,
                    TaskDefinitionType::Gradle,
                    discovered,
                );
                Err("Error parsing build.gradle.kts".to_string())
            }
        }
    } else {
        // No Gradle files found
        discovered.definitions.gradle = Some(TaskDefinitionFile {
            path: build_gradle_path,
            definition_type: TaskDefinitionType::Gradle,
            status: TaskFileStatus::NotFound,
        });
        Ok(())
    }
}

fn discover_github_actions_tasks(
    dir: &Path,
    discovered: &mut DiscoveredTasks,
) -> Result<(), String> {
    let mut workflow_files = Vec::new();

    // 1. Check .github/workflows/ (standard location)
    let workflows_dir = dir.join(".github").join("workflows");
    if workflows_dir.exists() && workflows_dir.is_dir() {
        match fs::read_dir(&workflows_dir) {
            Ok(entries) => {
                // Find all workflow files (*.yml, *.yaml) in the standard directory
                let files: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        if let Some(ext) = path.extension() {
                            ext == "yml" || ext == "yaml"
                        } else {
                            false
                        }
                    })
                    .collect();
                workflow_files.extend(files);
            }
            Err(e) => {
                discovered
                    .errors
                    .push(format!("Failed to read .github/workflows directory: {}", e));
            }
        }
    }

    // 2. Check root directory for workflow.yml or .github/workflow.yml
    for filename in &[
        "workflow.yml",
        "workflow.yaml",
        ".github/workflow.yml",
        ".github/workflow.yaml",
    ] {
        let file_path = dir.join(filename);
        if file_path.exists() && file_path.is_file() {
            workflow_files.push(file_path);
        }
    }

    // 3. Check custom directories that might contain workflows
    for custom_dir in &["workflows", "custom/workflows", ".gitlab/workflows"] {
        let custom_path = dir.join(custom_dir);
        if custom_path.exists() && custom_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&custom_path) {
                let files: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        if let Some(ext) = path.extension() {
                            ext == "yml" || ext == "yaml"
                        } else {
                            false
                        }
                    })
                    .collect();
                workflow_files.extend(files);
            }
        }
    }

    if workflow_files.is_empty() {
        return Ok(());
    }

    // Parse all the found workflow files
    let mut all_tasks = Vec::new();
    let mut errors = Vec::new();

    // Create a common parent directory for all workflows
    let workflows_parent = dir.join(".github").join("workflows");

    for file_path in workflow_files {
        match parse_github_actions(&file_path) {
            Ok(mut tasks) => {
                // Override the file path to use the common parent directory
                // instead of individual workflow files
                for task in &mut tasks {
                    task.file_path = workflows_parent.clone();
                }
                all_tasks.extend(tasks);
            }
            Err(e) => errors.push(format!(
                "Failed to parse workflow file {:?}: {}",
                file_path, e
            )),
        }
    }

    if !errors.is_empty() {
        discovered.errors.extend(errors);
    }

    if !all_tasks.is_empty() {
        discovered.definitions.github_actions = Some(TaskDefinitionFile {
            path: workflows_parent,
            definition_type: TaskDefinitionType::GitHubActions,
            status: TaskFileStatus::Parsed,
        });
        discovered.tasks.extend(all_tasks);
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
                            shadowed_by: check_shadowing(&name),
                            disambiguated_name: None,
                        });
                    }
                }
            }
        }
    }
}

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
        let makefile_content = r#".PHONY: test cd

test:
	@echo "Running tests"
cd:
	@echo "Change directory"
"#;
        create_test_makefile(temp_dir.path(), makefile_content);

        // Create package.json with 'test' task
        let package_json_path = temp_dir.path().join("package.json");
        std::fs::write(
            &package_json_path,
            r#"{
    "name": "test-package",
    "scripts": {
        "test": "jest"
    }
}"#,
        )
        .unwrap();

        let discovered = discover_tasks(temp_dir.path());

        // Both tasks should be discovered
        assert!(discovered.tasks.len() >= 2);

        // Verify both test tasks exist with different runners
        let make_test = discovered
            .tasks
            .iter()
            .find(|t| matches!(t.runner, TaskRunner::Make) && t.name == "test")
            .unwrap();

        // Check description contains "Running" but don't depend on exact text
        assert!(make_test.description.as_ref().unwrap().contains("Running"));

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
    #[serial]
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
        assert!(discovered
            .tasks
            .iter()
            .any(|t| t.name == "maven-compiler-plugin:compile"));
        assert!(discovered
            .tasks
            .iter()
            .any(|t| t.name == "spring-boot-maven-plugin:build-info"));

        // Verify task runners
        for task in discovered.tasks {
            if task.definition_type == TaskDefinitionType::MavenPom {
                assert_eq!(task.runner, TaskRunner::Maven);
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_discover_tasks_with_missing_runners() {
        // Setup
        reset_mock();
        enable_mock();

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();

        // Create a pom.xml file but don't mock the mvn executable
        let pom_content = r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
            <modelVersion>4.0.0</modelVersion>
            <groupId>com.example</groupId>
            <artifactId>test</artifactId>
            <version>1.0.0</version>
        </project>"#;
        let pom_path = temp_dir.path().join("pom.xml");
        let mut pom_file = File::create(&pom_path).unwrap();
        pom_file.write_all(pom_content.as_bytes()).unwrap();

        // Create a build.gradle file but don't mock the gradle executable
        let gradle_content = "task gradleTest { description 'Test task' }";
        let gradle_path = temp_dir.path().join("build.gradle");
        let mut gradle_file = File::create(&gradle_path).unwrap();
        gradle_file.write_all(gradle_content.as_bytes()).unwrap();

        // Set up empty test environment (no executables available)
        let env = TestEnvironment::new();
        set_test_environment(env);

        // Discover tasks
        let discovered = discover_tasks(temp_dir.path());

        // Even though runners are unavailable, tasks should still be discovered
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.runner == TaskRunner::Maven),
            "Maven tasks should be discovered even if runner is unavailable"
        );
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.runner == TaskRunner::Gradle),
            "Gradle tasks should be discovered even if runner is unavailable"
        );

        // Verify that the tasks are marked as having unavailable runners
        for task in &discovered.tasks {
            if task.runner == TaskRunner::Maven || task.runner == TaskRunner::Gradle {
                assert!(
                    !crate::runner::is_runner_available(&task.runner),
                    "Runner for {} should be marked as unavailable",
                    task.name
                );
            }
        }

        // Cleanup
        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    fn test_discover_github_actions_tasks_in_different_locations() {
        let temp_dir = TempDir::new().unwrap();

        // Create .github/workflows directory
        let github_workflows_dir = temp_dir.path().join(".github").join("workflows");
        std::fs::create_dir_all(&github_workflows_dir).unwrap();

        // Create a GitHub Actions workflow file in the standard location
        let github_workflow_content = r#"
name: CI
on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build
        run: echo "Building..."
  
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Test
        run: echo "Testing..."
"#;
        std::fs::write(github_workflows_dir.join("ci.yml"), github_workflow_content).unwrap();

        // Create a workflow file in the project root (should still be discovered)
        let root_workflow_content = r#"
name: Root Workflow
on: [push]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Deploy
        run: echo "Deploying..."
"#;
        std::fs::write(temp_dir.path().join("workflow.yml"), root_workflow_content).unwrap();

        // Create a workflow file in a custom directory
        let custom_dir = temp_dir.path().join("custom").join("workflows");
        std::fs::create_dir_all(&custom_dir).unwrap();
        let custom_workflow_content = r#"
name: Custom Workflow
on: [workflow_dispatch]

jobs:
  custom:
    runs-on: ubuntu-latest
    steps:
      - name: Custom Action
        run: echo "Custom action..."
"#;
        std::fs::write(custom_dir.join("custom.yml"), custom_workflow_content).unwrap();

        // Run task discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check GitHub Actions status
        assert!(matches!(
            discovered.definitions.github_actions.unwrap().status,
            TaskFileStatus::Parsed
        ));

        // Check if all workflows are discovered
        let act_tasks: Vec<&Task> = discovered
            .tasks
            .iter()
            .filter(|t| t.runner == TaskRunner::Act)
            .collect();

        // Should find 3 tasks: CI from .github/workflows/ci.yml,
        // Root Workflow from workflow.yml, and Custom Workflow from custom/workflows/custom.yml
        assert_eq!(
            act_tasks.len(),
            3,
            "Should discover 3 GitHub Actions workflows"
        );

        // Check for specific workflow names - now based on filenames
        let workflow_names: Vec<&str> = act_tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(workflow_names.contains(&"ci"));
        assert!(workflow_names.contains(&"workflow"));
        assert!(workflow_names.contains(&"custom"));

        // With the new workflow grouping, all tasks should have the same workflow directory
        let common_path = temp_dir.path().join(".github").join("workflows");
        for task in act_tasks {
            assert_eq!(task.file_path, common_path);
        }
    }
}
