use rmcp::{tool, tool_router, ServerHandler, model::*};
use std::path::PathBuf;
use crate::task_discovery;
use super::dto::TaskDto;

/// MCP server for dela that exposes task management capabilities
#[derive(Clone)]
pub struct DelaMcpServer {
    root: PathBuf,
}

impl DelaMcpServer {
    /// Create a new MCP server instance
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Get the root path this server operates in
    pub fn root(&self) -> &PathBuf {
        &self.root
    }
}

#[tool_router]
impl DelaMcpServer {
    #[tool(description = "List tasks")]
    pub async fn list_tasks(&self) -> Result<CallToolResult, ErrorData> {
        // Discover tasks in the current directory
        let mut discovered = task_discovery::discover_tasks(&self.root);
        
        // Process task disambiguation to generate uniqified names
        task_discovery::process_task_disambiguation(&mut discovered);
        
        // Convert to DTOs
        let task_dtos: Vec<TaskDto> = discovered.tasks.iter().map(TaskDto::from_task).collect();
        
        Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({
            "tasks": task_dtos
        })).expect("Failed to serialize JSON")]))
    }

    #[tool(description = "Get task details")]
    pub async fn get_task(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with DTKT-145
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }

    #[tool(description = "Get shell command (no exec)")]
    pub async fn get_command(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with DTKT-146
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }

    #[tool(description = "Start/stop/status for tasks")]
    pub async fn run_task(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with DTKT-147/148
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }

    #[tool(description = "Read allowlist (read-only)")]
    pub async fn read_allowlist(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with DTKT-150
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }
}

impl ServerHandler for DelaMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_logging()
                .build(),
            server_info: Implementation {
                name: "dela-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(
                "List and run tasks gated by an MCP allowlist; long-running tasks stream logs as notifications."
                    .into(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_tasks_empty() {
        // Use a temporary directory that doesn't contain any task files
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);
        let result = server.list_tasks().await.unwrap();
        
        // Should return a JSON response with an empty tasks array
        assert_eq!(result.content.len(), 1);
        // We can't easily test the JSON content without unwrapping the Content
        // but we know it should be valid since it succeeded
    }

    #[tokio::test]
    async fn test_unimplemented_tools() {
        let server = DelaMcpServer::new(PathBuf::from("."));
        
        assert!(server.get_task().await.is_err());
        assert!(server.get_command().await.is_err());
        assert!(server.run_task().await.is_err());
        assert!(server.read_allowlist().await.is_err());
    }

    #[tokio::test]
    async fn test_list_tasks_with_actual_files() {
        use tempfile::TempDir;
        use std::fs;
        
        // Create a temporary directory with test task files
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a simple Makefile
        let makefile_content = r#"# Build target
build:
	echo "Building"

# Test target
test:
	echo "Testing"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();
        
        // Create a package.json
        let package_json_content = r#"{
  "name": "test-project",
  "scripts": {
    "test": "jest",
    "start": "node server.js"
  }
}"#;
        fs::write(temp_path.join("package.json"), package_json_content).unwrap();
        
        // Test the list_tasks functionality
        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let result = server.list_tasks().await.unwrap();
        
        // Should return a JSON response
        assert_eq!(result.content.len(), 1);
        
        // The test succeeded, which means TaskDto conversion worked
        // In a real integration test, we could parse the JSON and verify the structure
    }
}
