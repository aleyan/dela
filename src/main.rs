use std::env;
use clap::{Parser, Subcommand};

mod types;
mod task_discovery;

use types::{DiscoveredTasks, TaskFileStatus};

/// dela - A task runner that delegates to others
#[derive(Parser)]
#[command(
    name = "dela",
    author = "Alex Yankov",
    version,
    about = "A task runner that delegates to others",
    long_about = "Dela scans your project directory for task definitions in various formats (Makefile, package.json, etc.) and lets you run them directly from your shell."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize dela and configure shell integration
    Init,
    
    /// Configure shell integration (used internally by init)
    ConfigureShell,
    
    /// List all available tasks in the current directory
    List,
    
    /// Run a specific task
    Run {
        /// Name of the task to run
        task: String,
    },
    
    /// Get the shell command for a task (used internally by shell functions)
    GetCommand {
        /// Name of the task to get the command for
        task: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            println!("Initializing dela...");
            // TODO(DTKT-14): Implement dela init command to automate creation of ~/.dela
            // TODO(DTKT-15): Modify dela init command to add eval of command_not_found_handle
        }
        Commands::ConfigureShell => {
            println!("Configuring shell...");
            // TODO(DTKT-13): Implement dela configure_shell command to return the command_not_found_handle
        }
        Commands::List => {
            let current_dir = env::current_dir().expect("Failed to get current directory");
            let discovered = task_discovery::discover_tasks(&current_dir);
            
            // Display task definition files status
            println!("Task definition files:");
            if let Some(makefile) = &discovered.definitions.makefile {
                print!("  Makefile: ");
                match &makefile.status {
                    TaskFileStatus::Parsed => println!("  Makefile: Found and parsed"),
                    TaskFileStatus::NotImplemented => println!("  Makefile: Found but parsing not yet implemented"),
                    TaskFileStatus::ParseError(e) => println!("  Makefile: Error parsing: {}", e),
                    TaskFileStatus::NotReadable(e) => println!("  Makefile: Not readable: {}", e),
                    TaskFileStatus::NotFound => println!("  Makefile: Not found"),
                }
            }
            if let Some(package_json) = &discovered.definitions.package_json {
                print!("  package.json: ");
                match &package_json.status {
                    TaskFileStatus::Parsed => println!("Found and parsed"),
                    TaskFileStatus::NotImplemented => println!("Found but parsing not yet implemented"),
                    TaskFileStatus::ParseError(e) => println!("Error parsing: {}", e),
                    TaskFileStatus::NotReadable(e) => println!("Not readable: {}", e),
                    TaskFileStatus::NotFound => println!("Not found"),
                }
            }
            if let Some(pyproject_toml) = &discovered.definitions.pyproject_toml {
                print!("  pyproject.toml: ");
                match &pyproject_toml.status {
                    TaskFileStatus::Parsed => println!("Found and parsed"),
                    TaskFileStatus::NotImplemented => println!("Found but parsing not yet implemented"),
                    TaskFileStatus::ParseError(e) => println!("Error parsing: {}", e),
                    TaskFileStatus::NotReadable(e) => println!("Not readable: {}", e),
                    TaskFileStatus::NotFound => println!("Not found"),
                }
            }
            println!();

            if discovered.tasks.is_empty() {
                println!("No tasks found in the current directory.");
                return;
            }

            println!("Available tasks:");
            for task in discovered.tasks {
                if let Some(desc) = task.description {
                    println!("  {} - {} (from {})", task.name, desc, task.file_path.display());
                } else {
                    println!("  {} (from {})", task.name, task.file_path.display());
                }
            }

            if !discovered.errors.is_empty() {
                println!("\nWarnings:");
                for error in discovered.errors {
                    println!("  {}", error);
                }
            }
        }
        Commands::Run { task } => {
            println!("Running task: {}", task);
            // TODO(DTKT-23): Complete dela run <task> for direct execution
            // TODO(DTKT-25): Prompt user if multiple matching tasks exist
            // TODO(DTKT-26): Implement logic to handle multiple tasks with the same name
        }
        Commands::GetCommand { task } => {
            println!("Getting command for task: {}", task);
            // TODO(DTKT-20): Implement shell environment inheritance for task execution
            // TODO(DTKT-21): Support both direct execution and subshell spawning based on task type
        }
    }
}
