use std::env;

const ZSH_CONFIG: &str = include_str!("../../resources/zsh.sh");
const BASH_CONFIG: &str = include_str!("../../resources/bash.sh");

#[derive(Debug, PartialEq)]
enum Shell {
    Zsh,
    Bash,
    Fish,
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
            name => Ok(Shell::Unknown(name.to_string())),
        }
    }
}

pub fn execute() -> Result<(), String> {
    // Get the current shell from SHELL environment variable
    let shell = env::var("SHELL")
        .map_err(|_| "SHELL environment variable not set".to_string())?;
    
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
        Shell::Fish => Err("Fish shell integration not yet implemented".to_string()),
        Shell::Unknown(name) => Err(format!("Unsupported shell: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn setup_test_env(shell: &str) {
        env::remove_var("SHELL");
        env::set_var("SHELL", shell);
    }

    #[test]
    #[serial]
    fn test_zsh_shell() {
        setup_test_env("/bin/zsh");
        let result = execute();
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_bash_shell() {
        setup_test_env("/bin/bash");
        let result = execute();
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_fish_shell() {
        setup_test_env("/usr/local/bin/fish");
        let result = execute();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Fish shell integration not yet implemented"
        );
    }

    #[test]
    #[serial]
    fn test_unknown_shell() {
        setup_test_env("/bin/unknown");
        let result = execute();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Unsupported shell: unknown"
        );
    }

    #[test]
    #[serial]
    fn test_invalid_shell_path() {
        setup_test_env("");
        let result = execute();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Invalid shell path"
        );
    }

    #[test]
    #[serial]
    fn test_missing_shell_env() {
        env::remove_var("SHELL");
        let result = execute();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "SHELL environment variable not set"
        );
    }
} 