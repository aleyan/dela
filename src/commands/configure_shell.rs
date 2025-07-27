use crate::environment::get_current_shell;

const ZSH_CONFIG: &str = include_str!("../../resources/zsh.sh");
const BASH_CONFIG: &str = include_str!("../../resources/bash.sh");
const FISH_CONFIG: &str = include_str!("../../resources/fish.sh");
const PWSH_CONFIG: &str = include_str!("../../resources/pwsh.ps1");

#[derive(Debug, PartialEq)]
enum Shell {
    Zsh,
    Bash,
    Fish,
    Pwsh,
    Unknown(String),
}

impl Shell {
    fn from_path(path: &str) -> Result<Shell, String> {
        let shell_path = std::path::PathBuf::from(path);
        let shell_name = shell_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "Invalid shell path".to_string())?;

        match shell_name {
            "zsh" => Ok(Shell::Zsh),
            "bash" => Ok(Shell::Bash),
            "fish" => Ok(Shell::Fish),
            "pwsh" => Ok(Shell::Pwsh),
            name => Ok(Shell::Unknown(name.to_string())),
        }
    }
}

pub fn execute() -> Result<(), String> {
    // Get the current shell from environment
    let shell = get_current_shell().ok_or("SHELL environment variable not set".to_string())?;

    // Parse the shell type
    let shell_type = Shell::from_path(&shell)?;

    // Handle each shell type
    match shell_type {
        Shell::Zsh => {
            print!("{}", ZSH_CONFIG);
            Ok(())
        }
        Shell::Bash => {
            print!("{}", BASH_CONFIG);
            Ok(())
        }
        Shell::Fish => {
            print!("{}", FISH_CONFIG);
            Ok(())
        }
        Shell::Pwsh => {
            print!("{}", PWSH_CONFIG);
            Ok(())
        }
        Shell::Unknown(name) => Err(format!("Unsupported shell: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{TestEnvironment, reset_to_real_environment, set_test_environment};
    use serial_test::serial;

    fn setup_test_env(shell: &str) {
        let test_env = TestEnvironment::new().with_shell(shell);
        set_test_environment(test_env);
    }

    #[test]
    #[serial]
    fn test_zsh_shell() {
        setup_test_env("/bin/zsh");
        let result = execute();
        assert!(result.is_ok());
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_bash_shell() {
        setup_test_env("/bin/bash");
        let result = execute();
        assert!(result.is_ok());
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_fish_shell() {
        setup_test_env("/usr/local/bin/fish");
        let result = execute();
        assert!(result.is_ok());
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_pwsh_shell() {
        setup_test_env("/usr/local/bin/pwsh");
        let result = execute();
        assert!(result.is_ok());
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_unknown_shell() {
        setup_test_env("/bin/unknown");
        let result = execute();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Unsupported shell: unknown");
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_invalid_shell_path() {
        setup_test_env("");
        let result = execute();
        assert!(result.is_err());
        reset_to_real_environment();
    }

    #[test]
    #[serial]
    fn test_missing_shell_env() {
        // Don't set any shell in test environment
        let test_env = TestEnvironment::new();
        set_test_environment(test_env);

        let result = execute();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "SHELL environment variable not set");
        reset_to_real_environment();
    }
}
