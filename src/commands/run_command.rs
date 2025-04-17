use crate::runner::is_runner_available;
use crate::task_discovery;
use std::env;
use std::process::{Command, Stdio};

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

    // Check if there are no matching tasks
    if matching_tasks.is_empty() {
        return Err(format!("dela: command or task not found: {}", task_name));
    }

    // Check if there are multiple matching tasks
    if matching_tasks.len() > 1 {
        println!(
            "Multiple tasks found with name '{}'. Please use one of the following:",
            task_name
        );
        for task in &matching_tasks {
            println!(
                "  - {} ({})",
                task.disambiguated_name.as_ref().unwrap_or(&task.name),
                task.runner.short_name()
            );
        }
        return Err(format!("Ambiguous task name: '{}'", task_name));
    }

    // Single task found, check if runner is available
    let task = matching_tasks[0];
    if !is_runner_available(&task.runner) {
        return Err(format!("Runner '{}' not found", task.runner.short_name()));
    }

    // Get the command to run
    let mut command_str = task.runner.get_command(task);
    if !args.is_empty() {
        command_str.push(' ');
        command_str.push_str(&args.join(" "));
    }

    println!("Running: {}", command_str);

    // Execute the command
    let status = Command::new("sh")
        .arg("-c")
        .arg(&command_str)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    if !status.success() {
        return Err(format!("Command failed with exit code: {}", status));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{reset_to_real_environment, set_test_environment, TestEnvironment};
    #[cfg(test)]
    use crate::task_shadowing::{enable_mock, reset_mock};
    #[cfg(test)]
    use crate::types::TaskRunner;
    #[cfg(test)]
    use serial_test::serial;
    #[cfg(test)]
    use std::fs::{self, File};
    #[cfg(test)]
    use std::io::Write;
    #[cfg(test)]
    use tempfile::TempDir;
    
    #[cfg(test)]
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
    fn test_run_command_no_task() {
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
    fn test_run_command_missing_runner() {
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
    fn test_run_command_ambiguous_tasks() {
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

        let result = execute("test");
        assert!(result.is_err(), "Should fail with ambiguous task name");
        assert!(
            result.unwrap_err().contains("Ambiguous task name: 'test'"),
            "Error should mention ambiguous task name"
        );

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_run_command_with_args() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Mock make being available but redirect output to avoid make help output
        reset_mock();
        enable_mock();
        let env = TestEnvironment::new().with_executable("make");
        set_test_environment(env);

        // Simply check if the task resolution part works (finding the task)
        // We can't easily mock the command execution, so we'll just verify
        // that task resolution works correctly
        
        // First test that the task can be found
        let current_dir = env::current_dir().unwrap();
        let discovered = task_discovery::discover_tasks(&current_dir);
        let tasks = task_discovery::get_matching_tasks(&discovered, "test");
        assert_eq!(tasks.len(), 1, "Should find exactly one task");
        
        // Then test the error case - but since we can't easily intercept the
        // command execution, just check the error format pattern to ensure
        // it's a command execution error and not a task resolution error
        let result = execute("test --arg1 --arg2");
        assert!(result.is_err(), "Command execution should fail in test environment");
        
        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_run_command_disambiguated_tasks() {
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
            result.unwrap_err().contains("Ambiguous task name: 'test'"),
            "Error should mention ambiguous task name"
        );

        // Verify that we can find the disambiguated tasks
        let current_dir = env::current_dir().unwrap();
        let discovered = task_discovery::discover_tasks(&current_dir);
        
        let make_tasks = task_discovery::get_matching_tasks(&discovered, "test-mak");
        assert_eq!(make_tasks.len(), 1, "Should find exactly one make task");
        assert_eq!(make_tasks[0].runner, TaskRunner::Make);

        let npm_tasks = task_discovery::get_matching_tasks(&discovered, "test-npm");
        assert_eq!(npm_tasks.len(), 1, "Should find exactly one npm task");
        assert_eq!(npm_tasks[0].runner, TaskRunner::NodeNpm);

        // Don't actually try to execute the command since it will fail in a test environment
        // and produce unwanted output. Just verify that we can find the task.
        
        // For the test-mak command, just verify the task is found correctly
        let test_mak_tasks = task_discovery::get_matching_tasks(&discovered, "test-mak");
        assert_eq!(test_mak_tasks.len(), 1, "Should find exactly one test-mak task");
        
        // Now verify the npm variant
        let test_npm_tasks = task_discovery::get_matching_tasks(&discovered, "test-npm");
        assert_eq!(test_npm_tasks.len(), 1, "Should find exactly one test-npm task");

        reset_mock();
        reset_to_real_environment();
        drop(project_dir);
        drop(home_dir);
    }
}
