use clap::{Parser, Subcommand};

mod allowlist;
mod commands;
mod parsers;
mod prompt;
mod runner;
mod runners {
    pub mod runners_package_json;
    pub mod runners_pyproject_toml;
}
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
    long_about = "Dela scans your project directory for task definitions in various formats (Makefile, package.json, etc.) and lets you run them directly from your shell.\n\nAfter running '$ dela init', you can:\n1. Use '$ dr <task>' to execute a task directly\n2. Execute task with bare name `$ <task>` through the shell integration"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize dela and configure shell integration
    ///
    /// This command will:
    /// 1. Create ~/.dela directory for configuration
    /// 2. Create an initial allowlist.toml
    /// 3. Add shell integration to your shell's config file
    ///
    /// Example: dela init
    Init,

    /// Configure shell integration (used internally by init)
    ///
    /// Outputs shell-specific configuration code that needs to be evaluated.
    /// This is typically called by your shell's config file, not directly.
    ///
    /// Example: eval "$(dela configure-shell)"
    #[command(name = "configure-shell")]
    ConfigureShell,

    /// List all available tasks in the current directory
    ///
    /// Shows tasks from Makefiles, package.json scripts, pyproject.toml, and more.
    /// Use --verbose for additional details about task sources and runners.
    ///
    /// Example: dela list
    /// Example: dela list --verbose
    List {
        /// Show detailed information about task definition files
        #[arg(short, long)]
        verbose: bool,
    },

    /// Run a specific task
    ///
    /// Note: This command is meant to be used through shell integration.
    /// Instead of 'dela run <task>', use 'dr <task>' or just '<task>'.
    ///
    /// Example: dr build
    /// Example: build
    Run {
        /// Name of the task to run
        task: String,
    },

    /// Get the shell command for a task (used internally by shell functions)
    ///
    /// Returns the actual command that would be executed for a task.
    /// This is used internally by shell integration and shouldn't be called directly.
    ///
    /// Example: dela get-command build
    #[command(name = "get-command")]
    GetCommand {
        /// Name of the task to get the command for
        task: String,
    },

    /// Check if a task is allowed to run (used internally by shell functions)
    ///
    /// Consults the allowlist at ~/.dela/allowlist.toml to determine if a task can be executed.
    /// If the command is not covered by the allowlist, it will prompt the user to allow or deny the command.
    /// This is used internally by shell integration and shouldn't be called directly.
    ///
    /// Example: dela allow-command build
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
