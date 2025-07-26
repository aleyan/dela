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
                if task.runner == crate::types::TaskRunner::TravisCi {
                    return Err("Travis CI tasks cannot be executed locally - they are only available for discovery".to_string());
                }
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
            // Multiple matches (should not happen with get_matching_tasks, but handle for safety)
            let error_msg = task_discovery::format_ambiguous_task_error(task_name, &matching_tasks);
            Err(error_msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
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
        unsafe {
            env::set_var("HOME", home_dir.path());
        }

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

        // Test with the execute function
        let result = execute("test --verbose --coverage");

        // Verify the command was executed successfully
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

    #[test]
    #[serial]
    fn test_get_command_disambiguated_tasks() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Create a package.json with the same task name
        let package_json_content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest"
            }
        }"#;

        File::create(project_dir.path().join("package.json"))
            .unwrap()
            .write_all(package_json_content.as_bytes())
            .unwrap();

        // Create package-lock.json to ensure npm is detected
        File::create(project_dir.path().join("package-lock.json"))
            .unwrap()
            .write_all(b"{}")
            .unwrap();

        // Mock both make and npm being available
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new()
            .with_executable("make")
            .with_executable("npm");
        set_test_environment(env);

        // First verify that ambiguous task gives error
        let result = execute("test");
        assert!(result.is_err(), "Should fail with ambiguous task name");
        assert!(
            result
                .unwrap_err()
                .contains("Multiple tasks named 'test' found"),
            "Error should mention multiple tasks"
        );

        // Verify task lookup for make variant works
        let result = execute("test-m");
        assert!(
            result.is_ok(),
            "Should succeed with disambiguated task name (make)"
        );

        // Verify task lookup for npm variant works
        let result = execute("test-n");
        assert!(
            result.is_ok(),
            "Should succeed with disambiguated task name (npm)"
        );

        // Verify arguments are correctly passed with disambiguated names
        let result = execute("test-m --verbose");
        assert!(
            result.is_ok(),
            "Should succeed with disambiguated task name and args"
        );

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }
}
