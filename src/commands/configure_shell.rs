use std::env;
use std::fs;
use std::path::PathBuf;

pub fn execute() -> Result<(), String> {
    // Get the current shell from SHELL environment variable
    let shell = env::var("SHELL")
        .map_err(|_| "SHELL environment variable not set".to_string())?;
    
    // Extract the shell name from the path
    let shell_path = PathBuf::from(&shell);
    let shell_name = shell_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid shell path".to_string())?;

    // Match on the exact shell name
    match shell_name {
        "zsh" => {
            // Read and print the zsh.sh file
            let zsh_config = fs::read_to_string("resources/zsh.sh")
                .map_err(|e| format!("Failed to read zsh.sh: {}", e))?;
            print!("{}", zsh_config);
            Ok(())
        }
        "bash" => Err("Bash shell integration not yet implemented".to_string()),
        "fish" => Err("Fish shell integration not yet implemented".to_string()),
        _ => Err("Invalid shell path".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_env(shell: &str) {
        env::set_var("SHELL", shell);
    }

    fn create_test_zsh_file() -> TempDir {
        let test_dir = TempDir::new().unwrap();
        let resources_dir = test_dir.path().join("resources");
        fs::create_dir(&resources_dir).unwrap();
        
        let zsh_content = "# Test zsh config\necho 'test'";
        let mut file = fs::File::create(resources_dir.join("zsh.sh")).unwrap();
        file.write_all(zsh_content.as_bytes()).unwrap();
        
        test_dir
    }

    #[test]
    fn test_zsh_shell() {
        let test_dir = create_test_zsh_file();
        setup_test_env("/bin/zsh");
        
        // Change to the test directory before executing
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(test_dir.path()).unwrap();
        
        let result = execute();
        if let Err(e) = &result {
            eprintln!("Error: {}", e);
            eprintln!("Current dir: {:?}", env::current_dir().unwrap());
            eprintln!("Test dir: {:?}", test_dir.path());
        }
        assert!(result.is_ok());

        // Restore directory before dropping test_dir
        env::set_current_dir(&original_dir).unwrap();
        drop(test_dir);
    }

    #[test]
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
    fn test_unknown_shell() {
        setup_test_env("/bin/unknown");
        let result = execute();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Invalid shell path"
        );
    }

    #[test]
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