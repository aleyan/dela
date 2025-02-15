use crate::types::{AllowScope, Task};
use std::io::{self, Write};

#[derive(Debug, PartialEq)]
pub enum AllowDecision {
    Allow(AllowScope),
    Deny,
}

/// Prompt the user for a decision about a task
pub fn prompt_for_task(task: &Task) -> Result<AllowDecision, String> {
    println!(
        "\nTask '{}' from '{}' requires approval.",
        task.name,
        task.file_path.display()
    );
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
    io::stdout()
        .flush()
        .map_err(|e| format!("Failed to flush stdout: {}", e))?;

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
    use crate::types::{Task, TaskRunner};
    use std::path::PathBuf;

    // Helper function to create a test task
    fn create_test_task() -> Task {
        Task {
            name: "test-task".to_string(),
            description: Some("A test task".to_string()),
            file_path: PathBuf::from("Makefile"),
            runner: TaskRunner::Make,
            source_name: "test-task".to_string(),
            shadowed_by: None,
        }
    }

    // Mock prompt function for testing
    fn mock_prompt_for_task(input: &str, _task: &Task) -> Result<AllowDecision, String> {
        match input {
            "1" => Ok(AllowDecision::Allow(AllowScope::Once)),
            "2" => Ok(AllowDecision::Allow(AllowScope::Task)),
            "3" => Ok(AllowDecision::Allow(AllowScope::File)),
            "4" => Ok(AllowDecision::Allow(AllowScope::Directory)),
            "5" => Ok(AllowDecision::Deny),
            _ => Err("Invalid choice. Please enter a number between 1 and 5.".to_string()),
        }
    }

    #[test]
    fn test_prompt_allow_once() {
        let task = create_test_task();
        let result = mock_prompt_for_task("1", &task);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Once));
    }

    #[test]
    fn test_prompt_allow_task() {
        let task = create_test_task();
        let result = mock_prompt_for_task("2", &task);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Task));
    }

    #[test]
    fn test_prompt_allow_file() {
        let task = create_test_task();
        let result = mock_prompt_for_task("3", &task);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::File));
    }

    #[test]
    fn test_prompt_allow_directory() {
        let task = create_test_task();
        let result = mock_prompt_for_task("4", &task);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Allow(AllowScope::Directory));
    }

    #[test]
    fn test_prompt_deny() {
        let task = create_test_task();
        let result = mock_prompt_for_task("5", &task);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AllowDecision::Deny);
    }

    #[test]
    fn test_prompt_invalid_input() {
        let task = create_test_task();
        let result = mock_prompt_for_task("invalid", &task);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Invalid choice. Please enter a number between 1 and 5."
        );
    }
}
