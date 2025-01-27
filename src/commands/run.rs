use std::path::Path;
use crate::task_discovery;
use crate::allowlist;

pub fn execute(task_name: &str) -> Result<(), String> {
    // Find the task
    let discovered = task_discovery::discover_tasks(Path::new("."));
    let task = discovered.tasks.iter()
        .find(|t| t.name == task_name)
        .ok_or_else(|| format!("Task '{}' not found", task_name))?;

    // Check if task is allowed
    if !allowlist::check_task_allowed(task)? {
        return Err(format!("Task '{}' was denied", task_name));
    }

    // TODO(DTKT-23): Execute the task
    println!("Task '{}' is allowed to run", task_name);
    Ok(())
} 