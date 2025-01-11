use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Get the path to the user's shell configuration file
fn get_shell_config_path() -> Result<PathBuf, String> {
    let shell = env::var("SHELL")
        .map_err(|_| "SHELL environment variable not set".to_string())?;
    
    let home = env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    
    let shell_path = PathBuf::from(&shell);
    let shell_name = shell_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid shell path".to_string())?;

    let config_path = match shell_name {
        "zsh" => PathBuf::from(&home).join(".zshrc"),
        "bash" => PathBuf::from(&home).join(".bashrc"),
        "fish" => PathBuf::from(&home).join(".config/fish/config.fish"),
        name => return Err(format!("Unsupported shell: {}", name)),
    };

    Ok(config_path)
}

/// Add dela shell integration to the shell config file
fn add_shell_integration(config_path: &PathBuf) -> Result<(), String> {
    // Read the current content
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read shell config: {}", e))?;

    // Check if dela integration is already present
    if content.contains("eval \"$(dela configure-shell)\"") {
        return Ok(());
    }

    // Open file in append mode
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(config_path)
        .map_err(|e| format!("Failed to open shell config: {}", e))?;

    // Add dela integration
    writeln!(file).map_err(|e| format!("Failed to write to shell config: {}", e))?;
    writeln!(file, "# dela shell integration").map_err(|e| format!("Failed to write to shell config: {}", e))?;
    writeln!(file, "eval \"$(dela configure-shell)\"").map_err(|e| format!("Failed to write to shell config: {}", e))?;

    Ok(())
}

pub fn execute() -> Result<(), String> {
    // Create ~/.dela directory if it doesn't exist
    let home = env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    let dela_dir = PathBuf::from(&home).join(".dela");
    
    if !dela_dir.exists() {
        fs::create_dir_all(&dela_dir)
            .map_err(|e| format!("Failed to create ~/.dela directory: {}", e))?;
        println!("Created ~/.dela directory");
    }

    // Get the shell config path
    let config_path = get_shell_config_path()?;

    // Add shell integration
    add_shell_integration(&config_path)?;
    println!("Added dela shell integration to {}", config_path.display());
    println!("\nPlease restart your shell or run:");
    println!("  source {}", config_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use serial_test::serial;

    fn setup_test_env(shell: &str, home: &PathBuf) -> Result<(), std::io::Error> {
        env::set_var("SHELL", shell);
        env::set_var("HOME", home.to_str().unwrap());
        Ok(())
    }

    #[test]
    #[serial]
    fn test_init_zsh() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        setup_test_env("/bin/zsh", &home).unwrap();

        // Create a minimal .zshrc
        let zshrc = home.join(".zshrc");
        fs::write(&zshrc, "# existing zsh config\n").unwrap();

        let result = execute();
        assert!(result.is_ok());

        // Verify the content
        let content = fs::read_to_string(&zshrc).unwrap();
        assert!(content.contains("eval \"$(dela configure-shell)\""));
    }

    #[test]
    #[serial]
    fn test_init_with_existing_integration() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        setup_test_env("/bin/zsh", &home).unwrap();

        // Create .zshrc with existing integration
        let zshrc = home.join(".zshrc");
        fs::write(&zshrc, "# existing config\neval \"$(dela configure-shell)\"\n").unwrap();

        let result = execute();
        assert!(result.is_ok());

        // Verify no duplicate integration was added
        let content = fs::read_to_string(&zshrc).unwrap();
        assert_eq!(content.matches("eval \"$(dela configure-shell)\"").count(), 1);
    }

    #[test]
    #[serial]
    fn test_init_creates_dela_dir() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        setup_test_env("/bin/zsh", &home).unwrap();

        // Create a minimal .zshrc
        let zshrc = home.join(".zshrc");
        fs::write(&zshrc, "# existing zsh config\n").unwrap();

        let result = execute();
        assert!(result.is_ok());

        // Verify ~/.dela was created
        let dela_dir = home.join(".dela");
        assert!(dela_dir.exists());
        assert!(dela_dir.is_dir());
    }

    #[test]
    #[serial]
    fn test_init_unsupported_shell() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        setup_test_env("/bin/unsupported", &home).unwrap();

        let result = execute();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Unsupported shell: unsupported");
    }
} 