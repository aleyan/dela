use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
enum Shell {
    Zsh,
    Bash,
    Fish,
    Unknown(String),
}

impl Shell {
    fn from_path(path: &str) -> Result<Shell, String> {
        let shell_path = PathBuf::from(path);
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
            // Read and print the zsh.sh file
            let zsh_config = fs::read_to_string("resources/zsh.sh")
                .map_err(|e| format!("Failed to read zsh.sh: {}", e))?;
            print!("{}", zsh_config);
            Ok(())
        }
        Shell::Bash => Err("Bash shell integration not yet implemented".to_string()),
        Shell::Fish => Err("Fish shell integration not yet implemented".to_string()),
        Shell::Unknown(name) => Err(format!("Unsupported shell: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    use serial_test::serial;

    fn setup_test_env(shell: &str) {
        env::remove_var("SHELL");
        env::set_var("SHELL", shell);
    }

    fn create_test_zsh_file() -> (TempDir, PathBuf) {
        let test_dir = TempDir::new().unwrap();
        let test_path = test_dir.path().to_path_buf();
        let resources_dir = test_path.join("resources");
        fs::create_dir(&resources_dir).unwrap();
        
        let zsh_content = "# Test zsh config\necho 'test'";
        let mut file = fs::File::create(resources_dir.join("zsh.sh")).unwrap();
        file.write_all(zsh_content.as_bytes()).unwrap();
        
        (test_dir, test_path)
    }

    #[test]
    #[serial]
    fn test_zsh_shell() {
        let (test_dir, test_path) = create_test_zsh_file();
        setup_test_env("/bin/zsh");
        
        // Change to the test directory before executing
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&test_path).unwrap();
        
        let result = execute();
        assert!(result.is_ok());

        // Restore directory before dropping test_dir
        env::set_current_dir(&original_dir).unwrap();
        drop(test_dir);
    }

    #[test]
    #[serial]
    fn test_bash_shell() {
        setup_test_env("/bin/bash");
        let result = execute();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Bash shell integration not yet implemented"
        );
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