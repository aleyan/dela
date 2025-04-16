use crate::commands::run_command;

pub fn execute(task_name: &str) -> Result<(), String> {
    println!("Note: The 'dela run' command is meant to be intercepted by shell integration.");
    println!("If you're seeing this message, it means either:");
    println!("1. Shell integration is not installed (run 'dela init' to set it up)");
    println!("2. You're running dela directly instead of through the shell function");

    // Execute the task directly when shell integration is not detected
    run_command::execute(task_name)
}
