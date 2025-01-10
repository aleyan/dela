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
        _ => Err(format!("Unsupported shell: {}", shell_name)),
    }
} 