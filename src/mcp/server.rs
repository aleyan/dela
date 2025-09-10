use rmcp::{tool, tool_router, ServerHandler, model::*};
use std::path::PathBuf;

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
        // Stub implementation - will be filled in with DTKT-144
        Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({
            "tasks": []
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
        let server = DelaMcpServer::new(PathBuf::from("."));
        let result = server.list_tasks().await.unwrap();
        
        // Since Content::json returns Result, we just check if the call succeeded
        // The actual JSON structure will be tested when we implement real functionality
        assert_eq!(result.content.len(), 1);
    }

    #[tokio::test]
    async fn test_unimplemented_tools() {
        let server = DelaMcpServer::new(PathBuf::from("."));
        
        assert!(server.get_task().await.is_err());
        assert!(server.get_command().await.is_err());
        assert!(server.run_task().await.is_err());
        assert!(server.read_allowlist().await.is_err());
    }
}
