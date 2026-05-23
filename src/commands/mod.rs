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

/// Returns an error if the current session is non-interactive (no TTY).
/// This prevents scripts and agents from running `dela allow` / `dela deny`.
pub(crate) fn gate_non_interactive(command_name: &str) -> anyhow::Result<()> {
    let is_terminal = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    if !is_terminal {
        anyhow::bail!(
            "'{}' should only be run by human users directly, and not by scripts or agents.",
            command_name
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_non_interactive_in_test_env() {
        let result = gate_non_interactive("dela allow");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "'dela allow' should only be run by human users directly, and not by scripts or agents."
        );
    }
}
