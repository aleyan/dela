use crate::runner::is_runner_available;
use crate::task_discovery;
use std::env;

pub fn execute(task: &str) -> Result<(), String> {
    // TODO(DTKT-20): Implement shell environment inheritance for task execution
    // TODO(DTKT-21): Support both direct execution and subshell spawning based on task type
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);

    // Find all tasks with the given name
    let matching_tasks: Vec<_> = discovered.tasks.iter().filter(|t| t.name == task).collect();

    match matching_tasks.len() {
        0 => {
            println!("No task named '{}' found in the current directory.", task);
            Err(format!("No task named '{}' found", task))
        }
        1 => {
            // Single task found, check if runner is available
            let task = matching_tasks[0];
            if !is_runner_available(&task.runner) {
                return Err(format!("Runner '{}' not found", task.runner.short_name()));
            }
            let command = task.runner.get_command(task);
            println!("{}", command);
            Ok(())
        }
        _ => {
            // Multiple tasks found, print error and list them
            println!("Multiple tasks named '{}' found:", task);
            for task in matching_tasks {
                println!("  • {} (from {})", task.name, task.file_path.display());
            }
            println!("Please use 'dela run {}' to choose which one to run.", task);
            Err(format!("Multiple tasks named '{}' found", task))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
    use crate::task_shadowing::{enable_mock, reset_mock};
    use serial_test::serial;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, TempDir) {
        // Create a temp dir for the project
        let project_dir = TempDir::new().expect("Failed to create temp directory");

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
    fn test_get_command_single_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Mock make being available
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("make");
        set_test_environment(env);

        let result = execute("test");
        assert!(result.is_ok(), "Should succeed for a single task");

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_get_command_no_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        let result = execute("nonexistent");
        assert!(result.is_err(), "Should fail when no task found");
        assert_eq!(result.unwrap_err(), "No task named 'nonexistent' found");

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_get_command_missing_runner() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Set up test environment with no executables to simulate missing make
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new();
        set_test_environment(env);

        let result = execute("test");
        assert!(result.is_err(), "Should fail when runner is missing");
        assert_eq!(result.unwrap_err(), "Runner 'make' not found");

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }
}
