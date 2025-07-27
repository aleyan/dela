use crate::types::Allowlist;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Get the current shell name by checking the parent process
fn get_current_shell() -> Result<String, String> {
    // Try to get shell from BASH_VERSION or ZSH_VERSION first
    if env::var("BASH_VERSION").is_ok() {
        return Ok("bash".to_string());
    }
    if env::var("ZSH_VERSION").is_ok() {
        return Ok("zsh".to_string());
    }

    // Fallback to $SHELL if version variables aren't set
    let shell = env::var("SHELL").map_err(|_| "SHELL environment variable not set".to_string())?;

    let shell_path = std::path::PathBuf::from(&shell);
    shell_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid shell path".to_string())
}

/// Get the appropriate shell config path based on current shell
fn get_shell_config_path() -> Result<PathBuf, String> {
    let shell_name = get_current_shell()?;
    let home = env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;
    let home_path = PathBuf::from(&home);

    match shell_name.as_str() {
        "zsh" => Ok(home_path.join(".zshrc")),
        "bash" => Ok(home_path.join(".bashrc")),
        "fish" => Ok(home_path.join(".config").join("fish").join("config.fish")),
        "pwsh" => Ok(home_path
            .join(".config")
            .join("powershell")
            .join("Microsoft.PowerShell_profile.ps1")),
        name => Err(format!("Unsupported shell: {}", name)),
    }
}

/// Add dela shell integration to the shell config file
fn add_shell_integration(config_path: &PathBuf) -> Result<(), String> {
    // Read the current content
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read shell config: {}", e))?;

    // Get the shell type from the path
    let shell = get_current_shell()?;

    // Check if dela integration is already present, with shell-specific patterns
    let integration_pattern = match shell.as_str() {
        "fish" => "eval (dela configure-shell | string collect)",
        "pwsh" => "Invoke-Expression (dela configure-shell | Out-String)",
        _ => "eval \"$(dela configure-shell)\"",
    };

    if content.contains(integration_pattern) {
        println!(
            "Shell integration already present in {}",
            config_path.display()
        );
        return Ok(());
    }

    // Create parent directory if it doesn't exist (needed for PowerShell)
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
    }

    // Open file in append mode
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(config_path)
        .map_err(|e| format!("Failed to open shell config: {}", e))?;

    // Add dela integration with shell-specific syntax
    writeln!(file).map_err(|e| format!("Failed to write to shell config: {}", e))?;
    writeln!(file, "# dela shell integration")
        .map_err(|e| format!("Failed to write to shell config: {}", e))?;
    writeln!(file, "{}", integration_pattern)
        .map_err(|e| format!("Failed to write to shell config: {}", e))?;

    Ok(())
}

pub fn execute() -> Result<(), String> {
    println!("Initializing dela...");

    // Get the shell config path first to validate shell support
    let config_path = get_shell_config_path()?;
    let shell_name = get_current_shell()?;

    println!(
        "Detected {} shell, configuring {}",
        shell_name,
        config_path.display()
    );

    // Create ~/.dela directory if it doesn't exist
    let home = env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;
    let dela_dir = PathBuf::from(&home).join(".dela");

    if !dela_dir.exists() {
        println!(
            "Creating dela configuration directory at {}",
            dela_dir.display()
        );
        fs::create_dir_all(&dela_dir)
            .map_err(|e| format!("Failed to create ~/.dela directory: {}", e))?;
    } else {
        println!(
            "Using existing dela configuration directory at {}",
            dela_dir.display()
        );
    }

    // Create empty allowlist.toml if it doesn't exist
    let allowlist_path = dela_dir.join("allowlist.toml");
    if !allowlist_path.exists() {
        println!("Creating empty allowlist at {}", allowlist_path.display());
        let empty_allowlist = Allowlist::default();
        let toml = toml::to_string_pretty(&empty_allowlist)
            .map_err(|e| format!("Failed to serialize empty allowlist: {}", e))?;
        fs::write(&allowlist_path, toml)
            .map_err(|e| format!("Failed to create allowlist file: {}", e))?;
    }

    // Add shell integration
    println!("Adding shell integration to {}", config_path.display());
    add_shell_integration(&config_path)?;

    println!("\nInitialization complete! To activate dela, either:");
    println!("1. Restart your shell");
    println!("2. Run: source {}", config_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn setup_test_env(shell: &str, home: &PathBuf) -> Result<(), std::io::Error> {
        unsafe {
            env::set_var("SHELL", shell);
            env::set_var("HOME", home.to_str().unwrap());
        }
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
        fs::write(
            &zshrc,
            "# existing config\neval \"$(dela configure-shell)\"\n",
        )
        .unwrap();

        let result = execute();
        assert!(result.is_ok());

        // Verify no duplicate integration was added
        let content = fs::read_to_string(&zshrc).unwrap();
        assert_eq!(
            content.matches("eval \"$(dela configure-shell)\"").count(),
            1
        );
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

    #[test]
    #[serial]
    fn test_init_fish() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        setup_test_env("/usr/bin/fish", &home).unwrap();

        // Create fish config directory and minimal config.fish
        let fish_config_dir = home.join(".config").join("fish");
        fs::create_dir_all(&fish_config_dir).unwrap();
        let config_fish = fish_config_dir.join("config.fish");
        fs::write(&config_fish, "# existing fish config\n").unwrap();

        let result = execute();
        assert!(result.is_ok());

        // Verify the content has the fish-specific integration pattern
        let content = fs::read_to_string(&config_fish).unwrap();
        assert!(content.contains("eval (dela configure-shell | string collect)"));
    }

    #[test]
    #[serial]
    fn test_init_pwsh() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        setup_test_env("/bin/pwsh", &home).unwrap();

        // Create PowerShell config directory and minimal profile
        let pwsh_config_dir = home.join(".config").join("powershell");
        fs::create_dir_all(&pwsh_config_dir).unwrap();
        let config_pwsh = pwsh_config_dir.join("Microsoft.PowerShell_profile.ps1");
        fs::write(&config_pwsh, "# existing PowerShell config\n").unwrap();

        let result = execute();
        assert!(result.is_ok());

        // Verify the content has the PowerShell-specific integration pattern
        let content = fs::read_to_string(&config_pwsh).unwrap();
        assert!(content.contains("Invoke-Expression (dela configure-shell | Out-String)"));
    }

    #[test]
    #[serial]
    fn test_init_creates_allowlist() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        setup_test_env("/bin/zsh", &home).unwrap();

        // Create a minimal .zshrc
        let zshrc = home.join(".zshrc");
        fs::write(&zshrc, "# existing zsh config\n").unwrap();

        let result = execute();
        assert!(result.is_ok());

        // Verify allowlist.toml was created
        let allowlist_path = home.join(".dela").join("allowlist.toml");
        assert!(allowlist_path.exists());
        assert!(allowlist_path.is_file());

        // Verify it contains valid TOML for an empty allowlist
        let content = fs::read_to_string(&allowlist_path).unwrap();
        let allowlist: Allowlist = toml::from_str(&content).unwrap();
        assert!(allowlist.entries.is_empty());
    }
}
