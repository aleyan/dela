use crate::mcp;
use anyhow::Context;
use std::fs;
use std::path::PathBuf;

fn dela_executable_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dela".to_string())
}

/// Supported editors for MCP config generation
#[derive(Debug, Clone, Copy)]
pub enum Editor {
    Cursor,
    Vscode,
    Codex,
    Gemini,
    ClaudeCode,
    Antigravity,
    Cline,
    OpenCode,
    Crush,
}

impl Editor {
    fn name(&self) -> &'static str {
        match self {
            Editor::Cursor => "Cursor",
            Editor::Vscode => "VSCode",
            Editor::Codex => "OpenAI Codex",
            Editor::Gemini => "Gemini CLI",
            Editor::ClaudeCode => "Claude Code",
            Editor::Antigravity => "Antigravity",
            Editor::Cline => "Cline",
            Editor::OpenCode => "OpenCode",
            Editor::Crush => "Crush",
        }
    }

    pub(crate) fn config_path(&self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        match self {
            Editor::Cursor => home.join(".cursor/mcp.json"),
            Editor::Vscode => home.join(".vscode/mcp.json"),
            Editor::Codex => home.join(".codex/config.toml"),
            Editor::Gemini => home.join(".gemini/settings.json"),
            Editor::ClaudeCode => home.join(".claude-code/settings.json"),
            Editor::Antigravity => home.join(".gemini/config/mcp_config.json"),
            Editor::Cline => std::env::var("CLINE_MCP_SETTINGS_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".cline/data/settings/cline_mcp_settings.json")),
            Editor::OpenCode => home.join(".config/opencode/opencode.json"),
            Editor::Crush => home.join(".config/crush/crush.json"),
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
            Editor::Cursor
            | Editor::Gemini
            | Editor::ClaudeCode
            | Editor::Antigravity
            | Editor::Cline
            | Editor::Crush => "mcpServers",
            Editor::Vscode => "servers",
            Editor::Codex => "mcp_servers",
            Editor::OpenCode => "mcp",
        }
    }

    /// The dela entry as a serde_json::Value (for JSON-based editors)
    fn dela_json_entry(&self) -> serde_json::Value {
        let exe_path = dela_executable_path();
        match self {
            Editor::Vscode => serde_json::json!({
                "type": "stdio",
                "command": exe_path,
                "args": ["mcp"]
            }),
            _ => serde_json::json!({
                "command": exe_path,
                "args": ["mcp"]
            }),
        }
    }
}

/// Merge dela into an existing JSON config file (Cursor, VSCode, Gemini, Claude Code)
fn merge_dela_into_json(editor: Editor, existing: &str) -> anyhow::Result<String> {
    let mut root: serde_json::Value = if existing.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(existing)
            .map_err(|e| anyhow::anyhow!("Failed to parse config as JSON: {}", e))?
    };

    let obj = root
        .as_object_mut()
        .context("Config file is not a JSON object")?;

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
        .with_context(|| format!("'{}' in config is not an object", key))?;

    servers_obj.insert("dela".to_string(), editor.dela_json_entry());

    let mut result = serde_json::to_string_pretty(&root)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    result.push('\n');
    Ok(result)
}

/// Merge dela into an existing TOML config file (Codex)
fn merge_dela_into_toml(existing: &str) -> anyhow::Result<String> {
    let mut table: toml::Table = if existing.trim().is_empty() {
        toml::Table::new()
    } else {
        toml::from_str(existing)
            .map_err(|e| anyhow::anyhow!("Failed to parse config as TOML: {}", e))?
    };

    if !table.contains_key("mcp_servers") {
        table.insert(
            "mcp_servers".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
    }

    let mcp_table = table
        .get_mut("mcp_servers")
        .and_then(|v| v.as_table_mut())
        .context("'mcp_servers' in config is not a table")?;

    let exe_path = dela_executable_path();
    let mut dela = toml::map::Map::new();
    dela.insert("command".to_string(), toml::Value::String(exe_path));
    dela.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("mcp".to_string())]),
    );
    mcp_table.insert("dela".to_string(), toml::Value::Table(dela));

    toml::to_string_pretty(&table).map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))
}

/// Generate MCP config file for an editor at a specific path
fn generate_config_at(editor: Editor, config_path: &PathBuf) -> anyhow::Result<()> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create {} directory: {}", editor.name(), e))?;
    }

    // Check if config already exists
    if config_path.exists() {
        let existing = fs::read_to_string(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read existing config: {}", e))?;

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
                    .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;
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

    // Write the config file using merge functions on empty config to generate absolute path
    let initial_content = match editor {
        Editor::Codex => "".to_string(),
        _ => "{}".to_string(),
    };
    let content = match editor {
        Editor::Codex => merge_dela_into_toml(&initial_content)?,
        _ => merge_dela_into_json(editor, &initial_content)?,
    };

    fs::write(config_path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;

    eprintln!(
        "✓ Created {} config at {}",
        editor.name(),
        config_path.display()
    );

    Ok(())
}

/// Generate MCP config file for an editor at its default global path
fn generate_config(editor: Editor) -> anyhow::Result<()> {
    let config_path = editor.config_path();
    generate_config_at(editor, &config_path)
}

/// Execute the MCP command
pub async fn execute(cwd: String, init_editor: Option<Editor>) -> anyhow::Result<()> {
    // Resolve the path relative to the current working directory
    let root_path = if cwd == "." {
        std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?
    } else {
        PathBuf::from(&cwd)
    };

    if let Some(editor) = init_editor {
        generate_config(editor)?;
        return Ok(());
    }

    crate::allowlist::load_allowlist().map_err(|e| {
        anyhow::anyhow!(
            crate::mcp::DelaError::mcp_not_ready(format!(
                "MCP server cannot start because dela configuration is unavailable: {}",
                e
            ))
            .to_error_data()
            .message
            .into_owned()
        )
    })?;

    // Start the MCP server
    mcp::run_stdio_server(root_path)
        .await
        .map_err(|e| anyhow::anyhow!(e))
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
        let expected_cmd = format!("\"command\": \"{}\"", dela_executable_path());
        assert!(content.contains(&expected_cmd));
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
        let expected_cmd = format!("\"command\": \"{}\"", dela_executable_path());
        assert!(content.contains(&expected_cmd));
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
        let expected_cmd = format!("\"command\": \"{}\"", dela_executable_path());
        assert!(content.contains(&expected_cmd));
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
        let expected_cmd = format!("\"command\": \"{}\"", dela_executable_path());
        assert!(content.contains(&expected_cmd));
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
        let expected_cmd = format!("command = \"{}\"", dela_executable_path());
        assert!(content.contains(&expected_cmd));
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
        assert_eq!(Editor::Cursor.config_path(), home.join(".cursor/mcp.json"));
        assert_eq!(Editor::Vscode.config_path(), home.join(".vscode/mcp.json"));
        assert_eq!(Editor::Codex.config_path(), home.join(".codex/config.toml"));
        assert_eq!(
            Editor::Gemini.config_path(),
            home.join(".gemini/settings.json")
        );
        assert_eq!(
            Editor::ClaudeCode.config_path(),
            home.join(".claude-code/settings.json")
        );
        assert_eq!(
            Editor::Antigravity.config_path(),
            home.join(".gemini/config/mcp_config.json")
        );
        assert_eq!(
            Editor::Cline.config_path(),
            home.join(".cline/data/settings/cline_mcp_settings.json")
        );
        assert_eq!(
            Editor::OpenCode.config_path(),
            home.join(".config/opencode/opencode.json")
        );
        assert_eq!(
            Editor::Crush.config_path(),
            home.join(".config/crush/crush.json")
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_cline_config_path_override() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let override_path = temp_dir.path().join("custom_cline_settings.json");
        let old_cline_path = std::env::var("CLINE_MCP_SETTINGS_PATH").ok();
        unsafe {
            std::env::set_var("CLINE_MCP_SETTINGS_PATH", &override_path);
        }
        let config_path = Editor::Cline.config_path();
        unsafe {
            if let Some(ref val) = old_cline_path {
                std::env::set_var("CLINE_MCP_SETTINGS_PATH", val);
            } else {
                std::env::remove_var("CLINE_MCP_SETTINGS_PATH");
            }
        }
        assert_eq!(config_path, override_path);
    }

    struct TestEnvGuard {
        old_dir: Option<std::path::PathBuf>,
        old_home: Option<String>,
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            if let Some(ref dir) = self.old_dir {
                let _ = std::env::set_current_dir(dir);
            }
            if let Some(ref home) = self.old_home {
                unsafe {
                    std::env::set_var("HOME", home);
                }
            } else {
                unsafe {
                    std::env::remove_var("HOME");
                }
            }
        }
    }

    #[test]
    fn test_editor_names_exhaustive() {
        for editor in &[
            Editor::Cursor,
            Editor::Vscode,
            Editor::Codex,
            Editor::Gemini,
            Editor::ClaudeCode,
            Editor::Antigravity,
            Editor::Cline,
            Editor::OpenCode,
            Editor::Crush,
        ] {
            let name = editor.name();
            assert!(!name.is_empty());
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_execute_init_cursor() {
        let temp_dir = TempDir::new().unwrap();
        // Save the old HOME env var and original CWD inside RAII guard
        let _guard = TestEnvGuard {
            old_dir: std::env::current_dir().ok(),
            old_home: std::env::var("HOME").ok(),
        };

        // Change current directory to temp_dir
        std::env::set_current_dir(temp_dir.path()).unwrap();

        unsafe {
            std::env::set_var("HOME", temp_dir.path());
        }

        let result = execute(".".to_string(), Some(Editor::Cursor)).await;
        if let Err(ref e) = result {
            panic!("execute failed with error: {:?}", e);
        }
        assert!(result.is_ok());

        let expected_path = temp_dir.path().join(".cursor/mcp.json");
        assert!(expected_path.exists());
    }
}
