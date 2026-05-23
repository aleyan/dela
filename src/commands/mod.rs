pub mod allow;
pub mod allow_command;
pub mod configure_shell;
pub mod deny;
pub mod get_command;
pub mod init;
pub mod list;
pub mod mcp;
pub mod run;
pub mod run_command;

use std::io::IsTerminal;

pub(crate) fn should_block(is_terminal: bool, is_test: bool, force_interactive: bool) -> bool {
    !is_terminal && !is_test && !force_interactive
}

pub(crate) fn gate_non_interactive(command_name: &str) {
    let is_terminal = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    let is_test = std::env::var("RUST_TEST_THREADS").is_ok() || std::env::var("CARGO_TEST").is_ok();
    let force_interactive = std::env::var("DELA_FORCE_INTERACTIVE").is_ok();

    if should_block(is_terminal, is_test, force_interactive) {
        eprintln!(
            "'{}' should only be run by human users directly, and not by scripts or agents.",
            command_name
        );
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_block_non_interactive() {
        assert!(!should_block(true, false, false)); // Interactive session
        assert!(should_block(false, false, false)); // Non-interactive session
        assert!(!should_block(false, true, false)); // In tests
        assert!(!should_block(false, false, true)); // Force interactive override
    }
}
