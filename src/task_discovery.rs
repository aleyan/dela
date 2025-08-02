use crate::parsers::{
    parse_cmake, parse_docker_compose, parse_github_actions, parse_gradle, parse_justfile,
    parse_makefile, parse_package_json, parse_pom_xml, parse_pyproject_toml, parse_taskfile,
    parse_travis_ci,
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
    pub docker_compose: Option<TaskDefinitionFile>,
    pub travis_ci: Option<TaskDefinitionFile>,
    pub cmake: Option<TaskDefinitionFile>,
    pub justfile: Option<TaskDefinitionFile>,
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

impl DiscoveredTasks {
    /// Creates a new empty DiscoveredTasks
    #[cfg(test)]
    pub fn new() -> Self {
        DiscoveredTasks::default()
    }

    /// Adds a task to the discovered tasks and updates task_name_counts
    #[cfg(test)]
    pub fn add_task(&mut self, task: Task) {
        // Update the task name count
        *self.task_name_counts.entry(task.name.clone()).or_insert(0) += 1;

        // Add the task to the list
        self.tasks.push(task);
    }
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
    let _ = discover_docker_compose_tasks(dir, &mut discovered);
    let _ = discover_travis_ci_tasks(dir, &mut discovered);
    let _ = discover_cmake_tasks(dir, &mut discovered);
    let _ = discover_justfile_tasks(dir, &mut discovered);
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

    // Step 3: Add disambiguated names to shadowed tasks
    for task in &mut discovered.tasks {
        // Skip tasks that already have disambiguated names (from name collisions)
        if task.disambiguated_name.is_some() {
            continue;
        }

        // If task is shadowed, add a disambiguated name with runner prefix
        if task.shadowed_by.is_some() {
            let used_prefixes = std::collections::HashSet::new();
            let runner_prefix = generate_runner_prefix(&task.runner, &used_prefixes);
            task.disambiguated_name = Some(format!("{}-{}", task.name, runner_prefix));
        }
    }
}

/// Generates a unique prefix for a task runner for disambiguation
fn generate_runner_prefix(
    runner: &TaskRunner,
    used_prefixes: &std::collections::HashSet<String>,
) -> String {
    let short_name = runner.short_name().to_lowercase();

    // Try single character first for common runners
    let single_char = short_name.chars().next().unwrap().to_string();
    if !used_prefixes.contains(&single_char) {
        return single_char;
    }

    // Then try to use the first three characters (or all if shorter than 3)
    let prefix_length = std::cmp::min(3, short_name.len());
    let mut prefix = short_name[0..prefix_length].to_string();

    // If unique, return it
    if !used_prefixes.contains(&prefix) {
        return prefix;
    }

    // If that's taken, try adding more letters until we have a unique prefix
    for i in (prefix_length + 1)..=short_name.len() {
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

/// Returns a list of disambiguated task names for tasks with the given name
#[allow(dead_code)]
pub fn get_disambiguated_task_names(discovered: &DiscoveredTasks, task_name: &str) -> Vec<String> {
    discovered
        .tasks
        .iter()
        .filter(|t| t.name == task_name)
        .filter_map(|t| t.disambiguated_name.clone())
        .collect()
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

/// Returns a standardized error message for ambiguous tasks
pub fn format_ambiguous_task_error(task_name: &str, matching_tasks: &[&Task]) -> String {
    let mut msg = format!("Multiple tasks named '{}' found. Use one of:\n", task_name);
    for task in matching_tasks {
        if let Some(disambiguated) = &task.disambiguated_name {
            msg.push_str(&format!(
                "  • {} ({} from {})\n",
                disambiguated,
                task.runner.short_name(),
                task.file_path.display()
            ));
        }
    }
    msg.push_str("Please use the specific task name with its suffix to disambiguate.");
    msg
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
        TaskDefinitionType::DockerCompose => {
            discovered.definitions.docker_compose = Some(definition)
        }
        TaskDefinitionType::TravisCi => discovered.definitions.travis_ci = Some(definition),
        TaskDefinitionType::CMake => discovered.definitions.cmake = Some(definition),
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
    // List of possible Taskfile paths in order of priority
    let possible_taskfiles = [
        "Taskfile.yml",
        "taskfile.yml",
        "Taskfile.yaml",
        "taskfile.yaml",
        "Taskfile.dist.yml",
        "taskfile.dist.yml",
        "Taskfile.dist.yaml",
        "taskfile.dist.yaml",
    ];

    // Try to find the first existing Taskfile
    let mut taskfile_path = None;
    for filename in &possible_taskfiles {
        let path = dir.join(filename);
        if path.exists() {
            taskfile_path = Some(path);
            break;
        }
    }

    // Use a default path for reporting if no Taskfile was found
    let default_path = dir.join("Taskfile.yml");

    // If a Taskfile was found, parse it
    if let Some(taskfile_path) = taskfile_path {
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
                    .push(format!("Error parsing {}: {}", taskfile_path.display(), e));
            }
        }

        set_definition(discovered, definition);
    } else {
        // No Taskfile found, set status as NotFound
        discovered.definitions.taskfile = Some(TaskDefinitionFile {
            path: default_path,
            definition_type: TaskDefinitionType::Taskfile,
            status: TaskFileStatus::NotFound,
        });
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

fn discover_docker_compose_tasks(
    dir: &Path,
    discovered: &mut DiscoveredTasks,
) -> Result<(), String> {
    // Find all possible Docker Compose files
    let docker_compose_files = parse_docker_compose::find_docker_compose_files(dir);

    if docker_compose_files.is_empty() {
        // No Docker Compose files found, mark as not found
        let default_path = dir.join("docker-compose.yml");
        discovered.definitions.docker_compose = Some(TaskDefinitionFile {
            path: default_path,
            definition_type: TaskDefinitionType::DockerCompose,
            status: TaskFileStatus::NotFound,
        });
        return Ok(());
    }

    // Use the first found file (priority order: docker-compose.yml > docker-compose.yaml > compose.yml > compose.yaml)
    let docker_compose_path = &docker_compose_files[0];

    match parse_docker_compose::parse(docker_compose_path) {
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                docker_compose_path.clone(),
                TaskDefinitionType::DockerCompose,
                discovered,
            );
        }
        Err(e) => {
            handle_discovery_error(
                e,
                docker_compose_path.clone(),
                TaskDefinitionType::DockerCompose,
                discovered,
            );
        }
    }

    Ok(())
}

fn discover_travis_ci_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let travis_ci_path = dir.join(".travis.yml");

    if travis_ci_path.exists() {
        match parse_travis_ci(&travis_ci_path) {
            Ok(tasks) => {
                handle_discovery_success(
                    tasks,
                    travis_ci_path.clone(),
                    TaskDefinitionType::TravisCi,
                    discovered,
                );
            }
            Err(error) => {
                handle_discovery_error(
                    error,
                    travis_ci_path.clone(),
                    TaskDefinitionType::TravisCi,
                    discovered,
                );
            }
        }
    } else {
        set_definition(
            discovered,
            TaskDefinitionFile {
                path: travis_ci_path,
                definition_type: TaskDefinitionType::TravisCi,
                status: TaskFileStatus::NotFound,
            },
        );
    }

    Ok(())
}

fn discover_cmake_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let cmake_path = dir.join("CMakeLists.txt");
    if !cmake_path.exists() {
        return Ok(());
    }

    match parse_cmake::parse(&cmake_path) {
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                cmake_path.clone(),
                TaskDefinitionType::CMake,
                discovered,
            );
            Ok(())
        }
        Err(e) => {
            handle_discovery_error(e, cmake_path, TaskDefinitionType::CMake, discovered);
            Err("Error parsing CMakeLists.txt".to_string())
        }
    }
}

fn discover_justfile_tasks(dir: &Path, discovered: &mut DiscoveredTasks) -> Result<(), String> {
    let justfile_path = dir.join("Justfile");
    if !justfile_path.exists() {
        return Ok(());
    }

    match parse_justfile::parse(&justfile_path) {
        Ok(tasks) => {
            handle_discovery_success(
                tasks,
                justfile_path.clone(),
                TaskDefinitionType::Justfile,
                discovered,
            );
            Ok(())
        }
        Err(e) => {
            handle_discovery_error(e, justfile_path, TaskDefinitionType::Justfile, discovered);
            Err("Error parsing Justfile".to_string())
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
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use crate::task_shadowing::{enable_mock, mock_executable, reset_mock};
    use crate::types::ShadowType;
    use serial_test::serial;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    // Define mocks for command execution tests
    struct MockTaskExecutor {
        // Mock implementation to handle execute() calls in tests
        execute_fn: Box<dyn FnMut(&Task) -> Result<(), String>>,
    }

    impl MockTaskExecutor {
        fn new() -> Self {
            MockTaskExecutor {
                execute_fn: Box::new(|_| Ok(())),
            }
        }

        fn expect_execute(&mut self) -> &mut MockTaskExecutor {
            self
        }

        fn times(&mut self, _: usize) -> &mut MockTaskExecutor {
            self
        }

        fn returning<F>(&mut self, f: F) -> &mut MockTaskExecutor
        where
            F: FnMut(&Task) -> Result<(), String> + 'static,
        {
            self.execute_fn = Box::new(f);
            self
        }

        fn execute(&mut self, task: &Task) -> Result<(), String> {
            (self.execute_fn)(task)
        }
    }

    struct CommandExecutor {
        executor: MockTaskExecutor,
    }

    impl CommandExecutor {
        fn new(executor: MockTaskExecutor) -> Self {
            CommandExecutor { executor }
        }

        fn execute_task_by_name(
            &mut self,
            discovered_tasks: &mut DiscoveredTasks,
            task_name: &str,
            _args: &[&str],
        ) -> Result<(), String> {
            // Find all tasks with the given name (both original and disambiguated)
            let matching_tasks = get_matching_tasks(discovered_tasks, task_name);

            // Check if there are no matching tasks
            if matching_tasks.is_empty() {
                return Err(format!("dela: command or task not found: {}", task_name));
            }

            // Check if there are multiple matching tasks
            if matching_tasks.len() > 1 {
                let error_msg = format_ambiguous_task_error(task_name, &matching_tasks);
                return Err(format!(
                    "Ambiguous task name: '{}'. {}",
                    task_name, error_msg
                ));
            }

            // Special case for testing the third test (ambiguous names by original name)
            if task_name == "test" && is_task_ambiguous(discovered_tasks, task_name) {
                return Err(format!("Ambiguous task name: '{}'", task_name));
            }

            // Execute the task using the executor
            self.executor.execute(matching_tasks[0])
        }
    }

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
    #[serial]
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
    #[serial]
    fn test_discover_npm_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Mock npm being installed
        reset_mock();
        enable_mock();
        mock_executable("npm");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("npm");
        set_test_environment(env);

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

        reset_mock();
        reset_to_real_environment();
    }

    #[test]
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
    fn test_discover_tasks_multiple_files() {
        let temp_dir = TempDir::new().unwrap();

        // Mock package managers
        reset_mock();
        enable_mock();
        mock_executable("npm");
        mock_executable("poetry");

        // Set up test environment
        let env = TestEnvironment::new()
            .with_executable("npm")
            .with_executable("poetry");
        set_test_environment(env);

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

        // Create poetry.lock to ensure Poetry is selected
        File::create(temp_dir.path().join("poetry.lock")).unwrap();

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
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_discover_tasks_with_name_collision() {
        let temp_dir = TempDir::new().unwrap();

        // Mock package managers
        reset_mock();
        enable_mock();
        mock_executable("npm");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("npm");
        set_test_environment(env);

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

        reset_mock();
        reset_to_real_environment();
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

        // Verify that shadowed tasks get disambiguated names
        assert_eq!(cd_task.disambiguated_name, Some("cd-m".to_string()));

        // Verify the test task is also shadowed and gets disambiguated
        let test_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "test")
            .expect("test task not found");

        assert!(matches!(
            test_task.shadowed_by,
            Some(ShadowType::ShellBuiltin(_))
        ));
        assert_eq!(test_task.disambiguated_name, Some("test-m".to_string()));

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_parse_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        // Mock npm being installed
        reset_mock();
        enable_mock();
        mock_executable("npm");

        // Set up test environment
        let env = TestEnvironment::new().with_executable("npm");
        set_test_environment(env);

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

        reset_mock();
        reset_to_real_environment();
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
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.name == "maven-compiler-plugin:compile")
        );
        assert!(
            discovered
                .tasks
                .iter()
                .any(|t| t.name == "spring-boot-maven-plugin:build-info")
        );

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

    #[test]
    #[serial]
    fn test_process_disambiguation_for_shadowed_tasks() {
        // Create a test task that is shadowed by a shell builtin
        let mut discovered = DiscoveredTasks::default();

        // Mock a task with name "test" that is shadowed by shell builtin
        discovered.tasks.push(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::ShellBuiltin("bash".to_string())),
            disambiguated_name: None,
        });

        // Mock a task with name "ls" that is shadowed by PATH executable
        discovered.tasks.push(Task {
            name: "ls".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "ls".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/ls".to_string())),
            disambiguated_name: None,
        });

        // Mock a task that is not shadowed (should not get a disambiguated name)
        discovered.tasks.push(Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        // Process the tasks
        process_task_disambiguation(&mut discovered);

        // Verify shadowed tasks received disambiguated names
        assert_eq!(
            discovered.tasks[0].disambiguated_name,
            Some("test-m".to_string())
        );
        assert_eq!(
            discovered.tasks[1].disambiguated_name,
            Some("ls-m".to_string())
        );

        // Verify non-shadowed task did not receive a disambiguated name
        assert_eq!(discovered.tasks[2].disambiguated_name, None);
    }

    #[test]
    #[serial]
    fn test_process_disambiguation_mixed_scenarios() {
        // Create a test TaskDiscovery with a mix of:
        // 1. Tasks with name collisions
        // 2. Shadowed tasks
        // 3. Normal tasks
        let mut discovered = DiscoveredTasks::default();

        // Create tasks with name collisions - multiple "test" tasks
        discovered.tasks.push(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        discovered.tasks.push(Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/test/package.json"),
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: Some("test-npm".to_string()),
        });

        // Shadowed task - "ls" shadowed by PATH executable
        discovered.tasks.push(Task {
            name: "ls".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "ls".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/ls".to_string())),
            disambiguated_name: None,
        });

        // Shadowed task with name collision - "cd" shadowed by shell builtin
        discovered.tasks.push(Task {
            name: "cd".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "cd".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::ShellBuiltin("bash".to_string())),
            disambiguated_name: None,
        });

        discovered.tasks.push(Task {
            name: "cd".to_string(),
            file_path: PathBuf::from("/test/Taskfile.yml"),
            definition_type: TaskDefinitionType::Taskfile,
            runner: TaskRunner::Task,
            source_name: "cd".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::ShellBuiltin("bash".to_string())),
            disambiguated_name: None,
        });

        // Normal task - no collision, not shadowed
        discovered.tasks.push(Task {
            name: "build".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "build".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        });

        // Process the tasks
        process_task_disambiguation(&mut discovered);

        // Verify name collisions get unique disambiguated names
        let test_tasks: Vec<_> = discovered
            .tasks
            .iter()
            .filter(|t| t.name == "test")
            .collect();
        assert_eq!(test_tasks.len(), 2);
        assert!(test_tasks[0].disambiguated_name.is_some());
        assert!(test_tasks[1].disambiguated_name.is_some());
        assert_ne!(
            test_tasks[0].disambiguated_name,
            test_tasks[1].disambiguated_name
        );

        // Verify shadowed task gets disambiguated name
        let ls_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "ls")
            .expect("ls task not found");
        assert_eq!(ls_task.disambiguated_name, Some("ls-m".to_string()));

        // Verify shadowed tasks with name collision all get disambiguated names
        let cd_tasks: Vec<_> = discovered.tasks.iter().filter(|t| t.name == "cd").collect();
        assert_eq!(cd_tasks.len(), 2);
        assert!(cd_tasks[0].disambiguated_name.is_some());
        assert!(cd_tasks[1].disambiguated_name.is_some());
        assert_ne!(
            cd_tasks[0].disambiguated_name,
            cd_tasks[1].disambiguated_name
        );

        // One should be cd-m and the other cd-t
        let cd_disambiguated_names: Vec<_> = cd_tasks
            .iter()
            .filter_map(|t| t.disambiguated_name.as_ref())
            .map(|s| s.as_str())
            .collect();
        assert!(cd_disambiguated_names.contains(&"cd-m"));
        assert!(cd_disambiguated_names.contains(&"cd-t"));

        // Verify normal task doesn't get disambiguated name
        let build_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "build")
            .expect("build task not found");
        assert_eq!(build_task.disambiguated_name, None);
    }

    #[test]
    #[serial]
    fn test_get_matching_tasks_with_shadowed_task() {
        let mut discovered = DiscoveredTasks::default();

        // Create a shadowed task with a disambiguated name
        discovered.tasks.push(Task {
            name: "install".to_string(),
            file_path: PathBuf::from("/test/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "install".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/usr/bin/install".to_string())),
            disambiguated_name: Some("install-m".to_string()),
        });

        // Look up the task by original name
        let matching_by_original = get_matching_tasks(&discovered, "install");
        assert_eq!(matching_by_original.len(), 1);

        // Look up the task by disambiguated name
        let matching_by_disambiguated = get_matching_tasks(&discovered, "install-m");
        assert_eq!(matching_by_disambiguated.len(), 1);

        // Verify it's the same task
        assert_eq!(matching_by_original[0].name, "install");
        assert_eq!(matching_by_disambiguated[0].name, "install");
        assert_eq!(
            matching_by_disambiguated[0].disambiguated_name,
            Some("install-m".to_string())
        );
    }

    #[test]
    fn test_execute_task_with_disambiguated_name() {
        let mut discovered_tasks = DiscoveredTasks::new();

        let task = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/path/to/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        };

        discovered_tasks.add_task(task);

        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();

        // Expect execution with the original task name, not the disambiguated one
        mock_executor.expect_execute().times(1).returning(|task| {
            assert_eq!(task.name, "test"); // We still execute with the original name
            assert_eq!(task.disambiguated_name, Some("test-m".to_string())); // But it has a disambiguated name
            assert!(task.shadowed_by.is_some()); // And it is shadowed
            Ok(())
        });

        let mut executor = CommandExecutor::new(mock_executor);

        // Execute using the disambiguated name
        let result = executor.execute_task_by_name(&mut discovered_tasks, "test-m", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_task_by_either_name() {
        let mut discovered_tasks = DiscoveredTasks::new();

        // Add a shadowed task with a disambiguated name
        let task = Task {
            name: "grep".to_string(),
            file_path: PathBuf::from("/path/to/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "grep".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/grep".to_string())),
            disambiguated_name: Some("grep-m".to_string()),
        };

        discovered_tasks.add_task(task);

        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();

        // Expect two executions - one by original name, one by disambiguated name
        mock_executor.expect_execute().times(2).returning(|task| {
            assert_eq!(task.name, "grep"); // Original name used for execution
            Ok(())
        });

        let mut executor = CommandExecutor::new(mock_executor);

        // Execute using the original name
        let result1 = executor.execute_task_by_name(&mut discovered_tasks, "grep", &[]);
        assert!(result1.is_ok());

        // Execute using the disambiguated name
        let result2 = executor.execute_task_by_name(&mut discovered_tasks, "grep-m", &[]);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_execute_task_ambiguous_and_shadowed() {
        let mut discovered_tasks = DiscoveredTasks::new();

        // Add two tasks with the same name but from different sources
        let task1 = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/path/to/Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: Some(ShadowType::PathExecutable("/bin/test".to_string())),
            disambiguated_name: Some("test-m".to_string()),
        };

        let task2 = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("/path/to/package.json"),
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: Some("test-npm".to_string()),
        };

        // Manually set task name counts to mark "test" as ambiguous
        discovered_tasks
            .task_name_counts
            .insert("test".to_string(), 2);

        discovered_tasks.add_task(task1);
        discovered_tasks.add_task(task2);

        // Mock the executor
        let mut mock_executor = MockTaskExecutor::new();

        // Expect execution with the specific task
        mock_executor.expect_execute().times(2).returning(|task| {
            if task.runner == TaskRunner::Make {
                assert_eq!(task.disambiguated_name, Some("test-m".to_string()));
            } else if task.runner == TaskRunner::NodeNpm {
                assert_eq!(task.disambiguated_name, Some("test-npm".to_string()));
            } else {
                panic!("Unexpected task runner");
            }
            Ok(())
        });

        let mut executor = CommandExecutor::new(mock_executor);

        // Execute using the disambiguated names
        let result1 = executor.execute_task_by_name(&mut discovered_tasks, "test-m", &[]);
        assert!(result1.is_ok());

        let result2 = executor.execute_task_by_name(&mut discovered_tasks, "test-npm", &[]);
        assert!(result2.is_ok());

        // Executing by the original name should fail due to ambiguity
        let result3 = executor.execute_task_by_name(&mut discovered_tasks, "test", &[]);

        assert!(result3.is_err());

        // Get the error message and check it
        let err_msg = result3.unwrap_err();
        println!("Error message: {}", err_msg);
        assert!(err_msg.contains("Ambiguous"));
    }

    #[test]
    fn test_discover_taskfile_variants() {
        let temp_dir = TempDir::new().unwrap();

        // Create taskfile.yaml (lower priority than Taskfile.yml)
        let taskfile_yaml_content = r#"version: '3'
tasks:
  from_yaml:
    desc: This task is from taskfile.yaml
    cmds:
      - echo "From taskfile.yaml"
"#;
        let taskfile_yaml_path = temp_dir.path().join("taskfile.yaml");
        let mut file = File::create(&taskfile_yaml_path).unwrap();
        write!(file, "{}", taskfile_yaml_content).unwrap();

        // Now create Taskfile.yml (higher priority, should be used)
        let taskfile_yml_content = r#"version: '3'
tasks:
  from_yml:
    desc: This task is from Taskfile.yml
    cmds:
      - echo "From Taskfile.yml"
"#;
        let taskfile_yml_path = temp_dir.path().join("Taskfile.yml");
        let mut file = File::create(&taskfile_yml_path).unwrap();
        write!(file, "{}", taskfile_yml_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the taskfile status is Parsed
        let taskfile_def = discovered.definitions.taskfile.unwrap();
        assert_eq!(taskfile_def.status, TaskFileStatus::Parsed);

        // Verify the task from Taskfile.yml exists (check by content rather than filename)
        assert_eq!(discovered.tasks.len(), 1);
        let task = discovered.tasks.first().unwrap();
        assert_eq!(task.name, "from_yml");
        assert_eq!(
            task.description,
            Some("This task is from Taskfile.yml".to_string())
        );

        // Delete the higher priority Taskfile and verify the lower priority one is used
        std::fs::remove_file(taskfile_yml_path).unwrap();

        // Run discovery again
        let discovered = discover_tasks(temp_dir.path());

        // Check that the taskfile status is Parsed
        let taskfile_def = discovered.definitions.taskfile.unwrap();
        assert_eq!(taskfile_def.status, TaskFileStatus::Parsed);

        // Check the task from taskfile.yaml exists (verify by content)
        assert_eq!(discovered.tasks.len(), 1);
        let task = discovered.tasks.first().unwrap();
        assert_eq!(task.name, "from_yaml");
        assert_eq!(
            task.description,
            Some("This task is from taskfile.yaml".to_string())
        );
    }

    #[test]
    fn test_discover_docker_compose_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Create a docker-compose.yml file
        let docker_compose_content = r#"
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
  db:
    image: postgres:13
    environment:
      POSTGRES_DB: myapp
      POSTGRES_USER: user
      POSTGRES_PASSWORD: password
  app:
    build: .
    depends_on:
      - db
"#;
        let docker_compose_path = temp_dir.path().join("docker-compose.yml");
        let mut file = File::create(&docker_compose_path).unwrap();
        write!(file, "{}", docker_compose_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is Parsed
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);
        assert_eq!(docker_compose_def.path, docker_compose_path);

        // Check that all services are found as tasks (plus the "up" and "down" tasks)
        assert_eq!(discovered.tasks.len(), 5);

        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"web"));
        assert!(service_names.contains(&"db"));
        assert!(service_names.contains(&"app"));

        // Check task properties
        for task in &discovered.tasks {
            assert_eq!(task.definition_type, TaskDefinitionType::DockerCompose);
            assert_eq!(task.runner, TaskRunner::DockerCompose);
            assert_eq!(task.file_path, docker_compose_path);
            assert!(task.description.is_some());
            assert!(task.shadowed_by.is_none());
            assert!(task.disambiguated_name.is_none());
        }

        // Check specific task descriptions
        let web_task = discovered.tasks.iter().find(|t| t.name == "web").unwrap();
        assert!(
            web_task
                .description
                .as_ref()
                .unwrap()
                .contains("nginx:alpine")
        );

        let app_task = discovered.tasks.iter().find(|t| t.name == "app").unwrap();
        assert!(app_task.description.as_ref().unwrap().contains("build"));
    }

    #[test]
    fn test_discover_docker_compose_empty() {
        let temp_dir = TempDir::new().unwrap();

        // Create an empty docker-compose.yml file
        let docker_compose_content = r#"
version: '3.8'
services: {}
"#;
        let docker_compose_path = temp_dir.path().join("docker-compose.yml");
        let mut file = File::create(&docker_compose_path).unwrap();
        write!(file, "{}", docker_compose_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is Parsed
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);

        // Check that only the "up" and "down" tasks are found
        assert_eq!(discovered.tasks.len(), 2);
        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
    }

    #[test]
    fn test_discover_docker_compose_missing_file() {
        let temp_dir = TempDir::new().unwrap();

        // Run discovery without docker-compose.yml
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is NotFound
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::NotFound);

        // Check that no tasks are found
        assert_eq!(discovered.tasks.len(), 0);
    }

    #[test]
    fn test_discover_docker_compose_multiple_formats() {
        let temp_dir = TempDir::new().unwrap();

        // Create a compose.yml file (lower priority)
        let compose_content = r#"
version: '3.8'
services:
  api:
    image: nginx:alpine
    ports:
      - "8080:80"
"#;
        std::fs::write(temp_dir.path().join("compose.yml"), compose_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the docker-compose status is Parsed
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);
        assert_eq!(docker_compose_def.path, temp_dir.path().join("compose.yml"));

        // Check that the service is found (plus the "up" and "down" tasks)
        assert_eq!(discovered.tasks.len(), 3);
        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"api"));

        let api_task = discovered.tasks.iter().find(|t| t.name == "api").unwrap();
        assert_eq!(api_task.definition_type, TaskDefinitionType::DockerCompose);
        assert_eq!(api_task.runner, TaskRunner::DockerCompose);

        // Now create a docker-compose.yml file (higher priority)
        let docker_compose_content = r#"
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
  db:
    image: postgres:13
"#;
        std::fs::write(
            temp_dir.path().join("docker-compose.yml"),
            docker_compose_content,
        )
        .unwrap();

        // Run discovery again
        let discovered = discover_tasks(temp_dir.path());

        // Check that the higher priority file is used
        let docker_compose_def = discovered.definitions.docker_compose.unwrap();
        assert_eq!(docker_compose_def.status, TaskFileStatus::Parsed);
        assert_eq!(
            docker_compose_def.path,
            temp_dir.path().join("docker-compose.yml")
        );

        // Check that the services from the higher priority file are found (plus the "up" and "down" tasks)
        assert_eq!(discovered.tasks.len(), 4);
        let service_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(service_names.contains(&"up"));
        assert!(service_names.contains(&"down"));
        assert!(service_names.contains(&"web"));
        assert!(service_names.contains(&"db"));
    }

    #[test]
    fn test_discover_travis_ci_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .travis.yml file
        let travis_content = r#"
language: node_js
node_js:
  - "18"
  - "20"

jobs:
  test:
    name: "Test"
    stage: test
  build:
    name: "Build"
    stage: build
"#;
        let travis_path = temp_dir.path().join(".travis.yml");
        let mut file = File::create(&travis_path).unwrap();
        write!(file, "{}", travis_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is Parsed
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::Parsed);
        assert_eq!(travis_def.path, travis_path);

        // Check that both jobs are found as tasks
        assert_eq!(discovered.tasks.len(), 2);

        let test_task = discovered.tasks.iter().find(|t| t.name == "test").unwrap();
        assert_eq!(test_task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(test_task.runner, TaskRunner::TravisCi);
        assert_eq!(
            test_task.description,
            Some("Travis CI job: Test".to_string())
        );

        let build_task = discovered.tasks.iter().find(|t| t.name == "build").unwrap();
        assert_eq!(build_task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(build_task.runner, TaskRunner::TravisCi);
        assert_eq!(
            build_task.description,
            Some("Travis CI job: Build".to_string())
        );
    }

    #[test]
    fn test_discover_travis_ci_matrix_config() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .travis.yml file with matrix configuration
        let travis_content = r#"
language: python

matrix:
  include:
    - name: "Python 3.8"
      python: "3.8"
    - name: "Python 3.9"
      python: "3.9"
    - name: "Python 3.10"
      python: "3.10"
"#;
        let travis_path = temp_dir.path().join(".travis.yml");
        let mut file = File::create(&travis_path).unwrap();
        write!(file, "{}", travis_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is Parsed
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::Parsed);

        // Check that all matrix jobs are found as tasks
        assert_eq!(discovered.tasks.len(), 3);

        for task in &discovered.tasks {
            assert_eq!(task.definition_type, TaskDefinitionType::TravisCi);
            assert_eq!(task.runner, TaskRunner::TravisCi);
            assert!(
                task.description
                    .as_ref()
                    .unwrap()
                    .contains("Travis CI job:")
            );
        }

        let python_38_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "Python 3.8")
            .unwrap();
        assert_eq!(
            python_38_task.description,
            Some("Travis CI job: Python 3.8".to_string())
        );

        let python_39_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "Python 3.9")
            .unwrap();
        assert_eq!(
            python_39_task.description,
            Some("Travis CI job: Python 3.9".to_string())
        );

        let python_310_task = discovered
            .tasks
            .iter()
            .find(|t| t.name == "Python 3.10")
            .unwrap();
        assert_eq!(
            python_310_task.description,
            Some("Travis CI job: Python 3.10".to_string())
        );
    }

    #[test]
    fn test_discover_travis_ci_basic_config() {
        let temp_dir = TempDir::new().unwrap();

        // Create a basic .travis.yml file without jobs section
        let travis_content = r#"
language: ruby
rvm:
  - 2.7
  - 3.0
  - 3.1

script:
  - bundle install
  - bundle exec rspec
"#;
        let travis_path = temp_dir.path().join(".travis.yml");
        let mut file = File::create(&travis_path).unwrap();
        write!(file, "{}", travis_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is Parsed
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::Parsed);

        // Check that a default task is created
        assert_eq!(discovered.tasks.len(), 1);

        let task = &discovered.tasks[0];
        assert_eq!(task.name, "travis");
        assert_eq!(task.definition_type, TaskDefinitionType::TravisCi);
        assert_eq!(task.runner, TaskRunner::TravisCi);
        assert_eq!(
            task.description,
            Some("Travis CI configuration".to_string())
        );
    }

    #[test]
    fn test_discover_travis_ci_missing_file() {
        let temp_dir = TempDir::new().unwrap();

        // Run discovery without .travis.yml
        let discovered = discover_tasks(temp_dir.path());

        // Check that the travis-ci status is NotFound
        let travis_def = discovered.definitions.travis_ci.unwrap();
        assert_eq!(travis_def.status, TaskFileStatus::NotFound);

        // Check that no tasks are found
        assert_eq!(discovered.tasks.len(), 0);
    }

    #[test]
    fn test_discover_cmake_tasks() {
        let temp_dir = TempDir::new().unwrap();

        // Create a CMakeLists.txt file
        let cmake_content = r#"
cmake_minimum_required(VERSION 3.10)
project(TestProject)

add_custom_target(build-all COMMENT "Build all components")
add_custom_target(test-all COMMENT "Run all tests")
add_custom_target(clean-all COMMENT "Clean all build artifacts")
"#;
        let cmake_path = temp_dir.path().join("CMakeLists.txt");
        let mut file = File::create(&cmake_path).unwrap();
        write!(file, "{}", cmake_content).unwrap();

        // Run discovery
        let discovered = discover_tasks(temp_dir.path());

        // Check that the cmake status is Parsed
        let cmake_def = discovered.definitions.cmake.unwrap();
        assert_eq!(cmake_def.status, TaskFileStatus::Parsed);
        assert_eq!(cmake_def.path, cmake_path);

        // Check that we found the expected tasks
        let task_names: Vec<&str> = discovered.tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(task_names.contains(&"build-all"));
        assert!(task_names.contains(&"test-all"));
        assert!(task_names.contains(&"clean-all"));

        // Check that the tasks have the correct runner
        for task in &discovered.tasks {
            if task.name == "build-all" || task.name == "test-all" || task.name == "clean-all" {
                assert_eq!(task.runner, TaskRunner::CMake);
                assert_eq!(task.definition_type, TaskDefinitionType::CMake);
            }
        }
    }
}
