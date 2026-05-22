use crate::allowlist;
use crate::task_discovery;
use crate::types::AllowScope;
use std::env;

/// Executes the 'dela allow' command to add a specific task to the allowlist.
pub fn execute(task_name: &str) -> anyhow::Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);

    // Find all tasks with the given name (both original and disambiguated)
    let matching_tasks = task_discovery::get_matching_tasks(&discovered, task_name);

    match matching_tasks.len() {
        0 => Err(anyhow::anyhow!(
            "dela: command or task not found: {}",
            task_name
        )),
        1 => {
            let task = matching_tasks[0];
            allowlist::check_task_allowed_with_scope(task, AllowScope::Task)?;
            println!(
                "Added task '{}' ({}) to allowlist.",
                task.name,
                task.runner.short_name()
            );
            Ok(())
        }
        _ => {
            let error_msg = task_discovery::format_ambiguous_task_error(task_name, &matching_tasks);
            eprintln!("{}", error_msg);
            Err(anyhow::anyhow!(
                "Multiple tasks named '{}' found",
                task_name
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{preferred_allowlist_path_for, preferred_config_dir_path_for};
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use serial_test::serial;
    use std::env;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    struct TestEnvGuard {
        project_dir: TempDir,
        home_dir: TempDir,
        original_cwd: std::path::PathBuf,
    }

    impl TestEnvGuard {
        fn new() -> Self {
            let original_cwd = env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
            let (project_dir, home_dir) = setup_test_env();
            Self {
                project_dir,
                home_dir,
                original_cwd,
            }
        }

        fn new_uninitialized() -> Self {
            let original_cwd = env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
            // Create a temp dir for HOME but don't create the dela config directory
            let home_dir = TempDir::new().expect("Failed to create temp HOME directory");

            // Set up test environment with the temp directory as HOME
            let test_env = TestEnvironment::new().with_home(home_dir.path().to_string_lossy());
            set_test_environment(test_env);

            Self {
                project_dir: TempDir::new().expect("Failed to create temp directory"),
                home_dir,
                original_cwd,
            }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original_cwd);
            reset_to_real_environment();
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

        // Create ~/.config/dela directory
        fs::create_dir_all(preferred_config_dir_path_for(home_dir.path()))
            .expect("Failed to create dela config directory");

        (project_dir, home_dir)
    }

    #[test]
    #[serial]
    fn test_execute_allow_single_task() {
        let guard = TestEnvGuard::new();
        env::set_current_dir(&guard.project_dir).expect("Failed to change directory");

        let result = execute("test");
        assert!(result.is_ok(), "Should succeed for a single task");

        // Verify it was added to the allowlist
        let allowlist_content =
            fs::read_to_string(preferred_allowlist_path_for(guard.home_dir.path())).unwrap();
        assert!(allowlist_content.contains("test"));
        assert!(allowlist_content.contains("scope = \"Task\""));
    }

    #[test]
    #[serial]
    fn test_execute_allow_no_task() {
        let guard = TestEnvGuard::new();
        env::set_current_dir(&guard.project_dir).expect("Failed to change directory");

        let result = execute("nonexistent");
        assert!(result.is_err(), "Should fail for nonexistent task");
        assert_eq!(
            result.unwrap_err().to_string(),
            "dela: command or task not found: nonexistent"
        );
    }

    #[test]
    #[serial]
    fn test_execute_allow_uninitialized() {
        let guard = TestEnvGuard::new_uninitialized();
        env::set_current_dir(&guard.project_dir).expect("Failed to change directory");

        // Create a test Makefile
        let makefile_content = "
test: ## Running tests
\t@echo Testing...
";
        let mut makefile = File::create(guard.project_dir.path().join("Makefile"))
            .expect("Failed to create Makefile");
        makefile
            .write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        let result = execute("test");
        assert!(result.is_err(), "Should fail when dela is not initialized");
        assert_eq!(
            result.unwrap_err().to_string(),
            "Dela is not initialized. Please run 'dela init' first."
        );
    }

    #[test]
    #[serial]
    fn test_execute_allow_disambiguated() {
        let guard = TestEnvGuard::new();
        env::set_current_dir(&guard.project_dir).expect("Failed to change directory");

        // Create a package.json with the same task name
        let package_json_content = r#"{
            "name": "test-package",
            "scripts": {
                "test": "jest"
            }
        }"#;

        File::create(guard.project_dir.path().join("package.json"))
            .unwrap()
            .write_all(package_json_content.as_bytes())
            .unwrap();

        // Create package-lock.json to ensure npm is detected
        File::create(guard.project_dir.path().join("package-lock.json"))
            .unwrap()
            .write_all(b"{}")
            .unwrap();

        // Ambiguous task name 'test' should fail
        let result = execute("test");
        assert!(result.is_err(), "Should fail for ambiguous task name");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Multiple tasks named 'test' found")
        );

        // Suffixed/disambiguated task name should succeed
        let result = execute("test-m");
        assert!(result.is_ok(), "Should succeed with disambiguated name");

        let allowlist_content =
            fs::read_to_string(preferred_allowlist_path_for(guard.home_dir.path())).unwrap();
        assert!(allowlist_content.contains("test")); // Note: allowlist uses original task name
        assert!(allowlist_content.contains("scope = \"Task\""));
    }
}
