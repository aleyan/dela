use clap::{Parser, Subcommand};

mod allowlist;
mod builtins;
mod commands;
mod environment;
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
    about = "A task runner that delegates to other runners",
    long_about = r#"
Dela scans your project directory for task definitions in various formats (Makefile, package.json, etc.) and lets you run them directly from your shell.

🚀 **Key Feature**: After running '$ dela init', you can execute tasks with just their name:
   $ build    # Runs the 'build' task from your Makefile/package.json/etc.
   $ test     # Runs the 'test' task
   $ dr build # Alternative explicit syntax

This works by integrating with your shell's command-not-found handler to automatically discover and run tasks from your project files.
"#,
    help_template = "\
{before-help}{name} {version}

{about}

{usage-heading}
{usage}

{all-args}{after-help}"
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

    // Internal commands (hidden from help by default)
    #[command(name = "configure-shell", hide = true)]
    ConfigureShell,

    #[command(name = "get-command", hide = true, trailing_var_arg = true)]
    GetCommand {
        /// Name of the task followed by any arguments to pass to it
        args: Vec<String>,
    },

    #[command(name = "allow-command", hide = true)]
    AllowCommand {
        /// Name of the task to check
        task: String,
        /// Automatically allow with a specific choice (2-5)
        #[arg(long)]
        allow: Option<u8>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => commands::init::execute(),
        Commands::ConfigureShell => commands::configure_shell::execute(),
        Commands::List { verbose } => commands::list::execute(verbose),
        Commands::Run { task } => commands::run::execute(&task),
        Commands::GetCommand { args } => {
            if args.is_empty() {
                Err("No task name provided".to_string())
            } else {
                commands::get_command::execute(&args.join(" "))
            }
        }
        Commands::AllowCommand { task, allow } => commands::allow_command::execute(&task, allow),
    };

    if let Err(err) = result {
        if err.starts_with("dela: command or task not found") {
            eprintln!("{}", err);
        } else {
            eprintln!("Error: {}", err);
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_command_not_found_error() {
        // Create a temporary file to capture stderr
        let mut stderr_file = NamedTempFile::new().unwrap();

        // Function to test error handling
        let mut handle_error = |err: &str| {
            if err.starts_with("dela: command or task not found") {
                writeln!(stderr_file, "{}", err).unwrap();
            } else {
                writeln!(stderr_file, "Error: {}", err).unwrap();
            }
        };

        // Test command not found error
        handle_error("dela: command or task not found: missing_command");

        // Test regular error
        handle_error("Failed to execute task");

        // Reset file position to beginning for reading
        stderr_file.as_file_mut().flush().unwrap();
        let content = std::fs::read_to_string(stderr_file.path()).unwrap();

        // Check output content
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "Expected exactly two error lines");

        // First line should NOT have "Error:" prefix
        assert_eq!(
            lines[0], "dela: command or task not found: missing_command",
            "Command not found error should not have 'Error:' prefix"
        );

        // Second line should have "Error:" prefix
        assert_eq!(
            lines[1], "Error: Failed to execute task",
            "Regular error should have 'Error:' prefix"
        );
    }
}
