use crate::mcp;
use std::fs;
use std::path::PathBuf;

/// MCP config template for editors using mcpServers format (Cursor, Gemini CLI)
const MCP_SERVERS_JSON_TEMPLATE: &str = r#"{
  "mcpServers": {
    "dela": {
      "command": "dela",
      "args": ["mcp"]
    }
  }
}
"#;

/// MCP config template for VSCode (.vscode/mcp.json)
const VSCODE_CONFIG_TEMPLATE: &str = r#"{
  "servers": {
    "dela": {
      "type": "stdio",
      "command": "dela",
      "args": ["mcp"]
    }
  }
}
"#;

/// MCP config template for OpenAI Codex (~/.codex/config.toml)
const CODEX_CONFIG_TEMPLATE: &str = r#"[mcp_servers.dela]
command = "dela"
args = ["mcp"]
"#;

/// Supported editors for MCP config generation
#[derive(Debug, Clone, Copy)]
pub enum Editor {
    Cursor,
    Vscode,
    Codex,
    Gemini,
    ClaudeCode,
}

impl Editor {
    fn name(&self) -> &'static str {
        match self {
            Editor::Cursor => "Cursor",
            Editor::Vscode => "VSCode",
            Editor::Codex => "OpenAI Codex",
            Editor::Gemini => "Gemini CLI",
            Editor::ClaudeCode => "Claude Code",
        }
    }

    fn config_path(&self, cwd: &PathBuf) -> PathBuf {
        match self {
            Editor::Cursor => cwd.join(".cursor/mcp.json"),
            Editor::Vscode => cwd.join(".vscode/mcp.json"),
            Editor::Codex => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".codex/config.toml"),
            Editor::Gemini => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".gemini/settings.json"),
            Editor::ClaudeCode => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".claude-code/settings.json"),
        }
    }

    fn template(&self) -> &'static str {
        match self {
            Editor::Cursor => MCP_SERVERS_JSON_TEMPLATE,
            Editor::Vscode => VSCODE_CONFIG_TEMPLATE,
            Editor::Codex => CODEX_CONFIG_TEMPLATE,
            Editor::Gemini => MCP_SERVERS_JSON_TEMPLATE,
            Editor::ClaudeCode => MCP_SERVERS_JSON_TEMPLATE,
        }
    }

    fn dela_marker(&self) -> &'static str {
        match self {
            Editor::Codex => "mcp_servers.dela",
            _ => "\"dela\"",
        }
    }
}

/// Generate MCP config file for an editor
fn generate_config(editor: Editor, cwd: &PathBuf) -> Result<(), String> {
    let config_path = editor.config_path(cwd);

    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("Failed to create {} directory: {}", editor.name(), e)
            })?;
        }
    }

    // Check if config already exists
    if config_path.exists() {
        let existing = fs::read_to_string(&config_path).map_err(|e| {
            format!("Failed to read existing config: {}", e)
        })?;

        if existing.contains(editor.dela_marker()) {
            eprintln!("✓ {} config already has dela at {}", editor.name(), config_path.display());
            return Ok(());
        } else {
            eprintln!("⚠ {} config exists at {}", editor.name(), config_path.display());
            eprintln!("  Please manually add dela to the config.");
            return Ok(());
        }
    }

    // Write the config file
    fs::write(&config_path, editor.template()).map_err(|e| {
        format!("Failed to write config file: {}", e)
    })?;

    eprintln!("✓ Created {} config at {}", editor.name(), config_path.display());

    Ok(())
}

/// Execute the MCP command
pub async fn execute(
    cwd: String,
    init_cursor: bool,
    init_vscode: bool,
    init_codex: bool,
    init_gemini: bool,
    init_claude_code: bool,
) -> Result<(), String> {
    // Resolve the path relative to the current working directory
    let root_path = if cwd == "." {
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
    } else {
        PathBuf::from(&cwd)
    };

    // Handle config generation flags
    let has_init_flag = init_cursor || init_vscode || init_codex || init_gemini || init_claude_code;

    if has_init_flag {
        if init_cursor {
            generate_config(Editor::Cursor, &root_path)?;
        }
        if init_vscode {
            generate_config(Editor::Vscode, &root_path)?;
        }
        if init_codex {
            generate_config(Editor::Codex, &root_path)?;
        }
        if init_gemini {
            generate_config(Editor::Gemini, &root_path)?;
        }
        if init_claude_code {
            generate_config(Editor::ClaudeCode, &root_path)?;
        }
        return Ok(());
    }

    // Start the MCP server
    mcp::run_stdio_server(root_path)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_cursor_config_new() {
        let temp_dir = TempDir::new().unwrap();
        let result = generate_config(Editor::Cursor, &temp_dir.path().to_path_buf());

        assert!(result.is_ok());

        let config_path = temp_dir.path().join(".cursor/mcp.json");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("\"dela\""));
        assert!(content.contains("\"command\": \"dela\""));
    }

    #[test]
    fn test_generate_vscode_config_new() {
        let temp_dir = TempDir::new().unwrap();
        let result = generate_config(Editor::Vscode, &temp_dir.path().to_path_buf());

        assert!(result.is_ok());

        let config_path = temp_dir.path().join(".vscode/mcp.json");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("\"servers\""));
        assert!(content.contains("\"type\": \"stdio\""));
    }

    #[test]
    fn test_generate_config_already_exists_with_dela() {
        let temp_dir = TempDir::new().unwrap();
        let cursor_dir = temp_dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();

        let config_path = cursor_dir.join("mcp.json");
        fs::write(&config_path, r#"{"mcpServers": {"dela": {"command": "dela"}}}"#).unwrap();

        let result = generate_config(Editor::Cursor, &temp_dir.path().to_path_buf());
        assert!(result.is_ok());
    }

    #[test]
    fn test_editor_config_paths() {
        let cwd = PathBuf::from("/test");
        assert_eq!(Editor::Cursor.config_path(&cwd), PathBuf::from("/test/.cursor/mcp.json"));
        assert_eq!(Editor::Vscode.config_path(&cwd), PathBuf::from("/test/.vscode/mcp.json"));
    }
}
