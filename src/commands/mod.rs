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

#[cfg(not(test))]
use std::io::IsTerminal;

/// Returns an error if the current session is non-interactive (no TTY).
/// This prevents scripts and agents from running `dela allow` / `dela deny`.
pub(crate) fn gate_non_interactive(command_name: &str) -> anyhow::Result<()> {
    let is_terminal = {
        #[cfg(test)]
        {
            std::env::var("DELA_SIMULATE_TERMINAL").is_ok()
        }
        #[cfg(not(test))]
        {
            std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
        }
    };
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
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_gate_non_interactive_in_test_env() {
        let result = gate_non_interactive("dela allow");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "'dela allow' should only be run by human users directly, and not by scripts or agents."
        );
    }

    #[test]
    #[serial]
    fn test_gate_non_interactive_simulated_terminal() {
        unsafe {
            std::env::set_var("DELA_SIMULATE_TERMINAL", "1");
        }
        let result = gate_non_interactive("dela allow");
        unsafe {
            std::env::remove_var("DELA_SIMULATE_TERMINAL");
        }
        assert!(result.is_ok());
    }
}
