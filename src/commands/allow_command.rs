use crate::allowlist;
use crate::task_discovery;
use crate::types::AllowScope;
use std::env;

pub fn execute(task_with_args: &str, allow: Option<u8>) -> Result<(), String> {
    let task_name = task_with_args
        .split_whitespace()
        .next()
        .ok_or_else(|| "No task name provided".to_string())?;

    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);

    // Find all tasks with the given name (both original and disambiguated)
    let matching_tasks = task_discovery::get_matching_tasks(&discovered, task_name);

    match matching_tasks.len() {
        0 => Err(format!("dela: command or task not found: {}", task_name)),
        1 => {
            // Single task found, check allowlist
            let task = matching_tasks[0];

            // If allow option is provided, use it directly
            if let Some(choice) = allow {
                match choice {
                    2 => {
                        allowlist::check_task_allowed_with_scope(task, AllowScope::Task)?;
                        Ok(())
                    }
                    3 => {
                        allowlist::check_task_allowed_with_scope(task, AllowScope::File)?;
                        Ok(())
                    }
                    4 => {
                        allowlist::check_task_allowed_with_scope(task, AllowScope::Directory)?;
                        Ok(())
                    }
                    5 => {
                        eprintln!("Task '{}' was denied by the allowlist.", task.name);
                        Err(format!(
                            "Dela task '{}' was denied by the ~/.dela/allowlist.toml",
                            task.name
                        ))
                    }
                    _ => Err(format!(
                        "Invalid allow choice {}. Please use a number between 2 and 5.",
                        choice
                    )),
                }
            } else {
                // Otherwise, use the interactive prompt
                if !allowlist::check_task_allowed(task)? {
                    eprintln!("Task '{}' was denied by the allowlist.", task.name);
                    return Err(format!(
                        "Dela task '{}' was denied by the ~/.dela/allowlist.toml",
                        task.name
                    ));
                }
                Ok(())
            }
        }
        _ => {
            // Multiple tasks found, print error and list them
            let error_msg = task_discovery::format_ambiguous_task_error(task_name, &matching_tasks);
            eprintln!("{}", error_msg);
            Err(format!("Multiple tasks named '{}' found", task_name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use serial_test::serial;
    use std::env;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    // Test helper to simulate user input (copied from prompt.rs)
    fn with_stdin<F>(input: &str, test: F)
    where
        F: FnOnce(),
    {
        use std::fs::File;
        use std::io::Write;
        use std::os::unix::io::FromRawFd;

        unsafe {
            let mut pipe = [0; 2];
            libc::pipe(&mut pipe[0]);

            // Write the test input to the write end of the pipe
            let mut writer = File::from_raw_fd(pipe[1]);
            writer.write_all(input.as_bytes()).unwrap();
            drop(writer);

            // Temporarily replace stdin with the read end of the pipe
            let old_stdin = libc::dup(0);
            libc::dup2(pipe[0], 0);

            // Run the test
            test();

            // Restore the original stdin
            libc::dup2(old_stdin, 0);
            libc::close(old_stdin);
            libc::close(pipe[0]);
        }
    }

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

        // Set up test environment with the temp directory as HOME
        let test_env = TestEnvironment::new().with_home(home_dir.path().to_string_lossy());
        set_test_environment(test_env);

        // Create ~/.dela directory
        fs::create_dir_all(home_dir.path().join(".dela"))
            .expect("Failed to create .dela directory");

        (project_dir, home_dir)
    }

    #[test]
    #[serial]
    fn test_allow_command_single_task() {
        let (project_dir, _home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Simulate user allowing the task
        with_stdin("1\n", || {
            let result = execute("test", None);
            assert!(result.is_ok(), "Should succeed for a single task");
        });

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_allow_command_with_args() {
        let (project_dir, _home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Simulate user allowing the task
        with_stdin("1\n", || {
            let result = execute("test --verbose --coverage", None);
            assert!(result.is_ok(), "Should succeed for task with arguments");
        });

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_allow_command_denied_task() {
        let (project_dir, _home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Simulate user denying the task
        with_stdin("5\n", || {
            let result = execute("test", None);
            assert!(result.is_err(), "Should fail when task is denied");
            assert_eq!(
                result.unwrap_err(),
                "Dela task 'test' was denied by the ~/.dela/allowlist.toml"
            );
        });

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_allow_command_no_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        let result = execute("nonexistent", None);
        assert!(result.is_err(), "Should fail when no task found");
        assert_eq!(
            result.unwrap_err(),
            "dela: command or task not found: nonexistent"
        );

        drop(project_dir);
        drop(home_dir);
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_allow_command_with_allow_option() {
        let (project_dir, _home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Test with valid allow options
        assert!(
            execute("test", Some(2)).is_ok(),
            "Should succeed with allow=2"
        );
        assert!(
            execute("test", Some(3)).is_ok(),
            "Should succeed with allow=3"
        );
        assert!(
            execute("test", Some(4)).is_ok(),
            "Should succeed with allow=4"
        );

        // Test with deny option
        let result = execute("test", Some(5));
        assert!(result.is_err(), "Should fail with allow=5");
        assert_eq!(
            result.unwrap_err(),
            "Dela task 'test' was denied by the ~/.dela/allowlist.toml"
        );

        // Test with invalid allow option
        let result = execute("test", Some(1));
        assert!(result.is_err(), "Should fail with allow=1");
        assert_eq!(
            result.unwrap_err(),
            "Invalid allow choice 1. Please use a number between 2 and 5."
        );

        // Test with out of range allow option
        let result = execute("test", Some(6));
        assert!(result.is_err(), "Should fail with allow=6");
        assert_eq!(
            result.unwrap_err(),
            "Invalid allow choice 6. Please use a number between 2 and 5."
        );

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_allow_command_uninitialized() {
        // Create a temp dir for HOME but don't create .dela directory
        let home_dir = TempDir::new().expect("Failed to create temp HOME directory");

        // Set up test environment with the temp directory as HOME
        let test_env = TestEnvironment::new().with_home(home_dir.path().to_string_lossy());
        set_test_environment(test_env);

        // Create a temp dir for the project
        let project_dir = TempDir::new().expect("Failed to create temp directory");
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Create a test Makefile
        let makefile_content = "
test: ## Running tests
\t@echo Testing...
";
        let mut makefile =
            File::create(project_dir.path().join("Makefile")).expect("Failed to create Makefile");
        makefile
            .write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        // Try to allow a task without .dela directory
        let result = execute("test", None);
        assert!(
            result.is_err(),
            "Should fail when .dela directory doesn't exist"
        );
        assert_eq!(
            result.unwrap_err(),
            "Dela is not initialized. Please run 'dela init' first."
        );

        // Verify .dela directory wasn't created
        assert!(
            !home_dir.path().join(".dela").exists(),
            ".dela directory should not be created"
        );

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_allow_command_with_allow_option_and_args() {
        let (project_dir, _home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Test with valid allow option and task arguments
        assert!(
            execute("test --verbose --coverage", Some(2)).is_ok(),
            "Should succeed with allow=2 and arguments"
        );

        // Test with deny option and task arguments
        let result = execute("test --verbose --coverage", Some(5));
        assert!(result.is_err(), "Should fail with allow=5 and arguments");
        assert_eq!(
            result.unwrap_err(),
            "Dela task 'test' was denied by the ~/.dela/allowlist.toml"
        );

        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_allow_command_disambiguated_tasks() {
        let (project_dir, _home_dir) = setup_test_env();
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

        // Check that ambiguous task 'test' is rejected
        let result = execute("test", None);
        assert!(result.is_err(), "Should error for ambiguous task");
        assert!(
            result
                .unwrap_err()
                .contains("Multiple tasks named 'test' found"),
            "Error should mention multiple tasks"
        );

        // Now try with the disambiguated name for make task
        let result = execute("test-m", Some(2)); // 2 = allow
        assert!(
            result.is_ok(),
            "Should succeed with disambiguated task name"
        );

        // Also try with the disambiguated name for npm task
        let result = execute("test-n", Some(2)); // 2 = allow
        assert!(
            result.is_ok(),
            "Should succeed with disambiguated task name"
        );

        // Test with disambiguated name and arguments
        let test_with_args = "test-m --verbose --watch";
        let result = execute(test_with_args, Some(2)); // 2 = allow
        assert!(
            result.is_ok(),
            "Should succeed with disambiguated task name and arguments"
        );

        // Test with npm task disambiguated name and arguments
        let npm_test_with_args = "test-n --ci --coverage";
        let result = execute(npm_test_with_args, Some(2)); // 2 = allow
        assert!(
            result.is_ok(),
            "Should succeed with npm disambiguated task name and arguments"
        );

        reset_to_real_environment();
    }
}
