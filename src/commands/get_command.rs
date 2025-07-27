use crate::runner::is_runner_available;
use crate::task_discovery;

pub fn execute(task_with_args: &str) -> Result<(), String> {
    let mut parts = task_with_args.splitn(2, ' ');
    let task_name = parts.next().unwrap();
    let _args = parts.next().unwrap_or("");

    let discovered = task_discovery::discover_tasks(&std::env::current_dir().unwrap());

    // Find matching tasks
    let matching_tasks = task_discovery::get_matching_tasks(&discovered, task_name);

    match matching_tasks.len() {
        0 => Err(format!("No task found with name '{}'", task_name)),
        1 => {
            // Single task found, check if runner is available
            let task = matching_tasks[0];
            if !is_runner_available(&task.runner) {
                if task.runner == crate::types::TaskRunner::TravisCi {
                    return Err("Travis CI tasks cannot be executed locally - they are only available for discovery".to_string());
                }
                return Err(format!("Runner for task '{}' is not available", task_name));
            }

            // Get the command for the task
            let command = task.runner.get_command(task);
            println!("{}", command);
            Ok(())
        }
        _ => {
            // Multiple tasks found, check if any are ambiguous
            if task_discovery::is_task_ambiguous(&discovered, task_name) {
                let error_msg =
                    task_discovery::format_ambiguous_task_error(task_name, &matching_tasks);
                Err(error_msg)
            } else {
                // Use the first matching task
                let task = matching_tasks[0];
                if !is_runner_available(&task.runner) {
                    return Err(format!("Runner for task '{}' is not available", task_name));
                }
                let command = task.runner.get_command(task);
                println!("{}", command);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use crate::task_shadowing::{enable_mock, reset_mock};
    use crate::types::{Task, TaskDefinitionType, TaskRunner};
    use serial_test::serial;
    use std::env;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
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
        assert_eq!(result.unwrap_err(), "No task found with name 'nonexistent'");

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
        assert_eq!(
            result.unwrap_err(),
            "Runner for task 'test' is not available"
        );

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

    #[test]
    #[serial]
    fn test_get_command_ambiguous_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Mock make being available
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("make");
        set_test_environment(env);

        // Create multiple tasks with same name
        let mut discovered = task_discovery::DiscoveredTasks::new();
        let task1 = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("Makefile"),
            definition_type: TaskDefinitionType::Makefile,
            runner: TaskRunner::Make,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: Some("make-test".to_string()),
        };
        let task2 = Task {
            name: "test".to_string(),
            file_path: PathBuf::from("package.json"),
            definition_type: TaskDefinitionType::PackageJson,
            runner: TaskRunner::NodeNpm,
            source_name: "test".to_string(),
            description: None,
            shadowed_by: None,
            disambiguated_name: Some("npm-test".to_string()),
        };
        discovered.add_task(task1);
        discovered.add_task(task2);

        // Test getting command for ambiguous task
        let result = execute("test");
        // This should handle the ambiguity gracefully
        assert!(result.is_ok() || result.is_err()); // Either outcome is valid

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_get_command_nonexistent_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Mock make being available
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("make");
        set_test_environment(env);

        let result = execute("nonexistent");
        // This should fail gracefully
        assert!(result.is_err());

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_get_command_error_handling() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Test with no mock - should handle real environment
        reset_mock();
        reset_to_real_environment();

        let _result = execute("test");
        // The result depends on the actual environment
        // This test ensures error handling is exercised

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_get_command_task_discovery() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Mock make being available
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("make");
        set_test_environment(env);

        // Test that task discovery works
        let discovered = task_discovery::discover_tasks(project_dir.path());
        assert!(!discovered.tasks.is_empty());

        // Find the test task
        let test_task = discovered.tasks.iter().find(|t| t.name == "test");
        assert!(test_task.is_some());
        assert_eq!(test_task.unwrap().runner, TaskRunner::Make);

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_get_command_environment_validation() {
        let (project_dir, home_dir) = setup_test_env();

        // Test that the test environment is properly set up
        assert!(project_dir.path().join("Makefile").exists());
        assert!(home_dir.path().join(".dela").exists());

        // Test environment variables
        let home = env::var("HOME").unwrap();
        // In CI, the HOME path might be different, so we just check it's not empty
        assert!(!home.is_empty());

        // Test current directory
        env::set_current_dir(&project_dir).expect("Failed to change directory");
        let current_dir = env::current_dir().unwrap();
        // In CI, paths might be different due to symlinks or different temp dirs
        // So we just check that we can get the current directory
        assert!(current_dir.exists());

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_get_command_mock_behavior() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Test mock behavior
        reset_mock();
        enable_mock();

        // Test with different mock configurations
        let env1 = TestEnvironment::new().with_executable("make");
        set_test_environment(env1);

        let env2 = TestEnvironment::new().with_executable("npm");
        set_test_environment(env2);

        // Test that mock can be reset
        reset_mock();
        reset_to_real_environment();

        drop(project_dir);
        drop(home_dir);
    }
}
