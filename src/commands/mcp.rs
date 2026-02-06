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

    fn config_path(&self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        match self {
            Editor::Cursor => home.join(".cursor/mcp.json"),
            Editor::Vscode => home.join(".vscode/mcp.json"),
            Editor::Codex => home.join(".codex/config.toml"),
            Editor::Gemini => home.join(".gemini/settings.json"),
            Editor::ClaudeCode => home.join(".claude-code/settings.json"),
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

    /// The top-level key under which MCP server entries live
    fn servers_key(&self) -> &'static str {
        match self {
            Editor::Cursor | Editor::Gemini | Editor::ClaudeCode => "mcpServers",
            Editor::Vscode => "servers",
            Editor::Codex => "mcp_servers",
        }
    }

    /// The dela entry as a serde_json::Value (for JSON-based editors)
    fn dela_json_entry(&self) -> serde_json::Value {
        match self {
            Editor::Vscode => serde_json::json!({
                "type": "stdio",
                "command": "dela",
                "args": ["mcp"]
            }),
            _ => serde_json::json!({
                "command": "dela",
                "args": ["mcp"]
            }),
        }
    }
}

/// Merge dela into an existing JSON config file (Cursor, VSCode, Gemini, Claude Code)
fn merge_dela_into_json(editor: Editor, existing: &str) -> Result<String, String> {
    let mut root: serde_json::Value = serde_json::from_str(existing)
        .map_err(|e| format!("Failed to parse config as JSON: {}", e))?;

    let obj = root
        .as_object_mut()
        .ok_or_else(|| "Config file is not a JSON object".to_string())?;

    let key = editor.servers_key();
    if !obj.contains_key(key) {
        obj.insert(
            key.to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }

    let servers_obj = obj
        .get_mut(key)
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| format!("'{}' in config is not an object", key))?;

    servers_obj.insert("dela".to_string(), editor.dela_json_entry());

    let mut result = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    result.push('\n');
    Ok(result)
}

/// Merge dela into an existing TOML config file (Codex)
fn merge_dela_into_toml(existing: &str) -> Result<String, String> {
    let mut table: toml::Table = toml::from_str(existing)
        .map_err(|e| format!("Failed to parse config as TOML: {}", e))?;

    if !table.contains_key("mcp_servers") {
        table.insert(
            "mcp_servers".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
    }

    let mcp_table = table
        .get_mut("mcp_servers")
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| "'mcp_servers' in config is not a table".to_string())?;

    let mut dela = toml::map::Map::new();
    dela.insert(
        "command".to_string(),
        toml::Value::String("dela".to_string()),
    );
    dela.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("mcp".to_string())]),
    );
    mcp_table.insert("dela".to_string(), toml::Value::Table(dela));

    toml::to_string_pretty(&table).map_err(|e| format!("Failed to serialize config: {}", e))
}

/// Generate MCP config file for an editor at a specific path
fn generate_config_at(editor: Editor, config_path: &PathBuf) -> Result<(), String> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {} directory: {}", editor.name(), e))?;
        }
    }

    // Check if config already exists
    if config_path.exists() {
        let existing = fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read existing config: {}", e))?;

        if existing.contains(editor.dela_marker()) {
            eprintln!(
                "✓ {} config already has dela at {}",
                editor.name(),
                config_path.display()
            );
            return Ok(());
        }

        // Try to merge dela into existing config
        let merged = match editor {
            Editor::Codex => merge_dela_into_toml(&existing),
            _ => merge_dela_into_json(editor, &existing),
        };

        match merged {
            Ok(content) => {
                fs::write(config_path, content)
                    .map_err(|e| format!("Failed to write config file: {}", e))?;
                eprintln!(
                    "✓ Added dela to {} config at {}",
                    editor.name(),
                    config_path.display()
                );
            }
            Err(e) => {
                eprintln!(
                    "⚠ Could not auto-merge into {} config at {}: {}",
                    editor.name(),
                    config_path.display(),
                    e
                );
                eprintln!("  Please manually add dela to the config.");
            }
        }
        return Ok(());
    }

    // Write the config file
    fs::write(config_path, editor.template())
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    eprintln!(
        "✓ Created {} config at {}",
        editor.name(),
        config_path.display()
    );

    Ok(())
}

/// Generate MCP config file for an editor at its default global path
fn generate_config(editor: Editor) -> Result<(), String> {
    let config_path = editor.config_path();
    generate_config_at(editor, &config_path)
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
            generate_config(Editor::Cursor)?;
        }
        if init_vscode {
            generate_config(Editor::Vscode)?;
        }
        if init_codex {
            generate_config(Editor::Codex)?;
        }
        if init_gemini {
            generate_config(Editor::Gemini)?;
        }
        if init_claude_code {
            generate_config(Editor::ClaudeCode)?;
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
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        let result = generate_config_at(Editor::Cursor, &config_path);

        assert!(result.is_ok());
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("\"dela\""));
        assert!(content.contains("\"command\": \"dela\""));
    }

    #[test]
    fn test_generate_vscode_config_new() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".vscode/mcp.json");
        let result = generate_config_at(Editor::Vscode, &config_path);

        assert!(result.is_ok());
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("\"servers\""));
        assert!(content.contains("\"type\": \"stdio\""));
    }

    #[test]
    fn test_generate_config_already_exists_with_dela() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        let original = r#"{"mcpServers": {"dela": {"command": "dela"}}}"#;
        fs::write(&config_path, original).unwrap();

        let result = generate_config_at(Editor::Cursor, &config_path);
        assert!(result.is_ok());

        // File should be unchanged -- already has dela
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_merge_cursor_into_existing_json_with_other_servers() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        fs::write(
            &config_path,
            r#"{
  "mcpServers": {
    "other-server": {
      "command": "other",
      "args": ["serve"]
    }
  }
}"#,
        )
        .unwrap();

        let result = generate_config_at(Editor::Cursor, &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        // Preserves existing server
        assert!(content.contains("\"other-server\""));
        assert!(content.contains("\"command\": \"other\""));
        // Adds dela
        assert!(content.contains("\"dela\""));
        assert!(content.contains("\"command\": \"dela\""));
    }

    #[test]
    fn test_merge_vscode_into_existing_json_with_other_servers() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".vscode/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        fs::write(
            &config_path,
            r#"{
  "servers": {
    "other-server": {
      "type": "stdio",
      "command": "other",
      "args": ["serve"]
    }
  }
}"#,
        )
        .unwrap();

        let result = generate_config_at(Editor::Vscode, &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        // Preserves existing server
        assert!(content.contains("\"other-server\""));
        assert!(content.contains("\"command\": \"other\""));
        // Adds dela with VSCode-specific format
        assert!(content.contains("\"dela\""));
        assert!(content.contains("\"type\": \"stdio\""));
        assert!(content.contains("\"command\": \"dela\""));
    }

    #[test]
    fn test_merge_into_existing_json_without_servers_key() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        // Config exists but has no mcpServers key
        fs::write(&config_path, r#"{"someOtherSetting": true}"#).unwrap();

        let result = generate_config_at(Editor::Cursor, &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        // Preserves existing setting
        assert!(content.contains("\"someOtherSetting\""));
        // Creates mcpServers with dela
        assert!(content.contains("\"mcpServers\""));
        assert!(content.contains("\"dela\""));
    }

    #[test]
    fn test_merge_codex_into_existing_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".codex/config.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        fs::write(
            &config_path,
            "[mcp_servers.other]\ncommand = \"other\"\nargs = [\"serve\"]\n",
        )
        .unwrap();

        let result = generate_config_at(Editor::Codex, &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        // Preserves existing server
        assert!(content.contains("other"));
        // Adds dela
        assert!(content.contains("[mcp_servers.dela]"));
        assert!(content.contains("command = \"dela\""));
    }

    #[test]
    fn test_merge_graceful_fallback_on_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        // Invalid JSON (e.g., JSONC with comments)
        let original = "// this is a comment\n{\"mcpServers\": {}}";
        fs::write(&config_path, original).unwrap();

        let result = generate_config_at(Editor::Cursor, &config_path);
        // Should still succeed (graceful fallback)
        assert!(result.is_ok());

        // File should be unchanged since merge failed
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_editor_config_paths_use_home_dir() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            Editor::Cursor.config_path(),
            home.join(".cursor/mcp.json")
        );
        assert_eq!(
            Editor::Vscode.config_path(),
            home.join(".vscode/mcp.json")
        );
        assert_eq!(
            Editor::Codex.config_path(),
            home.join(".codex/config.toml")
        );
        assert_eq!(
            Editor::Gemini.config_path(),
            home.join(".gemini/settings.json")
        );
        assert_eq!(
            Editor::ClaudeCode.config_path(),
            home.join(".claude-code/settings.json")
        );
    }
}
