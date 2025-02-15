use std::env;
use crate::task_discovery;
use crate::allowlist;

pub fn execute(task: &str) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);
    
    // Find all tasks with the given name
    let matching_tasks: Vec<_> = discovered.tasks
        .iter()
        .filter(|t| t.name == task)
        .collect();

    match matching_tasks.len() {
        0 => {
            println!("No task named '{}' found in the current directory.", task);
            Err(format!("No task named '{}' found", task))
        }
        1 => {
            // Single task found, check allowlist
            let task = matching_tasks[0];
            if !allowlist::check_task_allowed(task)? {
                return Err(format!("Dela task '{}' was denied by the ~/.dela/allowlist.toml", task.name));
            }
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
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;
    use serial_test::serial;

    // Test helper to simulate user input (copied from prompt.rs)
    fn with_stdin<F>(input: &str, test: F) where F: FnOnce() {
        use std::io::Write;
        use std::fs::File;
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
        let mut makefile = File::create(project_dir.path().join("Makefile"))
            .expect("Failed to create Makefile");
        makefile.write_all(makefile_content.as_bytes())
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
    fn test_allow_command_single_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Simulate user allowing the task
        with_stdin("1\n", || {
            let result = execute("test");
            assert!(result.is_ok(), "Should succeed for a single task");
        });

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_allow_command_denied_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        // Simulate user denying the task
        with_stdin("5\n", || {
            let result = execute("test");
            assert!(result.is_err(), "Should fail when task is denied");
            assert_eq!(result.unwrap_err(), "Task 'test' was denied");
        });

        drop(project_dir);
        drop(home_dir);
    }

    #[test]
    #[serial]
    fn test_allow_command_no_task() {
        let (project_dir, home_dir) = setup_test_env();
        env::set_current_dir(&project_dir).expect("Failed to change directory");

        let result = execute("nonexistent");
        assert!(result.is_err(), "Should fail when no task found");
        assert_eq!(
            result.unwrap_err(),
            "No task named 'nonexistent' found"
        );

        drop(project_dir);
        drop(home_dir);
    }
} 