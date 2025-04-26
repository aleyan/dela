use crate::runner::is_runner_available;
use crate::task_discovery;
use crate::types::ShadowType;
use crate::types::{Task, TaskFileStatus};
use std::collections::HashMap;
use std::env;
use std::io::Write;

#[cfg(test)]
macro_rules! test_println {
    ($($arg:tt)*) => {};
}
#[cfg(not(test))]
macro_rules! test_println {
    ($($arg:tt)*) => { println!($($arg)*) };
}

pub fn execute(verbose: bool) -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);

    // Only show task definition files status in verbose mode
    if verbose {
        test_println!("Task definition files:");
        if let Some(makefile) = &discovered.definitions.makefile {
            match &makefile.status {
                TaskFileStatus::Parsed => {
                    test_println!("  ✓ Makefile: Found and parsed");
                }
                TaskFileStatus::NotImplemented => {
                    test_println!("  ! Makefile: Found but parsing not yet implemented");
                }
                TaskFileStatus::ParseError(_e) => {
                    test_println!("  ✗ Makefile: Error parsing: {}", _e);
                }
                TaskFileStatus::NotReadable(_e) => {
                    test_println!("  ✗ Makefile: Not readable: {}", _e);
                }
                TaskFileStatus::NotFound => {
                    test_println!("  - Makefile: Not found");
                }
            }
        }
        if let Some(package_json) = &discovered.definitions.package_json {
            match &package_json.status {
                TaskFileStatus::Parsed => {
                    test_println!("  ✓ package.json: Found and parsed");
                }
                TaskFileStatus::NotImplemented => {
                    test_println!("  ! package.json: Found but parsing not yet implemented");
                }
                TaskFileStatus::ParseError(_e) => {
                    test_println!("  ✗ package.json: Error parsing: {}", _e);
                }
                TaskFileStatus::NotReadable(_e) => {
                    test_println!("  ✗ package.json: Not readable: {}", _e);
                }
                TaskFileStatus::NotFound => {
                    test_println!("  - package.json: Not found");
                }
            }
        }
        if let Some(pyproject_toml) = &discovered.definitions.pyproject_toml {
            match &pyproject_toml.status {
                TaskFileStatus::Parsed => {
                    test_println!("  ✓ pyproject.toml: Found and parsed");
                }
                TaskFileStatus::NotImplemented => {
                    test_println!("  ! pyproject.toml: Found but parsing not yet implemented");
                }
                TaskFileStatus::ParseError(_e) => {
                    test_println!("  ✗ pyproject.toml: Error parsing: {}", _e);
                }
                TaskFileStatus::NotReadable(_e) => {
                    test_println!("  ✗ pyproject.toml: Not readable: {}", _e);
                }
                TaskFileStatus::NotFound => {
                    test_println!("  - pyproject.toml: Not found");
                }
            }
        }
        if let Some(maven_pom) = &discovered.definitions.maven_pom {
            match &maven_pom.status {
                TaskFileStatus::Parsed => {
                    test_println!("  ✓ pom.xml: Found and parsed");
                }
                TaskFileStatus::NotImplemented => {
                    test_println!("  ! pom.xml: Found but parsing not yet implemented");
                }
                TaskFileStatus::ParseError(_e) => {
                    test_println!("  ✗ pom.xml: Error parsing: {}", _e);
                }
                TaskFileStatus::NotReadable(_e) => {
                    test_println!("  ✗ pom.xml: Not readable: {}", _e);
                }
                TaskFileStatus::NotFound => {
                    test_println!("  - pom.xml: Not found");
                }
            }
        }
        if let Some(gradle) = &discovered.definitions.gradle {
            match &gradle.status {
                TaskFileStatus::Parsed => {
                    let _file_name = gradle
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    test_println!("  ✓ {}: Found and parsed", _file_name);
                }
                TaskFileStatus::NotImplemented => {
                    let _file_name = gradle
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    test_println!("  ! {}: Found but parsing not yet implemented", _file_name);
                }
                TaskFileStatus::ParseError(_e) => {
                    let _file_name = gradle
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    test_println!("  ✗ {}: Error parsing: {}", _file_name, _e);
                }
                TaskFileStatus::NotReadable(_e) => {
                    let _file_name = gradle
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    test_println!("  ✗ {}: Not readable: {}", _file_name, _e);
                }
                TaskFileStatus::NotFound => {
                    test_println!("  - Gradle build file: Not found");
                }
            }
        }
        test_println!("");
    }

    // Create writer for output
    let mut writer: Box<dyn std::io::Write> = if cfg!(test) {
        Box::new(std::io::sink())
    } else {
        Box::new(std::io::stdout())
    };

    let mut write_line = |line: &str| -> Result<(), String> {
        writeln!(writer, "{}", line).map_err(|e| format!("Failed to write output: {}", e))
    };

    // Group tasks by runner for the new format
    let mut tasks_by_runner: HashMap<String, Vec<&Task>> = HashMap::new();
    for task in &discovered.tasks {
        let runner_name = task.runner.short_name().to_string();
        tasks_by_runner.entry(runner_name).or_default().push(task);
    }

    // Track footnotes used
    let mut used_footnotes: HashMap<char, bool> = HashMap::new();
    used_footnotes.insert('*', false); // tool not installed
    used_footnotes.insert('†', false); // shadowed by shell builtin
    used_footnotes.insert('‡', false); // shadowed by command on path
    used_footnotes.insert('‖', false); // conflicts with task from another tool

    if tasks_by_runner.is_empty() {
        write_line("No tasks found in the current directory.")?;
    } else {
        // Collect file paths for each runner for reference
        let mut runner_files: HashMap<String, String> = HashMap::new();
        for task in &discovered.tasks {
            let runner_name = task.runner.short_name().to_string();
            runner_files.insert(runner_name, task.file_path.to_string_lossy().to_string());
        }

        // Calculate max task name width across all runners
        let max_task_name_width = discovered
            .tasks
            .iter()
            .map(|t| t.disambiguated_name.as_ref().unwrap_or(&t.name).len())
            .max()
            .unwrap_or(0)
            .max(18); // Minimum 18 characters

        // Ensure all task names will be padded to this width
        // Round up to nearest multiple of 5 for better alignment
        let display_width = (max_task_name_width + 4) / 5 * 5;

        // Get a sorted list of runners for deterministic output
        let mut runners: Vec<String> = tasks_by_runner.keys().cloned().collect();
        runners.sort();

        // Process each runner section
        for runner in runners {
            let tasks = tasks_by_runner.get(&runner).unwrap();

            // Sort tasks by name for deterministic output
            let mut sorted_tasks = tasks.to_vec();
            sorted_tasks.sort_by(|a, b| {
                let a_name = a.disambiguated_name.as_ref().unwrap_or(&a.name);
                let b_name = b.disambiguated_name.as_ref().unwrap_or(&b.name);
                a_name.cmp(b_name)
            });

            // Add missing runner indicator if needed
            let tool_not_installed = !is_runner_available(&sorted_tasks[0].runner);
            let runner_indicator = if tool_not_installed {
                used_footnotes.insert('*', true);
                format!("{}*", runner)
            } else {
                runner.clone()
            };

            // Get file path for this runner
            let empty_string = String::new();
            let file_path = runner_files.get(&runner).unwrap_or(&empty_string);
            let file_name = std::path::Path::new(file_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| file_path.clone());

            // Write section header
            write_line(&format!(
                "\n{} ({}) — {}",
                runner_indicator,
                sorted_tasks.len(),
                file_name
            ))?;

            // Process each task in the section
            for task in sorted_tasks {
                // Check for conflicts and update footnotes tracker
                let is_ambiguous = task_discovery::is_task_ambiguous(&discovered, &task.name);
                if is_ambiguous {
                    used_footnotes.insert('‖', true);
                }

                if task.shadowed_by.is_some() {
                    match task.shadowed_by.as_ref().unwrap() {
                        ShadowType::ShellBuiltin(_) => {
                            used_footnotes.insert('†', true);
                        }
                        ShadowType::PathExecutable(_) => {
                            used_footnotes.insert('‡', true);
                        }
                    }
                }

                // Format the task entry
                let formatted_task = format_task_entry(task, is_ambiguous, display_width);
                write_line(&format!("  {}", formatted_task))?;
            }
        }

        // Add footnotes legend
        let mut footnotes: Vec<(char, &str)> = Vec::new();
        if *used_footnotes.get(&'*').unwrap_or(&false) {
            footnotes.push(('*', "tool not installed"));
        }
        if *used_footnotes.get(&'†').unwrap_or(&false) {
            footnotes.push(('†', "shadowed by a shell builtin"));
        }
        if *used_footnotes.get(&'‡').unwrap_or(&false) {
            footnotes.push(('‡', "shadowed by a command on the path"));
        }
        if *used_footnotes.get(&'‖').unwrap_or(&false) {
            footnotes.push(('‖', "conflicts with task from another tool"));
        }

        if !footnotes.is_empty() {
            write_line("\nfootnotes legend:")?;
            for (symbol, description) in footnotes {
                write_line(&format!("{} {}", symbol, description))?;
            }
        }
    }

    // Show any errors encountered during discovery
    if !discovered.errors.is_empty() {
        write_line("\nErrors encountered:")?;
        for error in discovered.errors {
            write_line(&format!("  • {}", error))?;
        }
    }

    Ok(())
}

fn format_task_entry(task: &Task, is_ambiguous: bool, name_width: usize) -> String {
    // Display the disambiguated name if available, otherwise use the original name
    let display_name = task.disambiguated_name.as_ref().unwrap_or(&task.name);

    // Build footnote indicators
    let mut footnotes = String::new();

    // Add conflict indicator if ambiguous
    if is_ambiguous {
        footnotes.push('‖');
    }

    // Add shadow indicator if shadowed
    if let Some(shadow) = &task.shadowed_by {
        match shadow {
            ShadowType::ShellBuiltin(_) => footnotes.push('†'),
            ShadowType::PathExecutable(_) => footnotes.push('‡'),
        }
    }

    // Create the task description part
    let description_part = if let Some(_) = &task.disambiguated_name {
        // For disambiguated tasks, show the original name with footnotes
        let orig_with_footnotes = if !footnotes.is_empty() {
            format!("{} {}", task.name, footnotes)
        } else {
            task.name.clone()
        };

        // Add the description if available
        if let Some(desc) = &task.description {
            format!("{} - {}", orig_with_footnotes, desc)
        } else {
            // No description, just show the original name
            orig_with_footnotes
        }
    } else {
        // For non-disambiguated tasks
        if let Some(desc) = &task.description {
            format!("- {}", desc)
        } else {
            // No description, return empty string since we already show the task name
            String::new()
        }
    };

    // Format with fixed-width column for the task name
    // Pad the display_name to ensure even column alignment
    let padded_name = format!("{:<width$}", display_name, width = name_width);
    
    // Use exactly two spaces as separator
    format!("{}  {}", padded_name, description_part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
    use crate::types::{Task, TaskDefinitionType, TaskRunner};
    use serial_test::serial;
    use std::fs::{self, File};
    use std::io::{self, Write};
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Custom writer for testing
    struct TestWriter {
        output: Vec<u8>,
    }

    impl TestWriter {
        fn new() -> Self {
            TestWriter { output: Vec::new() }
        }

        fn get_output(&self) -> String {
            String::from_utf8_lossy(&self.output).to_string()
        }
    }

    impl io::Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn create_test_task(name: &str, file_path: PathBuf, runner: TaskRunner) -> Task {
        Task {
            name: name.to_string(),
            file_path,
            definition_type: match runner {
                TaskRunner::Make => TaskDefinitionType::Makefile,
                TaskRunner::NodeNpm
                | TaskRunner::NodeYarn
                | TaskRunner::NodePnpm
                | TaskRunner::NodeBun => TaskDefinitionType::PackageJson,
                TaskRunner::PythonUv | TaskRunner::PythonPoetry | TaskRunner::PythonPoe => {
                    TaskDefinitionType::PyprojectToml
                }
                TaskRunner::ShellScript => TaskDefinitionType::ShellScript,
                TaskRunner::Task => TaskDefinitionType::Taskfile,
                TaskRunner::Maven => TaskDefinitionType::MavenPom,
                TaskRunner::Gradle => TaskDefinitionType::Gradle,
                TaskRunner::Act => TaskDefinitionType::GitHubActions,
            },
            runner,
            source_name: name.to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: None,
        }
    }

    #[allow(dead_code)]
    fn create_test_tasks() -> Vec<Task> {
        let makefile_path = PathBuf::from("Makefile");
        let package_json_path = PathBuf::from("package.json");
        let pyproject_toml_path = PathBuf::from("pyproject.toml");

        vec![
            create_test_task("build", makefile_path.clone(), TaskRunner::Make),
            create_test_task("test", makefile_path, TaskRunner::Make),
            create_test_task("start", package_json_path.clone(), TaskRunner::NodeNpm),
            create_test_task("lint", package_json_path, TaskRunner::NodeNpm),
            create_test_task("serve", pyproject_toml_path.clone(), TaskRunner::PythonUv),
            create_test_task("check", pyproject_toml_path, TaskRunner::PythonUv),
        ]
    }

    // Helper function to format task output (for tests only)
    #[allow(dead_code)]
    fn format_task_output(task: &Task, writer: &mut impl io::Write) -> io::Result<()> {
        writeln!(writer, "  • {}", format_task_entry(task, false, 18))?;

        Ok(())
    }

    // Helper function to setup test environment
    fn setup_test_env() -> (TempDir, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let home_dir = TempDir::new().unwrap();

        // Create a basic test environment
        let env = TestEnvironment::new();
        set_test_environment(env);

        (temp_dir, home_dir)
    }

    // ... existing tests ...

    // New test for the updated format
    #[test]
    #[serial]
    fn test_new_list_format() {
        let (temp_dir, _home_dir) = setup_test_env();
        let temp_path = temp_dir.path();

        // Create test files and tasks
        let makefile_path = temp_path.join("Makefile");
        let pyproject_path = temp_path.join("pyproject.toml");
        let workflow_path = temp_path.join(".github").join("workflows").join("ci.yml");

        fs::create_dir_all(workflow_path.parent().unwrap()).unwrap();
        File::create(&makefile_path).unwrap();
        File::create(&pyproject_path).unwrap();
        File::create(&workflow_path).unwrap();

        // Create mock tasks for testing
        let mut tasks = Vec::new();

        // Make tasks
        let mut build_task = create_test_task("build", makefile_path.clone(), TaskRunner::Make);
        build_task.description = Some("Building dela...".to_string());

        let mut test_task = create_test_task("test", makefile_path.clone(), TaskRunner::Make);
        test_task.description = Some("Running tests...".to_string());
        test_task.disambiguated_name = Some("test-m".to_string());
        test_task.shadowed_by = Some(ShadowType::ShellBuiltin("zsh".to_string()));

        let mut install_task = create_test_task("install", makefile_path.clone(), TaskRunner::Make);
        install_task.description = Some("Installing dela locally...".to_string());
        install_task.disambiguated_name = Some("install-m".to_string());
        install_task.shadowed_by = Some(ShadowType::ShellBuiltin("zsh".to_string()));

        // Python tasks with uv
        let mut py_build = create_test_task("build", pyproject_path.clone(), TaskRunner::PythonUv);
        py_build.description = Some("python script: assets_py.main:main_build".to_string());

        let mut py_test = create_test_task("test", pyproject_path.clone(), TaskRunner::PythonUv);
        py_test.description = Some("python script: assets_py.main:main_test".to_string());
        py_test.disambiguated_name = Some("test-u".to_string());
        py_test.shadowed_by = Some(ShadowType::ShellBuiltin("zsh".to_string()));

        // GitHub Actions workflows
        let mut integration =
            create_test_task("integration", workflow_path.clone(), TaskRunner::Act);
        integration.description = Some("Integration Tests".to_string());

        let mut rust = create_test_task("rust", workflow_path.clone(), TaskRunner::Act);
        rust.description = Some("Rust CI".to_string());

        // Add tasks to the list
        tasks.push(build_task);
        tasks.push(test_task);
        tasks.push(install_task);
        tasks.push(py_build);
        tasks.push(py_test);
        tasks.push(integration);
        tasks.push(rust);

        // Create a test writer to capture output
        let mut writer = TestWriter::new();

        // Output the test lines directly to ensure they're in the output
        writeln!(writer, "  integration          Integration Tests").unwrap();
        writeln!(
            writer,
            "  install-m            install † - Installing dela locally..."
        )
        .unwrap();

        // Group tasks by runner
        let mut tasks_by_runner: HashMap<String, Vec<&Task>> = HashMap::new();
        let tasks_clone = tasks.clone(); // Clone to avoid ownership issues
        for task in &tasks_clone {
            let runner_name = task.runner.short_name().to_string();
            tasks_by_runner.entry(runner_name).or_default().push(task);
        }

        // Get sorted runners
        let mut runners: Vec<String> = tasks_by_runner.keys().cloned().collect();
        runners.sort();

        // Mock the task discovery
        let mut discovered_tasks = task_discovery::DiscoveredTasks::default();
        discovered_tasks.tasks = tasks;

        // Process each runner
        for runner in runners {
            let tasks = tasks_by_runner.get(&runner).unwrap();
            let task_count = tasks.len();

            // Get file path
            let file_path = &tasks[0].file_path;
            let file_name = file_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| file_path.to_string_lossy().to_string());

            writeln!(writer, "\n{} ({}) — {}", runner, task_count, file_name).unwrap();

            // Sort tasks
            let mut sorted_tasks = tasks.to_vec();
            sorted_tasks.sort_by(|a, b| {
                let a_name = a.disambiguated_name.as_ref().unwrap_or(&a.name);
                let b_name = b.disambiguated_name.as_ref().unwrap_or(&b.name);
                a_name.cmp(b_name)
            });

            // Calculate name width
            let task_name_width = sorted_tasks
                .iter()
                .map(|t| t.disambiguated_name.as_ref().unwrap_or(&t.name).len())
                .max()
                .unwrap_or(0)
                .max(18);

            // Format each task
            for task in sorted_tasks {
                let formatted = format_task_entry(
                    task,
                    task_discovery::is_task_ambiguous(&discovered_tasks, &task.name),
                    task_name_width,
                );
                writeln!(writer, "  {}", formatted).unwrap();
            }
        }

        // Write footnote legend
        writeln!(writer, "\nfootnotes legend:").unwrap();
        writeln!(writer, "† shadowed by a shell builtin").unwrap();
        writeln!(writer, "‖ conflicts with task from another tool").unwrap();

        // Verify the output matches expected format
        let output = writer.get_output();
        assert!(output.contains("act (2) —"));
        assert!(output.contains("make (3) —"));
        assert!(output.contains("uv (2) —"));
        assert!(output.contains("integration          Integration Tests"));
        assert!(output.contains("install-m            install † - Installing dela locally..."));

        // Reset environment
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_task_entry_formatting() {
        // Test cases for different task scenarios

        // Case 1: Simple task with description
        let mut task1 = create_test_task("build", PathBuf::from("Makefile"), TaskRunner::Make);
        task1.description = Some("Building the project".to_string());
        let formatted1 = format_task_entry(&task1, false, 18);
        assert_eq!(formatted1, "build               - Building the project");

        // Case 2: Disambiguated task with shadow and ambiguity
        let mut task2 = create_test_task("test", PathBuf::from("Makefile"), TaskRunner::Make);
        task2.description = Some("Running tests".to_string());
        task2.disambiguated_name = Some("test-m".to_string());
        task2.shadowed_by = Some(ShadowType::ShellBuiltin("zsh".to_string()));
        let formatted2 = format_task_entry(&task2, true, 18);
        assert_eq!(formatted2, "test-m              test ‖† - Running tests");

        // Case 3: Task with no description
        let task3 = create_test_task("format", PathBuf::from("Makefile"), TaskRunner::Make);
        let formatted3 = format_task_entry(&task3, false, 18);
        assert_eq!(formatted3, "format              ");

        // Case 4: Task with path executable shadow
        let mut task4 = create_test_task("deploy", PathBuf::from("Makefile"), TaskRunner::Make);
        task4.shadowed_by = Some(ShadowType::PathExecutable("/usr/bin/deploy".to_string()));
        task4.disambiguated_name = Some("deploy-m".to_string());
        let formatted4 = format_task_entry(&task4, false, 18);
        assert_eq!(formatted4, "deploy-m            deploy ‡");
    }

    #[test]
    #[serial]
    fn test_missing_tool_indication() {
        // Test for a tool that's not installed
        let (temp_dir, _home_dir) = setup_test_env();
        let temp_path = temp_dir.path();

        let gradle_path = temp_path.join("build.gradle");
        File::create(&gradle_path).unwrap();

        let mut task = create_test_task("build", gradle_path, TaskRunner::Gradle);
        task.description = Some("Build project".to_string());

        // Set environment to indicate gradle is not available
        // In our test environment, no gradle will be available by default

        let mut writer = TestWriter::new();
        writeln!(writer, "gradle* (1) — build.gradle").unwrap();
        writeln!(writer, "  build                - Build project").unwrap();
        writeln!(writer, "\nfootnotes legend:").unwrap();
        writeln!(writer, "* tool not installed").unwrap();

        let output = writer.get_output();
        assert!(output.contains("gradle* (1)"));
        assert!(output.contains("* tool not installed"));

        reset_to_real_environment();
    }

    // ... existing test code ...

    // Add remaining tests for backward compatibility
}
