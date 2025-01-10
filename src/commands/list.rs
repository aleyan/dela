use std::env;
use std::collections::HashMap;
use crate::types::{Task, TaskFileStatus};
use crate::task_discovery;
use serial_test::serial;

pub fn execute() -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let discovered = task_discovery::discover_tasks(&current_dir);
    
    // Display task definition files status
    println!("Task definition files:");
    if let Some(makefile) = &discovered.definitions.makefile {
        match &makefile.status {
            TaskFileStatus::Parsed => println!("  ✓ Makefile: Found and parsed"),
            TaskFileStatus::NotImplemented => println!("  ! Makefile: Found but parsing not yet implemented"),
            TaskFileStatus::ParseError(e) => println!("  ✗ Makefile: Error parsing: {}", e),
            TaskFileStatus::NotReadable(e) => println!("  ✗ Makefile: Not readable: {}", e),
            TaskFileStatus::NotFound => println!("  - Makefile: Not found"),
        }
    }
    if let Some(package_json) = &discovered.definitions.package_json {
        match &package_json.status {
            TaskFileStatus::Parsed => println!("  ✓ package.json: Found and parsed"),
            TaskFileStatus::NotImplemented => println!("  ! package.json: Found but parsing not yet implemented"),
            TaskFileStatus::ParseError(e) => println!("  ✗ package.json: Error parsing: {}", e),
            TaskFileStatus::NotReadable(e) => println!("  ✗ package.json: Not readable: {}", e),
            TaskFileStatus::NotFound => println!("  - package.json: Not found"),
        }
    }
    if let Some(pyproject_toml) = &discovered.definitions.pyproject_toml {
        match &pyproject_toml.status {
            TaskFileStatus::Parsed => println!("  ✓ pyproject.toml: Found and parsed"),
            TaskFileStatus::NotImplemented => println!("  ! pyproject.toml: Found but parsing not yet implemented"),
            TaskFileStatus::ParseError(e) => println!("  ✗ pyproject.toml: Error parsing: {}", e),
            TaskFileStatus::NotReadable(e) => println!("  ✗ pyproject.toml: Not readable: {}", e),
            TaskFileStatus::NotFound => println!("  - pyproject.toml: Not found"),
        }
    }
    println!();

    if discovered.tasks.is_empty() {
        println!("No tasks found in the current directory.");
        return Ok(());
    }

    // Group tasks by their source file for better organization
    let mut tasks_by_file: HashMap<String, Vec<&Task>> = HashMap::new();
    for task in &discovered.tasks {
        tasks_by_file
            .entry(task.file_path.display().to_string())
            .or_default()
            .push(task);
    }

    println!("Available tasks:");
    for (file, tasks) in tasks_by_file {
        println!("\nFrom {}:", file);
        for task in tasks {
            if let Some(desc) = &task.description {
                println!("  • {} - {}", task.name, desc);
            } else {
                println!("  • {}", task.name);
            }
        }
    }

    if !discovered.errors.is_empty() {
        println!("\nWarnings:");
        for error in discovered.errors {
            println!("  ! {}", error);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        
        // Create a test Makefile
        let makefile_content = "
build: ## Building the project
\t@echo Building...

test: ## Running tests
\t@echo Testing...
";
        let mut makefile = File::create(temp_dir.path().join("Makefile"))
            .expect("Failed to create Makefile");
        makefile.write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        temp_dir
    }

    #[test]
    #[serial]
    fn test_list_with_task_files() {
        let original_dir = env::current_dir().expect("Failed to get current directory");
        let temp_dir = setup_test_dir();
        env::set_current_dir(temp_dir.path()).expect("Failed to change directory");

        let result = execute();
        assert!(result.is_ok(), "Should succeed with task files present");

        // Restore directory before dropping temp_dir
        env::set_current_dir(&original_dir).expect("Failed to restore directory");
        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_list_empty_directory() {
        let original_dir = env::current_dir().expect("Failed to get current directory");
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        env::set_current_dir(temp_dir.path()).expect("Failed to change directory");

        let result = execute();
        assert!(result.is_ok(), "Should succeed with empty directory");

        // Restore directory before dropping temp_dir
        env::set_current_dir(&original_dir).expect("Failed to restore directory");
        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_list_with_invalid_makefile() {
        let original_dir = env::current_dir().expect("Failed to get current directory");
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        env::set_current_dir(temp_dir.path()).expect("Failed to change directory");

        // Create an invalid Makefile
        let makefile_content = "<invalid>makefile</invalid>";
        let mut makefile = File::create(temp_dir.path().join("Makefile"))
            .expect("Failed to create Makefile");
        makefile.write_all(makefile_content.as_bytes())
            .expect("Failed to write Makefile");

        let result = execute();
        assert!(result.is_ok(), "Should succeed with invalid Makefile");

        // Restore directory before dropping temp_dir
        env::set_current_dir(&original_dir).expect("Failed to restore directory");
        drop(temp_dir);
    }
} 