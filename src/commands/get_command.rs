use std::env;
use crate::task_discovery;

pub fn execute(task: &str) {
    // TODO(DTKT-20): Implement shell environment inheritance for task execution
    // TODO(DTKT-21): Support both direct execution and subshell spawning based on task type
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let discovered = task_discovery::discover_tasks(&current_dir);
    
    // Find all tasks with the given name
    let matching_tasks: Vec<_> = discovered.tasks
        .iter()
        .filter(|t| t.name == task)
        .collect();

    match matching_tasks.len() {
        0 => {
            eprintln!("No task named '{}' found in the current directory.", task);
            std::process::exit(1);
        }
        1 => {
            // Single task found, return its command
            let task = matching_tasks[0];
            let command = task.runner.get_command(task);
            println!("{}", command);
        }
        _ => {
            // Multiple tasks found, print error and list them
            eprintln!("Multiple tasks named '{}' found:", task);
            for task in matching_tasks {
                eprintln!("  • {} (from {})", task.name, task.file_path.display());
            }
            eprintln!("Please use 'dela run {}' to choose which one to run.", task);
            std::process::exit(1);
        }
    }
} 