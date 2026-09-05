use crate::mcp;
use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

fn dela_executable_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.canonicalize().ok())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dela".to_string())
}

fn command_needs_update(command: Option<&str>) -> bool {
    // A different valid absolute path may be another installation; keeping it avoids
    // needlessly reserializing a user-global config whenever dela is invoked elsewhere.
    command.is_none_or(|command| {
        let path = Path::new(command);
        !path.is_absolute() || !path.exists()
    })
}

/// Preserve only server launch arguments, excluding help and editor initialization.
fn args_need_update(args: Option<&[String]>) -> bool {
    let Some([command, options @ ..]) = args else {
        return true;
    };
    if command != "mcp" {
        return true;
    }
    match options {
        [] => false,
        [flag, cwd] if flag == "--cwd" => cwd.is_empty() || cwd.starts_with('-'),
        [option] => option.strip_prefix("--cwd=").is_none_or(str::is_empty),
        _ => true,
    }
}

/// An editor to configure, plus the workspace `--cwd` pinned it to (if any)
#[derive(Debug, Clone, Copy)]
struct InitTarget<'a> {
    editor: Editor,
    /// Set only when the user passed `--cwd`, which pins the generated entry to one
    /// workspace instead of letting dela discover tasks from wherever the editor starts it.
    workspace: Option<&'a Path>,
}

impl InitTarget<'_> {
    /// The arguments dela should be launched with
    fn desired_args(&self) -> Vec<String> {
        let mut args = vec!["mcp".to_string()];
        if let Some(workspace) = self.workspace {
            args.push("--cwd".to_string());
            args.push(workspace.to_string_lossy().into_owned());
        }
        args
    }

    /// Whether args already present should be replaced. An explicit `--cwd` is a direct
    /// instruction and wins; without one, existing args are only repaired if broken.
    fn args_need_replacing(&self, existing: Option<&[String]>) -> bool {
        match self.workspace {
            Some(_) => existing != Some(self.desired_args().as_slice()),
            None => args_need_update(existing),
        }
    }
}

/// The on-disk format of an editor's config file
#[derive(Debug, Clone, Copy, PartialEq)]
enum ConfigFormat {
    Json,
    Toml,
}

/// How an editor expects the dela launch command to be encoded in a server entry
#[derive(Debug, Clone, Copy)]
enum CommandShape {
    /// `"command": "<dela>", "args": ["mcp"]`
    CommandArgs,
    /// `"command": ["<dela>", "mcp"]`
    CommandArray,
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
    Grok,
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
            Editor::Grok => "Grok Build",
        }
    }

    pub(crate) fn config_path(&self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        match self {
            Editor::Cursor => home.join(".cursor/mcp.json"),
            // VSCode reads user-level MCP config from the user profile folder; ~/.vscode
            // only holds argv.json and extensions. `.vscode/mcp.json` is workspace-scoped.
            Editor::Vscode => dirs::config_dir()
                .unwrap_or_else(|| home.join(".config"))
                .join("Code/User/mcp.json"),
            Editor::Codex => home.join(".codex/config.toml"),
            Editor::Gemini => home.join(".gemini/settings.json"),
            // Claude Code keeps user-scope servers in ~/.claude.json, the same file
            // `claude mcp add --scope user` writes.
            Editor::ClaudeCode => home.join(".claude.json"),
            Editor::Antigravity => home.join(".gemini/config/mcp_config.json"),
            Editor::Cline => std::env::var("CLINE_MCP_SETTINGS_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".cline/data/settings/cline_mcp_settings.json")),
            Editor::OpenCode => home.join(".config/opencode/opencode.json"),
            Editor::Crush => home.join(".config/crush/crush.json"),
            // Grok Build keeps user-scope servers in its main config, the same file
            // `grok mcp add --scope user` writes.
            Editor::Grok => std::env::var_os("GROK_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".grok"))
                .join("config.toml"),
        }
    }

    /// The top-level key under which MCP server entries live
    fn servers_key(&self) -> &'static str {
        match self {
            Editor::Cursor
            | Editor::Gemini
            | Editor::ClaudeCode
            | Editor::Antigravity
            | Editor::Cline => "mcpServers",
            Editor::Vscode => "servers",
            Editor::Codex | Editor::Grok => "mcp_servers",
            Editor::OpenCode | Editor::Crush => "mcp",
        }
    }

    /// The file format the editor's config is written in
    fn config_format(&self) -> ConfigFormat {
        match self {
            Editor::Codex | Editor::Grok => ConfigFormat::Toml,
            _ => ConfigFormat::Json,
        }
    }

    /// A key dela wrote in the past that the editor does not accept, cleaned up on re-init
    fn legacy_servers_key(&self) -> Option<&'static str> {
        match self {
            // Crush's root schema is additionalProperties:false, so a leftover
            // "mcpServers" key keeps the whole config invalid even once "mcp" is right.
            Editor::Crush => Some("mcpServers"),
            _ => None,
        }
    }

    /// The transport discriminator the editor requires on each server entry, if any
    fn entry_type(&self) -> Option<&'static str> {
        match self {
            Editor::Vscode | Editor::ClaudeCode | Editor::Crush => Some("stdio"),
            Editor::OpenCode => Some("local"),
            _ => None,
        }
    }

    /// How the editor encodes the launch command in a server entry
    fn command_shape(&self) -> CommandShape {
        match self {
            // OpenCode's McpLocalConfig takes the executable and its arguments as a
            // single array and rejects a separate "args" key.
            Editor::OpenCode => CommandShape::CommandArray,
            _ => CommandShape::CommandArgs,
        }
    }
}

impl InitTarget<'_> {
    /// The dela entry as a serde_json::Value (for JSON-based editors)
    fn dela_json_entry(&self) -> serde_json::Value {
        let exe_path = dela_executable_path();
        let args = self.desired_args();
        let mut entry = serde_json::Map::new();
        if let Some(entry_type) = self.editor.entry_type() {
            entry.insert("type".to_string(), serde_json::json!(entry_type));
        }
        match self.editor.command_shape() {
            CommandShape::CommandArgs => {
                entry.insert("command".to_string(), serde_json::json!(exe_path));
                entry.insert("args".to_string(), serde_json::json!(args));
            }
            CommandShape::CommandArray => {
                let mut argv = vec![exe_path];
                argv.extend(args);
                entry.insert("command".to_string(), serde_json::json!(argv));
            }
        }
        serde_json::Value::Object(entry)
    }
}

fn merge_dela_json_entry(
    target: InitTarget,
    servers: &mut serde_json::Map<String, serde_json::Value>,
) -> bool {
    let Some(existing_entry) = servers
        .get_mut("dela")
        .and_then(|value| value.as_object_mut())
    else {
        servers.insert("dela".to_string(), target.dela_json_entry());
        return true;
    };

    let mut mutated = false;
    match target.editor.command_shape() {
        CommandShape::CommandArgs => {
            if command_needs_update(
                existing_entry
                    .get("command")
                    .and_then(serde_json::Value::as_str),
            ) {
                existing_entry.insert(
                    "command".to_string(),
                    serde_json::Value::String(dela_executable_path()),
                );
                mutated = true;
            }
            let existing_args = json_string_array(existing_entry.get("args"));
            if target.args_need_replacing(existing_args.as_deref()) {
                existing_entry.insert("args".to_string(), serde_json::json!(target.desired_args()));
                mutated = true;
            }
        }
        CommandShape::CommandArray => {
            // Accept the array shape or the string+args shape left by an older dela, so a
            // valid alternate install and any extra args survive the conversion.
            let existing_argv = json_string_array(existing_entry.get("command"));
            let (existing_exe, existing_args) = match existing_argv.as_deref() {
                Some([exe, args @ ..]) => (Some(exe.clone()), Some(args.to_vec())),
                _ => (
                    existing_entry
                        .get("command")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    json_string_array(existing_entry.get("args")),
                ),
            };

            let exe = existing_exe
                .filter(|exe| !command_needs_update(Some(exe)))
                .unwrap_or_else(dela_executable_path);
            let args = if target.args_need_replacing(existing_args.as_deref()) {
                target.desired_args()
            } else {
                existing_args.unwrap_or_else(|| target.desired_args())
            };

            let mut argv = vec![exe];
            argv.extend(args);
            let expected = serde_json::json!(argv);
            if existing_entry.get("command") != Some(&expected) {
                existing_entry.insert("command".to_string(), expected);
                mutated = true;
            }
            // An "args" key left over from the command/args shape is rejected outright.
            if existing_entry.shift_remove("args").is_some() {
                mutated = true;
            }
        }
    }
    if let Some(entry_type) = target.editor.entry_type()
        && existing_entry.get("type") != Some(&serde_json::json!(entry_type))
    {
        existing_entry.insert("type".to_string(), serde_json::json!(entry_type));
        mutated = true;
    }
    mutated
}

/// A JSON array read as plain strings, or None if it is absent or holds anything else
fn json_string_array(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    value
        .and_then(serde_json::Value::as_array)?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}

/// Remove the invalid legacy key only when no unrelated entries would be lost.
fn remove_legacy_dela_entry(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    legacy_key: &str,
) -> anyhow::Result<bool> {
    let Some(value) = obj.get(legacy_key) else {
        return Ok(false);
    };
    if value
        .as_object()
        .is_none_or(|servers| servers.keys().any(|name| name != "dela"))
    {
        anyhow::bail!(
            "'{}' is not supported by Crush. Manually migrate its entries to 'mcp' and remove '{}' before re-running initialization.",
            legacy_key,
            legacy_key
        );
    }
    obj.shift_remove(legacy_key);
    Ok(true)
}

/// Merge dela into an existing JSON config file (Cursor, VSCode, Gemini, Claude Code)
fn merge_dela_into_json(target: InitTarget, existing: &str) -> anyhow::Result<Option<String>> {
    let mut mutated = false;
    let mut root: serde_json::Value = if existing.trim().is_empty() {
        mutated = true;
        serde_json::json!({})
    } else {
        serde_json::from_str(existing)
            .map_err(|e| anyhow::anyhow!("Failed to parse config as JSON: {}", e))?
    };

    let obj = root
        .as_object_mut()
        .context("Config file is not a JSON object")?;

    let key = target.editor.servers_key();
    if !obj.contains_key(key) {
        obj.insert(
            key.to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
        mutated = true;
    }

    let servers_obj = obj
        .get_mut(key)
        .and_then(|v| v.as_object_mut())
        .with_context(|| format!("'{}' in config is not an object", key))?;
    mutated |= merge_dela_json_entry(target, servers_obj);

    if let Some(legacy_key) = target.editor.legacy_servers_key() {
        mutated |= remove_legacy_dela_entry(obj, legacy_key)?;
    }

    if !mutated {
        return Ok(None);
    }

    let mut result = serde_json::to_string_pretty(&root)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    result.push('\n');
    Ok(Some(result))
}

/// A TOML array read as plain strings, or None if it is absent or holds anything else
fn toml_string_array(item: Option<&toml_edit::Item>) -> Option<Vec<String>> {
    item?
        .as_array()?
        .iter()
        .map(|value| value.as_str().map(str::to_string))
        .collect()
}

fn toml_args_value(args: &[String]) -> toml_edit::Item {
    toml_edit::value(args.iter().collect::<toml_edit::Array>())
}

fn merge_dela_toml_entry(target: InitTarget, servers: &mut toml_edit::Table) -> bool {
    let Some(dela) = servers
        .get_mut("dela")
        .and_then(|item| item.as_table_like_mut())
    else {
        let mut dela = toml_edit::Table::new();
        dela.insert("command", toml_edit::value(dela_executable_path()));
        dela.insert("args", toml_args_value(&target.desired_args()));
        if matches!(target.editor, Editor::Grok) {
            // Grok writes this itself. Seed it once on creation only -- never on merge, so
            // a later `grok mcp disable dela` is not silently undone by a re-init.
            dela.insert("enabled", toml_edit::value(true));
        }
        servers.insert("dela", toml_edit::Item::Table(dela));
        return true;
    };

    let mut mutated = false;
    if command_needs_update(dela.get("command").and_then(|item| item.as_str())) {
        dela.insert("command", toml_edit::value(dela_executable_path()));
        mutated = true;
    }
    let existing_args = toml_string_array(dela.get("args"));
    if target.args_need_replacing(existing_args.as_deref()) {
        dela.insert("args", toml_args_value(&target.desired_args()));
        mutated = true;
    }
    mutated
}

/// Merge dela into an existing TOML config file (Codex)
///
/// Uses toml_edit rather than a parse/reserialize round trip so comments, key order and
/// formatting in the user's config survive untouched.
fn merge_dela_into_toml(target: InitTarget, existing: &str) -> anyhow::Result<Option<String>> {
    let mut mutated = existing.trim().is_empty();
    let mut doc: toml_edit::DocumentMut = existing
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse config as TOML: {}", e))?;

    let key = target.editor.servers_key();
    if !doc.contains_key(key) {
        let mut servers = toml_edit::Table::new();
        // Implicit so the entry renders as [mcp_servers.dela], not a bare [mcp_servers].
        servers.set_implicit(true);
        doc.insert(key, toml_edit::Item::Table(servers));
        mutated = true;
    }

    let servers = doc
        .get_mut(key)
        .and_then(|item| item.as_table_mut())
        .with_context(|| format!("'{}' in config is not a table", key))?;
    mutated |= merge_dela_toml_entry(target, servers);

    if !mutated {
        return Ok(None);
    }

    Ok(Some(doc.to_string()))
}

/// Write through a temp file in the same directory so an interrupted write can never
/// truncate a config that also holds unrelated state (notably Claude Code's ~/.claude.json).
fn write_config_atomically(config_path: &Path, content: &str) -> anyhow::Result<()> {
    let tmp_path = config_path.with_extension(format!("dela-tmp-{}", std::process::id()));
    fs::write(&tmp_path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;

    // The temp file is a fresh inode created under the process umask, so replacing a
    // restrictive config would otherwise widen it (~/.claude.json is 0600 and holds
    // account state). Carry the destination's permissions over before the rename.
    if let Ok(metadata) = fs::metadata(config_path)
        && let Err(e) = fs::set_permissions(&tmp_path, metadata.permissions())
    {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow::anyhow!(
            "Failed to preserve permissions of {}: {}",
            config_path.display(),
            e
        ));
    }

    fs::rename(&tmp_path, config_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        anyhow::anyhow!("Failed to write config file: {}", e)
    })
}

fn merge_editor_config(target: InitTarget, existing: &str) -> anyhow::Result<Option<String>> {
    match target.editor.config_format() {
        ConfigFormat::Toml => merge_dela_into_toml(target, existing),
        ConfigFormat::Json => merge_dela_into_json(target, existing),
    }
}

fn update_existing_config(target: InitTarget, config_path: &Path) -> anyhow::Result<()> {
    let existing = fs::read_to_string(config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read existing config: {}", e))?;

    match merge_editor_config(target, &existing) {
        Ok(Some(content)) => {
            write_config_atomically(config_path, &content)?;
            eprintln!(
                "✓ Updated dela in {} config at {}",
                target.editor.name(),
                config_path.display()
            );
        }
        Ok(None) => {
            eprintln!(
                "✓ {} config already has dela at {}",
                target.editor.name(),
                config_path.display()
            );
        }
        Err(error) => {
            eprintln!(
                "⚠ Could not auto-merge into {} config at {}: {}",
                target.editor.name(),
                config_path.display(),
                error
            );
            eprintln!("  Please manually add dela to the config.");
        }
    }
    Ok(())
}

/// Generate MCP config file for an editor at a specific path
fn generate_config_at(target: InitTarget, config_path: &Path) -> anyhow::Result<()> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|e| {
            anyhow::anyhow!("Failed to create {} directory: {}", target.editor.name(), e)
        })?;
    }

    if config_path.exists() {
        return update_existing_config(target, config_path);
    }

    let initial_content = match target.editor.config_format() {
        ConfigFormat::Toml => "".to_string(),
        ConfigFormat::Json => "{}".to_string(),
    };
    let content = merge_editor_config(target, &initial_content)?
        .context("Empty editor config did not produce a dela entry")?;

    write_config_atomically(config_path, &content)?;

    eprintln!(
        "✓ Created {} config at {}",
        target.editor.name(),
        config_path.display()
    );

    Ok(())
}

/// Generate MCP config file for an editor at its default global path
fn generate_config(target: InitTarget) -> anyhow::Result<()> {
    let config_path = target.editor.config_path();
    generate_config_at(target, &config_path)
}

/// Absolute path for a `--cwd` written into a config file
///
/// Editor configs are global and the editor may launch dela from anywhere, so a relative
/// path in the generated entry would resolve against the wrong directory.
fn pinned_workspace(cwd: &str) -> anyhow::Result<PathBuf> {
    let path = if cwd == "." {
        std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?
    } else {
        PathBuf::from(cwd)
    };
    // Refuse rather than fall back to the relative path: a pin that cannot be resolved
    // now becomes an entry that silently finds no tasks later, and nothing repairs it.
    let resolved = path.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Cannot pin --cwd to {}: {}. Pass a directory that exists.",
            path.display(),
            e
        )
    })?;
    if !resolved.is_dir() {
        return Err(anyhow::anyhow!(
            "Cannot pin --cwd to {}: not a directory.",
            resolved.display()
        ));
    }
    Ok(resolved)
}

/// Execute the MCP command
pub async fn execute(cwd: Option<String>, init_editor: Option<Editor>) -> anyhow::Result<()> {
    if let Some(editor) = init_editor {
        // Without --cwd the entry stays workspace-agnostic and dela discovers tasks from
        // wherever the editor starts it; with --cwd it is pinned to that workspace.
        let workspace = cwd.as_deref().map(pinned_workspace).transpose()?;
        return generate_config(InitTarget {
            editor,
            workspace: workspace.as_deref(),
        });
    }

    // Resolve the path relative to the current working directory
    let root_path = match cwd.as_deref() {
        None | Some(".") => std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?,
        Some(cwd) => PathBuf::from(cwd),
    };

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

    /// An unpinned target, matching `dela mcp --init-<editor>` with no --cwd
    fn global(editor: Editor) -> InitTarget<'static> {
        InitTarget {
            editor,
            workspace: None,
        }
    }

    #[test]
    fn test_generate_cursor_config_new() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        let result = generate_config_at(global(Editor::Cursor), &config_path);

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
        let result = generate_config_at(global(Editor::Vscode), &config_path);

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

        let original = format!(
            "{{\n  \"mcpServers\": {{\n    \"dela\": {{\n      \"args\": [\n        \"mcp\"\n      ],\n      \"command\": \"{}\"\n    }}\n  }}\n}}\n",
            dela_executable_path()
        );
        fs::write(&config_path, &original).unwrap();

        let result = generate_config_at(global(Editor::Cursor), &config_path);
        assert!(result.is_ok());

        // File should be unchanged -- already has dela with exact absolute path
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_generate_config_leaves_valid_alternate_json_command_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let alternate_dela = temp_dir.path().join("installed/dela");
        fs::create_dir_all(alternate_dela.parent().unwrap()).unwrap();
        fs::write(&alternate_dela, "").unwrap();

        let original = format!(
            r#"{{"mcpServers":{{"dela":{{"args":["mcp"],"command":"{}"}}}}}}"#,
            alternate_dela.display()
        );
        fs::write(&config_path, &original).unwrap();

        let result = generate_config_at(global(Editor::Cursor), &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_generate_config_leaves_valid_alternate_toml_command_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".codex/config.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let alternate_dela = temp_dir.path().join("installed/dela");
        fs::create_dir_all(alternate_dela.parent().unwrap()).unwrap();
        fs::write(&alternate_dela, "").unwrap();

        let original = format!(
            "# User comments above dela\n[mcp_servers.dela]\n# Command comment\ncommand = \"{}\"\nargs = [\"mcp\"]\n\n# User comments below\n",
            alternate_dela.display()
        );
        fs::write(&config_path, &original).unwrap();

        let result = generate_config_at(global(Editor::Codex), &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_command_update_policy() {
        let temp_dir = TempDir::new().unwrap();
        let existing_dela = temp_dir.path().join("installed/dela");
        fs::create_dir_all(existing_dela.parent().unwrap()).unwrap();
        fs::write(&existing_dela, "").unwrap();
        let missing_dela = temp_dir.path().join("missing/dela");

        assert!(command_needs_update(None));
        assert!(command_needs_update(Some("dela")));
        assert!(command_needs_update(Some(missing_dela.to_str().unwrap())));
        assert!(!command_needs_update(Some(existing_dela.to_str().unwrap())));
    }

    #[test]
    fn test_merge_preserves_user_added_args() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("mcp.json");
        let original = format!(
            r#"{{"mcpServers":{{"dela":{{"command":"{}","args":["mcp","--cwd","/some/workspace"]}}}}}}"#,
            dela_executable_path()
        );
        fs::write(&config_path, &original).unwrap();

        generate_config_at(global(Editor::Cursor), &config_path).unwrap();

        // Nothing was broken, so the file is left byte-identical -- including the --cwd.
        assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
    }

    #[test]
    fn test_merge_preserves_user_added_args_in_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let original = format!(
            "[mcp_servers.dela]\ncommand = \"{}\"\nargs = [\"mcp\", \"--cwd\", \"/some/workspace\"]\n",
            dela_executable_path()
        );
        fs::write(&config_path, &original).unwrap();

        generate_config_at(global(Editor::Codex), &config_path).unwrap();

        assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
    }

    #[test]
    fn test_merge_repairs_args_that_would_not_start_the_server() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("mcp.json");
        fs::write(
            &config_path,
            format!(
                r#"{{"mcpServers":{{"dela":{{"command":"{}","args":["serve","--cwd","/w"]}}}}}}"#,
                dela_executable_path()
            ),
        )
        .unwrap();

        generate_config_at(global(Editor::Cursor), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed["mcpServers"]["dela"]["args"],
            serde_json::json!(["mcp"])
        );
    }

    #[test]
    fn test_launch_argument_grammar() {
        for args in [
            vec!["mcp"],
            vec!["mcp", "--cwd", "/workspace with spaces"],
            vec!["mcp", "--cwd=/workspace"],
        ] {
            let args: Vec<String> = args.into_iter().map(str::to_string).collect();
            assert!(!args_need_update(Some(&args)), "{args:?}");
        }
        assert!(args_need_update(None));
    }

    #[test]
    fn test_all_config_shapes_repair_malformed_launch_arguments() {
        for args in [
            vec![],
            vec!["serve"],
            vec!["mcp", "--cwd"],
            vec!["mcp", "--cwd", ""],
            vec!["mcp", "--cwd="],
            vec!["mcp", "--cwd", "--help"],
            vec!["mcp", "--cwd", "/w", "--cwd", "/other"],
            vec!["mcp", "--unknown"],
            vec!["mcp", "--help"],
            vec!["mcp", "--init-cursor"],
            vec!["mcp", "unexpected"],
        ] {
            for editor in [Editor::Cursor, Editor::OpenCode, Editor::Grok] {
                let target = global(editor);
                let original = match editor.config_format() {
                    ConfigFormat::Json => {
                        let mut entry = target.dela_json_entry();
                        match editor.command_shape() {
                            CommandShape::CommandArgs => entry["args"] = serde_json::json!(args),
                            CommandShape::CommandArray => {
                                let mut argv = vec![dela_executable_path()];
                                argv.extend(args.iter().map(|arg| arg.to_string()));
                                entry["command"] = serde_json::json!(argv);
                            }
                        }
                        serde_json::json!({editor.servers_key(): {"dela": entry}}).to_string()
                    }
                    ConfigFormat::Toml => format!(
                        "[mcp_servers.dela]\ncommand = {:?}\nargs = {:?}\nenabled = true\n",
                        dela_executable_path(),
                        args
                    ),
                };
                let repaired = merge_editor_config(target, &original).unwrap().unwrap();
                let expected = merge_editor_config(target, "").unwrap().unwrap();
                assert_eq!(repaired, expected, "{editor:?}: {args:?}");
                assert!(merge_editor_config(target, &repaired).unwrap().is_none());
            }
        }
    }

    #[test]
    fn test_init_with_cwd_pins_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("mcp.json");
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();

        let target = InitTarget {
            editor: Editor::Cursor,
            workspace: Some(&workspace),
        };
        generate_config_at(target, &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed["mcpServers"]["dela"]["args"],
            serde_json::json!(["mcp", "--cwd", workspace.to_string_lossy()])
        );
    }

    #[test]
    fn test_explicit_cwd_overrides_an_existing_pin() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("mcp.json");
        let workspace = temp_dir.path().join("new");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(
            &config_path,
            format!(
                r#"{{"mcpServers":{{"dela":{{"command":"{}","args":["mcp","--cwd","/old"]}}}}}}"#,
                dela_executable_path()
            ),
        )
        .unwrap();

        let target = InitTarget {
            editor: Editor::Cursor,
            workspace: Some(&workspace),
        };
        generate_config_at(target, &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed["mcpServers"]["dela"]["args"],
            serde_json::json!(["mcp", "--cwd", workspace.to_string_lossy()])
        );
    }

    #[test]
    fn test_opencode_pins_workspace_in_command_array() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("opencode.json");
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();

        let target = InitTarget {
            editor: Editor::OpenCode,
            workspace: Some(&workspace),
        };
        generate_config_at(target, &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed["mcp"]["dela"]["command"],
            serde_json::json!([
                dela_executable_path(),
                "mcp",
                "--cwd",
                workspace.to_string_lossy()
            ])
        );
    }

    #[test]
    fn test_opencode_migration_carries_over_extra_args() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("opencode.json");
        fs::write(
            &config_path,
            format!(
                r#"{{"mcp":{{"dela":{{"command":"{}","args":["mcp","--cwd","/w"]}}}}}}"#,
                dela_executable_path()
            ),
        )
        .unwrap();

        generate_config_at(global(Editor::OpenCode), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed["mcp"]["dela"]["command"],
            serde_json::json!([dela_executable_path(), "mcp", "--cwd", "/w"])
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_merge_preserves_restrictive_config_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".claude.json");
        fs::write(&config_path, r#"{"numStartups":1}"#).unwrap();
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).unwrap();

        generate_config_at(global(Editor::ClaudeCode), &config_path).unwrap();

        // The atomic replace must not widen a config that holds account state.
        let mode = fs::metadata(&config_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "permissions widened to {mode:o}");
    }

    #[test]
    #[cfg(unix)]
    fn test_write_leaves_no_temp_file_behind() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("mcp.json");
        generate_config_at(global(Editor::Cursor), &config_path).unwrap();
        generate_config_at(global(Editor::Cursor), &config_path).unwrap();

        let leftovers: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok().map(|e| e.file_name()))
            .filter(|name| name.to_string_lossy().contains("dela-tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_pinned_workspace_makes_a_relative_path_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let _guard = TestEnvGuard {
            old_dir: std::env::current_dir().ok(),
            old_home: std::env::var("HOME").ok(),
        };
        std::env::set_current_dir(temp_dir.path()).unwrap();
        fs::create_dir_all("workspace").unwrap();

        let resolved = pinned_workspace("./workspace").unwrap();

        assert!(resolved.is_absolute(), "{resolved:?} is not absolute");
        assert!(resolved.ends_with("workspace"));
    }

    #[test]
    fn test_pinned_workspace_rejects_a_missing_directory() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("typo");

        let error = pinned_workspace(missing.to_str().unwrap()).unwrap_err();

        // Better to fail loudly than to write an entry that finds no tasks.
        assert!(
            error.to_string().contains("Cannot pin --cwd"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn test_pinned_workspace_rejects_a_file() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("not-a-dir");
        fs::write(&file, "").unwrap();

        let error = pinned_workspace(file.to_str().unwrap()).unwrap_err();

        assert!(
            error.to_string().contains("not a directory"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn test_generate_grok_config_new() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        generate_config_at(global(Editor::Grok), &config_path).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(
            content,
            format!(
                "[mcp_servers.dela]\ncommand = \"{}\"\nargs = [\"mcp\"]\nenabled = true\n",
                dela_executable_path()
            )
        );
    }

    #[test]
    fn test_grok_merge_preserves_existing_settings() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        // ~/.grok/config.toml is Grok's main config, not an MCP-only file.
        let original = "\
[cli]
installer = \"internal\"
auto_update = true

[ui]
yolo = false
";
        fs::write(&config_path, original).unwrap();

        generate_config_at(global(Editor::Grok), &config_path).unwrap();

        let merged = fs::read_to_string(&config_path).unwrap();
        assert!(
            merged.starts_with(original),
            "existing settings changed: {merged}"
        );
        assert!(merged.contains("[mcp_servers.dela]"));
        assert!(merged.contains("enabled = true"));
    }

    #[test]
    fn test_grok_merge_does_not_re_enable_a_disabled_server() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        // What `grok mcp disable dela` leaves behind.
        let original = format!(
            "disabled_mcp_servers = [\"dela\"]\n\n[mcp_servers.dela]\ncommand = \"{}\"\nargs = [\"mcp\"]\nenabled = false\n",
            dela_executable_path()
        );
        fs::write(&config_path, &original).unwrap();

        generate_config_at(global(Editor::Grok), &config_path).unwrap();

        // Re-running init must not silently undo a deliberate disable.
        assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
    }

    #[test]
    fn test_grok_repairs_stale_command_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        fs::write(
            &config_path,
            "[mcp_servers.dela]\ncommand = \"dela\"\nargs = [\"mcp\"]\nenabled = false\n",
        )
        .unwrap();

        generate_config_at(global(Editor::Grok), &config_path).unwrap();

        let merged = fs::read_to_string(&config_path).unwrap();
        assert!(merged.contains(&format!("command = \"{}\"", dela_executable_path())));
        // The broken path is fixed without touching the user's enabled choice.
        assert!(merged.contains("enabled = false"));
    }

    #[test]
    fn test_codex_merge_preserves_comments_and_layout() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let original = "\
# Codex settings a user wrote by hand
model = \"o3\"

# Keep this comment attached to the server
[mcp_servers.other]
command = \"other\"  # trailing note
args = [\"serve\"]
";
        fs::write(&config_path, original).unwrap();

        generate_config_at(global(Editor::Codex), &config_path).unwrap();

        let merged = fs::read_to_string(&config_path).unwrap();
        assert!(merged.contains("# Codex settings a user wrote by hand"));
        assert!(merged.contains("# Keep this comment attached to the server"));
        assert!(merged.contains("command = \"other\"  # trailing note"));
        // The untouched part of the file is preserved verbatim, dela is appended.
        assert!(
            merged.starts_with(original),
            "existing content was rewritten: {merged}"
        );
        assert!(merged.contains("[mcp_servers.dela]"));
    }

    #[test]
    fn test_generate_config_repairs_empty_args() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let original = format!(
            r#"{{"mcpServers":{{"dela":{{"args":[],"command":"{}"}}}}}}"#,
            dela_executable_path()
        );
        fs::write(&config_path, original).unwrap();

        generate_config_at(global(Editor::Cursor), &config_path).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed["mcpServers"]["dela"]["args"],
            serde_json::json!(["mcp"])
        );
    }

    #[test]
    fn test_generate_config_updates_relative_command_to_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cursor/mcp.json");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        let original = r#"{"mcpServers": {"dela": {"command": "dela", "args": ["mcp"]}}}"#;
        fs::write(&config_path, original).unwrap();

        let result = generate_config_at(global(Editor::Cursor), &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        let expected_cmd = format!("\"command\": \"{}\"", dela_executable_path());
        assert!(content.contains(&expected_cmd));
    }

    #[test]
    fn test_merge_codex_updates_relative_command_and_preserves_subtables() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".codex/config.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();

        fs::write(
            &config_path,
            "[mcp_servers.dela]\ncommand = \"dela\"\nargs = [\"mcp\"]\n\n[mcp_servers.dela.tools.status]\napproval_mode = \"approve\"\n",
        )
        .unwrap();

        let result = generate_config_at(global(Editor::Codex), &config_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        let expected_cmd = format!("command = \"{}\"", dela_executable_path());
        assert!(content.contains(&expected_cmd));
        assert!(content.contains("[mcp_servers.dela.tools.status]"));
        assert!(content.contains("approval_mode = \"approve\""));
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

        let result = generate_config_at(global(Editor::Cursor), &config_path);
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

        let result = generate_config_at(global(Editor::Vscode), &config_path);
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

        let result = generate_config_at(global(Editor::Cursor), &config_path);
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

        let result = generate_config_at(global(Editor::Codex), &config_path);
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

        let result = generate_config_at(global(Editor::Cursor), &config_path);
        // Should still succeed (graceful fallback)
        assert!(result.is_ok());

        // File should be unchanged since merge failed
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    #[serial_test::serial]
    fn test_editor_config_paths_use_home_dir() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(Editor::Cursor.config_path(), home.join(".cursor/mcp.json"));
        assert_eq!(
            Editor::Vscode.config_path(),
            dirs::config_dir().unwrap().join("Code/User/mcp.json")
        );
        assert_eq!(Editor::Codex.config_path(), home.join(".codex/config.toml"));
        assert_eq!(
            Editor::Gemini.config_path(),
            home.join(".gemini/settings.json")
        );
        assert_eq!(Editor::ClaudeCode.config_path(), home.join(".claude.json"));
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

    #[test]
    #[serial_test::serial]
    fn test_grok_config_path_override_and_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let old_grok_home = std::env::var_os("GROK_HOME");
        unsafe {
            std::env::set_var("GROK_HOME", temp_dir.path());
        }
        let overridden = Editor::Grok.config_path();
        unsafe {
            std::env::remove_var("GROK_HOME");
        }
        let fallback = Editor::Grok.config_path();
        unsafe {
            if let Some(value) = old_grok_home {
                std::env::set_var("GROK_HOME", value);
            }
        }
        assert_eq!(overridden, temp_dir.path().join("config.toml"));
        assert_eq!(
            fallback,
            dirs::home_dir().unwrap().join(".grok/config.toml")
        );
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
            Editor::Grok,
        ] {
            let name = editor.name();
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn test_generate_claude_code_config_new() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".claude.json");
        generate_config_at(global(Editor::ClaudeCode), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed["mcpServers"]["dela"],
            serde_json::json!({
                "type": "stdio",
                "command": dela_executable_path(),
                "args": ["mcp"]
            })
        );
    }

    #[test]
    fn test_generate_claude_code_config_preserves_unrelated_state() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".claude.json");
        fs::write(
            &config_path,
            r#"{"numStartups": 42, "projects": {"/tmp/x": {"allowedTools": []}}}"#,
        )
        .unwrap();

        generate_config_at(global(Editor::ClaudeCode), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(parsed["numStartups"], 42);
        assert_eq!(
            parsed["projects"]["/tmp/x"]["allowedTools"],
            serde_json::json!([])
        );
        assert_eq!(
            parsed["mcpServers"]["dela"]["args"],
            serde_json::json!(["mcp"])
        );
    }

    #[test]
    fn test_claude_code_merge_is_byte_faithful_to_unrelated_state() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".claude.json");
        // ~/.claude.json is a live state file: key order and float precision both have to
        // survive a merge (serde_json's "preserve_order" and "float_roundtrip" features).
        let original = r#"{"numStartups":7,"lastCost":0.41145139999999997,"tipsHistory":{"z":1,"a":2},"frame_ms":0.027125000022351742}"#;
        fs::write(&config_path, original).unwrap();

        generate_config_at(global(Editor::ClaudeCode), &config_path).unwrap();

        let merged = fs::read_to_string(&config_path).unwrap();
        assert!(
            merged.contains("0.41145139999999997"),
            "float was rounded: {merged}"
        );
        assert!(
            merged.contains("0.027125000022351742"),
            "float was rounded: {merged}"
        );
        let key_order: Vec<&str> = merged
            .match_indices('"')
            .step_by(2)
            .filter_map(|(i, _)| merged[i + 1..].split('"').next())
            .collect();
        let numstartups = key_order.iter().position(|k| *k == "numStartups");
        let tips = key_order.iter().position(|k| *k == "tipsHistory");
        assert!(
            numstartups < tips,
            "top-level key order was resorted: {merged}"
        );
        assert!(
            merged.find("\"z\"") < merged.find("\"a\""),
            "nested key order was resorted: {merged}"
        );
    }

    #[test]
    fn test_generate_opencode_config_uses_local_command_array() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("opencode.json");
        generate_config_at(global(Editor::OpenCode), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            parsed["mcp"]["dela"],
            serde_json::json!({
                "type": "local",
                "command": [dela_executable_path(), "mcp"]
            })
        );
    }

    #[test]
    fn test_opencode_repairs_legacy_command_args_entry() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("opencode.json");
        let alternate_dela = temp_dir.path().join("installed/dela");
        fs::create_dir_all(alternate_dela.parent().unwrap()).unwrap();
        fs::write(&alternate_dela, "").unwrap();

        // The shape dela wrote before: string command plus a rejected "args" key.
        fs::write(
            &config_path,
            format!(
                r#"{{"mcp":{{"dela":{{"command":"{}","args":["mcp"]}}}}}}"#,
                alternate_dela.display()
            ),
        )
        .unwrap();

        generate_config_at(global(Editor::OpenCode), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        let entry = parsed["mcp"]["dela"].as_object().unwrap();
        assert!(!entry.contains_key("args"));
        assert_eq!(entry["type"], "local");
        // A valid alternate install path is still preserved, just re-encoded.
        assert_eq!(
            entry["command"],
            serde_json::json!([alternate_dela.display().to_string(), "mcp"])
        );
    }

    #[test]
    fn test_generate_crush_config_new() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("crush.json");
        generate_config_at(global(Editor::Crush), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert!(parsed.get("mcpServers").is_none());
        assert_eq!(
            parsed["mcp"]["dela"],
            serde_json::json!({
                "type": "stdio",
                "command": dela_executable_path(),
                "args": ["mcp"]
            })
        );
    }

    #[test]
    fn test_crush_drops_legacy_mcp_servers_key() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("crush.json");
        fs::write(
            &config_path,
            format!(
                r#"{{"mcpServers":{{"dela":{{"command":"{}","args":["mcp"]}}}}}}"#,
                dela_executable_path()
            ),
        )
        .unwrap();

        generate_config_at(global(Editor::Crush), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        // Crush's root is additionalProperties:false, so the stale key has to go.
        assert!(parsed.get("mcpServers").is_none());
        assert_eq!(parsed["mcp"]["dela"]["type"], "stdio");
    }

    #[test]
    #[serial_test::serial]
    fn test_crush_requires_manual_migration_without_writing() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("crush.json");
        for legacy in [
            serde_json::json!({"dela": {"command": "dela"}, "other": {"command": "other"}}),
            serde_json::json!({"other": {"command": "other"}}),
            serde_json::json!("invalid"),
        ] {
            let original = serde_json::json!({"mcpServers": legacy}).to_string();
            fs::write(&config_path, &original).unwrap();

            let error = merge_editor_config(global(Editor::Crush), &original).unwrap_err();
            assert!(error.to_string().contains("Manually migrate"));
            generate_config_at(global(Editor::Crush), &config_path).unwrap();

            assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
        }
    }

    #[test]
    fn test_vscode_entry_keeps_stdio_type() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("Code/User/mcp.json");
        generate_config_at(global(Editor::Vscode), &config_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(parsed["servers"]["dela"]["type"], "stdio");
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

        let result = execute(None, Some(Editor::Cursor)).await;
        if let Err(ref e) = result {
            panic!("execute failed with error: {:?}", e);
        }
        assert!(result.is_ok());

        let expected_path = temp_dir.path().join(".cursor/mcp.json");
        assert!(expected_path.exists());
    }
}
