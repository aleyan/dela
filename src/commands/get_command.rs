use std::env;
use crate::task_discovery;

pub fn execute(task: &str) -> Result<(), String> {
    // TODO(DTKT-20): Implement shell environment inheritance for task execution
    // TODO(DTKT-21): Support both direct execution and subshell spawning based on task type
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
            eprintln!("No task named '{}' found in the current directory.", task);
            Err(format!("No task named '{}' found", task))
        }
        1 => {
            // Single task found, return its command
            let task = matching_tasks[0];
            let command = task.runner.get_command(task);
            println!("{}", command);
            Ok(())
        }
        _ => {
            // Multiple tasks found, print error and list them
            eprintln!("Multiple tasks named '{}' found:", task);
            for task in matching_tasks {
                eprintln!("  • {} (from {})", task.name, task.file_path.display());
            }
            eprintln!("Please use 'dela run {}' to choose which one to run.", task);
            Err(format!("Multiple tasks named '{}' found", task))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::io::Write;

    fn setup_test_dir() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        
        // Create a test Makefile
        let makefile_content = "
build: ## Building the project
\t@echo Building...

test: ## Running tests
\t@echo Testing...
";
        let mut makefile = fs::File::create(temp_dir.path().join("Makefile"))
            .expect("Failed to create Makefile");
        makefile.write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        temp_dir
    }

    #[test]
    fn test_get_command_single_task() {
        let original_dir = env::current_dir().expect("Failed to get current directory");
        let temp_dir = setup_test_dir();
        env::set_current_dir(&temp_dir).expect("Failed to change directory");

        let result = execute("test");
        assert!(result.is_ok(), "Should succeed for a single task");

        // Keep temp_dir alive until after we restore the directory
        env::set_current_dir(&original_dir).expect("Failed to restore directory");
        drop(temp_dir);
    }

    #[test]
    fn test_get_command_no_task() {
        let original_dir = env::current_dir().expect("Failed to get current directory");
        let temp_dir = setup_test_dir();
        env::set_current_dir(&temp_dir).expect("Failed to change directory");

        let result = execute("nonexistent");
        assert!(result.is_err(), "Should fail when no task found");
        assert_eq!(
            result.unwrap_err(),
            "No task named 'nonexistent' found"
        );

        // Keep temp_dir alive until after we restore the directory
        env::set_current_dir(&original_dir).expect("Failed to restore directory");
        drop(temp_dir);
    }
} 