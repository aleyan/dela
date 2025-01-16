use clap::{Parser, Subcommand};

mod types;
mod task_discovery;
mod parse_makefile;
mod parse_package_json;
mod parse_pyproject_toml;
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
            commands::init::execute()
        }
        Commands::ConfigureShell => {
            commands::configure_shell::execute()
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

    if let Err(err) = result {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
