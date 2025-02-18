use crate::runner::is_runner_available;
use crate::task_discovery;
use crate::task_shadowing::ShadowType;
use crate::types::{Task, TaskFileStatus};
use std::collections::HashMap;
use std::env;

pub fn execute(verbose: bool) -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);

    // Only show task definition files status in verbose mode
    if verbose {
        println!("Task definition files:");
        if let Some(makefile) = &discovered.definitions.makefile {
            match &makefile.status {
                TaskFileStatus::Parsed => println!("  ✓ Makefile: Found and parsed"),
                TaskFileStatus::NotImplemented => {
                    println!("  ! Makefile: Found but parsing not yet implemented")
                }
                TaskFileStatus::ParseError(e) => println!("  ✗ Makefile: Error parsing: {}", e),
                TaskFileStatus::NotReadable(e) => println!("  ✗ Makefile: Not readable: {}", e),
                TaskFileStatus::NotFound => println!("  - Makefile: Not found"),
            }
        }
        if let Some(package_json) = &discovered.definitions.package_json {
            match &package_json.status {
                TaskFileStatus::Parsed => println!("  ✓ package.json: Found and parsed"),
                TaskFileStatus::NotImplemented => {
                    println!("  ! package.json: Found but parsing not yet implemented")
                }
                TaskFileStatus::ParseError(e) => println!("  ✗ package.json: Error parsing: {}", e),
                TaskFileStatus::NotReadable(e) => println!("  ✗ package.json: Not readable: {}", e),
                TaskFileStatus::NotFound => println!("  - package.json: Not found"),
            }
        }
        if let Some(pyproject_toml) = &discovered.definitions.pyproject_toml {
            match &pyproject_toml.status {
                TaskFileStatus::Parsed => println!("  ✓ pyproject.toml: Found and parsed"),
                TaskFileStatus::NotImplemented => {
                    println!("  ! pyproject.toml: Found but parsing not yet implemented")
                }
                TaskFileStatus::ParseError(e) => {
                    println!("  ✗ pyproject.toml: Error parsing: {}", e)
                }
                TaskFileStatus::NotReadable(e) => {
                    println!("  ✗ pyproject.toml: Not readable: {}", e)
                }
                TaskFileStatus::NotFound => println!("  - pyproject.toml: Not found"),
            }
        }
        println!();
    }

    // Group tasks by file for better organization
    let mut tasks_by_file: HashMap<String, Vec<&Task>> = HashMap::new();
    for task in &discovered.tasks {
        let file_path = task.file_path.to_string_lossy().to_string();
        tasks_by_file.entry(file_path).or_default().push(task);
    }

    // Print tasks grouped by file
    if tasks_by_file.is_empty() {
        println!("No tasks found in the current directory.");
    } else {
        // Collect all shadow info for footer
        let mut shadow_infos = Vec::new();

        for (file, tasks) in tasks_by_file {
            println!("\nTasks from {}:", file);
            for task in tasks {
                let mut output = String::new();

                // Basic task name
                output.push_str(&format!("  • {}", task.name));

                // Add runner info and availability
                if !is_runner_available(&task.runner) {
                    output.push_str(&format!(" (requires {}, not found)", task.runner.name()));
                }

                // Add shadow symbol if present
                if let Some(ref shadow_type) = task.shadowed_by {
                    match shadow_type {
                        ShadowType::ShellBuiltin(_) => output.push_str(" †"),
                        ShadowType::PathExecutable(_) => output.push_str(" ‡"),
                    }
                    if let Some(info) = format_shadow_info(task) {
                        shadow_infos.push(info);
                    }
                }

                // Add source name if different from task name
                if task.name != task.source_name {
                    output.push_str(&format!(" (source: {})", task.source_name));
                }

                println!("{}", output);
            }
        }

        // Print shadow info footer
        if !shadow_infos.is_empty() {
            println!("\nShadowed tasks:");
            for info in shadow_infos {
                println!("  {}", info);
            }
        }
    }

    // Show any errors encountered during discovery
    if !discovered.errors.is_empty() {
        println!("\nErrors encountered:");
        for error in discovered.errors {
            println!("  • {}", error);
        }
    }

    Ok(())
}

fn format_shadow_info(task: &Task) -> Option<String> {
    task.shadowed_by
        .as_ref()
        .map(|shadow_type| match shadow_type {
            ShadowType::ShellBuiltin(name) => format!("† task '{}' shadowed by {} shell builtin", task.name, name),
            ShadowType::PathExecutable(path) => format!("‡ task '{}' shadowed by executable at {}", task.name, path),
        })
}

impl Task {
    // Removing unused method
}

#[cfg(test)]
mod tests {
    use super::*;
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
                TaskRunner::PythonUv | TaskRunner::PythonPoetry => TaskDefinitionType::PyprojectToml,
                TaskRunner::ShellScript => TaskDefinitionType::ShellScript,
            },
            runner,
            source_name: name.to_string(),
            description: None,
            shadowed_by: None,
        }
    }

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

    // Helper function to format task output
    fn format_task_output(task: &Task, writer: &mut impl io::Write) -> io::Result<()> {
        let shadow_symbol = if task.shadowed_by.is_some() {
            match task.shadowed_by.as_ref().unwrap() {
                ShadowType::ShellBuiltin(_) => " †",
                ShadowType::PathExecutable(_) => " ‡",
            }
        } else {
            ""
        };

        if let Some(desc) = &task.description {
            writeln!(writer, "  • {}{} - {}", task.name, shadow_symbol, desc)
        } else {
            writeln!(writer, "  • {}{}", task.name, shadow_symbol)
        }
    }

    // Helper function to format shadow info
    fn format_shadow_info(task: &Task) -> Option<String> {
        task.shadowed_by
            .as_ref()
            .map(|shadow_type| match shadow_type {
                ShadowType::ShellBuiltin(shell) => {
                    format!("† task '{}' shadowed by {} shell builtin", task.name, shell)
                }
                ShadowType::PathExecutable(path) => {
                    format!("‡ task '{}' shadowed by executable at {}", task.name, path)
                }
            })
    }

    // Helper function to group tasks by file
    fn tasks_by_file(tasks: &[Task]) -> HashMap<String, Vec<&Task>> {
        let mut tasks_by_file: HashMap<String, Vec<&Task>> = HashMap::new();
        for task in tasks {
            tasks_by_file
                .entry(task.file_path.display().to_string())
                .or_default()
                .push(task);
        }
        tasks_by_file
    }

    fn setup_test_env() -> (TempDir, TempDir) {
        // Create a temp dir for the project
        let project_dir = TempDir::new().expect("Failed to create temp directory");

        // Create a temp dir for HOME and set it up
        let home_dir = TempDir::new().expect("Failed to create temp HOME directory");
        env::set_var("HOME", home_dir.path());

        // Create ~/.dela directory
        fs::create_dir_all(home_dir.path().join(".dela"))
            .expect("Failed to create .dela directory");

        (project_dir, home_dir)
    }

    #[test]
    #[serial]
    fn test_list_empty_directory() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        let result = execute(false);
        assert!(result.is_ok());

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_list_with_task_files() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        let makefile_path = PathBuf::from("Makefile");
        let package_json_path = PathBuf::from("package.json");
        let pyproject_toml_path = PathBuf::from("pyproject.toml");

        let _tasks = vec![
            create_test_task("build", makefile_path.clone(), TaskRunner::Make),
            create_test_task("test", makefile_path, TaskRunner::Make),
            create_test_task("start", package_json_path.clone(), TaskRunner::NodeNpm),
            create_test_task("lint", package_json_path, TaskRunner::NodeNpm),
            create_test_task("serve", pyproject_toml_path.clone(), TaskRunner::PythonUv),
            create_test_task("check", pyproject_toml_path, TaskRunner::PythonUv),
        ];

        let result = execute(false);
        assert!(result.is_ok());

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_list_with_invalid_makefile() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Create an invalid Makefile
        let makefile_content = "invalid makefile content";
        let mut makefile =
            File::create(project_dir.path().join("Makefile")).expect("Failed to create Makefile");
        makefile
            .write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        let result = execute(false);
        assert!(result.is_ok());

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_list_verbose_mode() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Create a test Makefile
        let makefile_content = "
build: ## Building the project
\t@echo Building...

test: ## Running tests
\t@echo Testing...
";
        let mut makefile =
            File::create(project_dir.path().join("Makefile")).expect("Failed to create Makefile");
        makefile
            .write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        // Test verbose output
        let result = execute(true);
        assert!(result.is_ok());

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_list_non_verbose_mode() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Create a test Makefile
        let makefile_content = "
build: ## Building the project
\t@echo Building...

test: ## Running tests
\t@echo Testing...
";
        let mut makefile =
            File::create(project_dir.path().join("Makefile")).expect("Failed to create Makefile");
        makefile
            .write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        // Test non-verbose output
        let result = execute(false);
        assert!(result.is_ok());

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_list_with_shadowed_tasks_direct() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Create a Makefile with tasks that will be shadowed
        let makefile_content = r#"
cd: ## Change directory
    @echo "This shadows the cd builtin"

ls: ## List files
    @echo "This shadows the ls command"

custom: ## Custom command
    @echo "This shadows a PATH executable"
"#;
        let mut file =
            File::create(project_dir.path().join("Makefile")).expect("Failed to create Makefile");
        file.write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        // Discover tasks and manually set shadow information
        let mut discovered = task_discovery::discover_tasks(project_dir.path());

        // Set shadow information for each task
        for task in &mut discovered.tasks {
            match task.name.as_str() {
                "cd" => task.shadowed_by = Some(ShadowType::ShellBuiltin("zsh".to_string())),
                "ls" => task.shadowed_by = Some(ShadowType::ShellBuiltin("zsh".to_string())),
                "custom" => {
                    task.shadowed_by =
                        Some(ShadowType::PathExecutable("/usr/bin/custom".to_string()))
                }
                _ => {}
            }
        }

        let mut writer = TestWriter::new();

        // Print tasks
        writeln!(writer, "Available tasks:").unwrap();
        for (file, tasks) in tasks_by_file(&discovered.tasks) {
            writeln!(writer, "\nFrom {}:", file).unwrap();
            for task in tasks {
                format_task_output(task, &mut writer).unwrap();
            }
        }

        // Print shadow information footer
        let shadow_infos: Vec<_> = discovered
            .tasks
            .iter()
            .filter_map(format_shadow_info)
            .collect();

        if !shadow_infos.is_empty() {
            writeln!(writer, "\nShadowed tasks:").unwrap();
            for info in shadow_infos {
                writeln!(writer, "  {}", info).unwrap();
            }
        }

        let output = writer.get_output();

        // Verify shell builtin shadowing
        assert!(
            output.contains("cd †"),
            "Output missing cd task with shell builtin symbol"
        );
        assert!(
            output.contains("ls †"),
            "Output missing ls task with shell builtin symbol"
        );
        assert!(
            output.contains("† task 'cd' shadowed by zsh shell builtin"),
            "Output missing cd shell builtin shadow info"
        );
        assert!(
            output.contains("† task 'ls' shadowed by zsh shell builtin"),
            "Output missing ls shell builtin shadow info"
        );

        // Verify PATH executable shadowing
        assert!(
            output.contains("custom ‡"),
            "Output missing custom task with executable symbol"
        );
        assert!(
            output.contains("‡ task 'custom' shadowed by executable at /usr/bin/custom"),
            "Output missing custom executable shadow info"
        );

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    fn test_shadow_formatting() {
        let mut writer = TestWriter::new();

        // Create test tasks with different shadow types
        let mut tasks = vec![
            create_test_task("test1", PathBuf::from("Makefile"), TaskRunner::Make),
            create_test_task("test2", PathBuf::from("Makefile"), TaskRunner::Make),
            create_test_task("test3", PathBuf::from("Makefile"), TaskRunner::Make),
        ];

        // Add shadow information
        tasks[1].shadowed_by = Some(ShadowType::ShellBuiltin("bash".to_string()));
        tasks[2].shadowed_by = Some(ShadowType::PathExecutable("/usr/bin/test3".to_string()));

        // Print tasks
        for (file, file_tasks) in tasks_by_file(&tasks) {
            writeln!(writer, "\nFrom {}:", file).unwrap();
            for task in file_tasks {
                format_task_output(task, &mut writer).unwrap();
            }
        }

        // Print shadow information
        let shadow_infos: Vec<_> = tasks.iter().filter_map(format_shadow_info).collect();

        if !shadow_infos.is_empty() {
            writeln!(writer, "\nShadowed tasks:").unwrap();
            for info in shadow_infos {
                writeln!(writer, "  {}", info).unwrap();
            }
        }

        let output = writer.get_output();

        // Verify task listing format
        assert!(output.contains("• test1"), "Missing unshadowed task");
        assert!(
            output.contains("• test2 †"),
            "Incorrect shell builtin format"
        );
        assert!(output.contains("• test3 ‡"), "Incorrect executable format");

        // Verify shadow information format
        assert!(
            output.contains("† task 'test2' shadowed by bash shell builtin"),
            "Incorrect shell builtin info"
        );
        assert!(
            output.contains("‡ task 'test3' shadowed by executable at /usr/bin/test3"),
            "Incorrect executable info"
        );
    }
}
