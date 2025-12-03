use crate::mcp;
use std::path::PathBuf;

/// Execute the MCP command
pub async fn execute(cwd: String) -> Result<(), String> {
    // Resolve the path relative to the current working directory
    let root_path = if cwd == "." {
        match std::env::current_dir() {
            Ok(path) => path,
            Err(_) => {
                std::process::exit(1);
            }
        }
    } else {
        PathBuf::from(cwd)
    };

    mcp::run_stdio_server(root_path)
        .await
        .map_err(|e| e.to_string())
}
