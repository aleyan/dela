use crate::runner::is_runner_available;
use crate::task_discovery;
use std::env;

pub fn execute(task_with_args: &str) -> Result<(), String> {
    let mut parts = task_with_args.split_whitespace();
    let task_name = parts
        .next()
        .ok_or_else(|| "No task name provided".to_string())?;
    let args: Vec<&str> = parts.collect();

    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);

    // Find all tasks with the given name (both original and disambiguated)
    let matching_tasks = task_discovery::get_matching_tasks(&discovered, task_name);

    match matching_tasks.len() {
        0 => Err(format!("dela: command or task not found: {}", task_name)),
        1 => {
            // Single task found, check if runner is available
            let task = matching_tasks[0];
            if !is_runner_available(&task.runner) {
                return Err(format!("Runner '{}' not found", task.runner.short_name()));
            }
            let mut command = task.runner.get_command(task);
            if !args.is_empty() {
                command.push(' ');
                command.push_str(&args.join(" "));
            }
            println!("{}", command);
            Ok(())
        }
        _ => {
            // Multiple tasks found, print error and list them
            println!("Multiple tasks named '{}' found:", task_name);
            for task in &matching_tasks {
                // Show disambiguated name if available
                let display_name = task.disambiguated_name.as_ref().unwrap_or(&task.name);
                println!("  • {} ({} from {})", display_name, task.runner.short_name(), task.file_path.display());
            }
            println!(
                "Please use a disambiguated name (e.g., '{}-{}') to specify which one to run.",
                task_name, matching_tasks[0].runner.short_name()[0..1].to_string()
            );
            Err(format!("Multiple tasks named '{}' found", task_name))
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
    fn test_get_command_with_args() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Mock make being available
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("make");
        set_test_environment(env);

        let result = execute("test --verbose --coverage");
        assert!(result.is_ok(), "Should succeed for task with arguments");

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
        assert_eq!(
            result.unwrap_err(),
            "dela: command or task not found: nonexistent"
        );

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
