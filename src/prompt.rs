use std::io::{self, Write};
use crate::types::{AllowScope, Task};

#[derive(Debug, PartialEq)]
pub enum AllowDecision {
    Allow(AllowScope),
    Deny,
}

/// Prompt the user for a decision about a task
pub fn prompt_for_task(task: &Task) -> Result<AllowDecision, String> {
    println!("\nTask '{}' from '{}' requires approval.", task.name, task.file_path.display());
    if let Some(desc) = &task.description {
        println!("Description: {}", desc);
    }
    println!("\nHow would you like to proceed?");
    println!("1) Allow once (this time only)");
    println!("2) Allow this task (remember for this task)");
    println!("3) Allow file (remember for all tasks in this file)");
    println!("4) Allow directory (remember for all tasks in this directory)");
    println!("5) Deny (don't run this task)");
    
    print!("\nEnter your choice (1-5): ");
    io::stdout().flush().map_err(|e| format!("Failed to flush stdout: {}", e))?;
    
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;
    
    match input.trim() {
        "1" => Ok(AllowDecision::Allow(AllowScope::Once)),
        "2" => Ok(AllowDecision::Allow(AllowScope::Task)),
        "3" => Ok(AllowDecision::Allow(AllowScope::File)),
        "4" => Ok(AllowDecision::Allow(AllowScope::Directory)),
        "5" => Ok(AllowDecision::Deny),
        _ => Err("Invalid choice. Please enter a number between 1 and 5.".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::types::{Task, TaskRunner};

    // Helper function to create a test task
    fn create_test_task() -> Task {
        Task {
            name: "test-task".to_string(),
            description: Some("A test task".to_string()),
            file_path: PathBuf::from("Makefile"),
            runner: TaskRunner::Make,
            source_name: "test-task".to_string(),
        }
    }

    // Test helper to simulate user input
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

    #[test]
    fn test_prompt_allow_once() {
        with_stdin("1\n", || {
            let task = create_test_task();
            let result = prompt_for_task(&task);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Once));
        });
    }

    #[test]
    fn test_prompt_allow_task() {
        with_stdin("2\n", || {
            let task = create_test_task();
            let result = prompt_for_task(&task);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Task));
        });
    }

    #[test]
    fn test_prompt_allow_file() {
        with_stdin("3\n", || {
            let task = create_test_task();
            let result = prompt_for_task(&task);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::File));
        });
    }

    #[test]
    fn test_prompt_allow_directory() {
        with_stdin("4\n", || {
            let task = create_test_task();
            let result = prompt_for_task(&task);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Directory));
        });
    }

    #[test]
    fn test_prompt_deny() {
        with_stdin("5\n", || {
            let task = create_test_task();
            let result = prompt_for_task(&task);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), AllowDecision::Deny);
        });
    }

    #[test]
    fn test_prompt_invalid_input() {
        with_stdin("invalid\n", || {
            let task = create_test_task();
            let result = prompt_for_task(&task);
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                "Invalid choice. Please enter a number between 1 and 5."
            );
        });
    }
} 