use clap::{Parser, Subcommand};

mod allowlist;
mod commands;
mod package_manager;
mod parsers;
mod prompt;
mod task_discovery;
mod task_shadowing;
mod types;

/// dela - A task runner that delegates to others
#[derive(Parser)]
#[command(
    name = "dela",
    author = "Alex Yankov",
    version,
    about = "A task runner that delegates to others",
    long_about = "Dela scans your project directory for task definitions in various formats (Makefile, package.json, etc.) and lets you run them directly from your shell.\n\nAfter running 'dela init', you can:\n1. Use 'dr <task>' to execute a task directly\n2. Type the task name to execute it through the shell integration"
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
    List {
        /// Show detailed information about task definition files
        #[arg(short, long)]
        verbose: bool,
    },

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

    /// Check if a task is allowed to run (used internally by shell functions)
    #[command(name = "allow-command")]
    AllowCommand {
        /// Name of the task to check
        task: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => commands::init::execute(),
        Commands::ConfigureShell => commands::configure_shell::execute(),
        Commands::List { verbose } => commands::list::execute(verbose),
        Commands::Run { task } => commands::run::execute(&task),
        Commands::GetCommand { task } => commands::get_command::execute(&task),
        Commands::AllowCommand { task } => commands::allow_command::execute(&task),
    };

    if let Err(err) = result {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
