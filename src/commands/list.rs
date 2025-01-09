use std::env;
use std::collections::HashMap;
use crate::types::{Task, TaskFileStatus};
use crate::task_discovery;

pub fn execute() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
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
        return;
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
} 