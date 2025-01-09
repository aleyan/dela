use clap::{Parser, Subcommand};

mod types;
mod task_discovery;
mod parse_makefile;
mod commands;

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
    #[command(name = "configure-shell")]
    ConfigureShell,
    
    /// List all available tasks in the current directory
    List,
    
    /// Run a specific task
    Run {
        /// Name of the task to run
        task: String,
    },
    
    /// Get the shell command for a task (used internally by shell functions)
    #[command(name = "get-command")]
    GetCommand {
        /// Name of the task to get the command for
        task: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => {
            println!("Initializing dela...");
            // TODO(DTKT-14): Implement dela init command to automate creation of ~/.dela
            // TODO(DTKT-15): Modify dela init command to add eval of command_not_found_handle
            Ok(())
        }
        Commands::ConfigureShell => {
            println!("Configuring shell...");
            // TODO(DTKT-13): Implement dela configure_shell command to return the command_not_found_handle
            Ok(())
        }
        Commands::List => commands::list::execute(),
        Commands::Run { task } => {
            println!("Running task: {}", task);
            // TODO(DTKT-23): Complete dela run <task> for direct execution
            // TODO(DTKT-25): Prompt user if multiple matching tasks exist
            // TODO(DTKT-26): Implement logic to handle multiple tasks with the same name
            Ok(())
        }
        Commands::GetCommand { task } => commands::get_command::execute(&task),
    };

    if let Err(_) = result {
        std::process::exit(1);
    }
}
