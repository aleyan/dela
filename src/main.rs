use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
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
            // TODO: Implement init logic
        }
        Commands::ConfigureShell => {
            println!("Configuring shell...");
            // TODO: Implement shell configuration
        }
        Commands::List => {
            println!("Listing tasks...");
            // TODO: Implement task listing
        }
        Commands::Run { task } => {
            println!("Running task: {}", task);
            // TODO: Implement task running
        }
        Commands::GetCommand { task } => {
            println!("Getting command for task: {}", task);
            // TODO: Implement command retrieval
        }
    }
}
