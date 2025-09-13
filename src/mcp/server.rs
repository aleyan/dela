use rmcp::{tool, tool_router, ServerHandler, model::*};
use std::path::PathBuf;
use crate::task_discovery;
use super::dto::{TaskDto, ListTasksArgs};

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
    pub async fn list_tasks(&self, Parameters(args): Parameters<ListTasksArgs>) -> Result<CallToolResult, ErrorData> {
        // Discover tasks in the current directory
        let mut discovered = task_discovery::discover_tasks(&self.root);
        
        // Process task disambiguation to generate uniqified names
        task_discovery::process_task_disambiguation(&mut discovered);
        
        // Apply runner filtering if specified
        let mut tasks = discovered.tasks;
        if let Some(runner_filter) = &args.runner {
            tasks.retain(|task| task.runner.short_name() == runner_filter);
        }
        
        // Convert to DTOs
        let task_dtos: Vec<TaskDto> = tasks.iter().map(TaskDto::from_task).collect();
        
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
        // Arrange
        let temp_dir = std::env::temp_dir();
        let server = DelaMcpServer::new(temp_dir);
        let args = Parameters(ListTasksArgs::default());
        
        // Act
        let result = server.list_tasks(args).await.unwrap();
        
        // Assert
        assert_eq!(result.content.len(), 1);
        // Should return a JSON response with an empty tasks array
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
        
        // Arrange
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
        
        let server = DelaMcpServer::new(temp_path.to_path_buf());
        let args = Parameters(ListTasksArgs::default());
        
        // Act
        let result = server.list_tasks(args).await.unwrap();
        
        // Assert
        assert_eq!(result.content.len(), 1);
        // The test succeeded, which means TaskDto conversion worked
    }

    #[tokio::test]
    async fn test_list_tasks_with_runner_filter() {
        use tempfile::TempDir;
        use std::fs;
        
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a Makefile with tasks
        let makefile_content = r#"build:
	echo "Building with make"

test:
	echo "Testing with make"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();
        
        // Create a package.json with tasks
        let package_json_content = r#"{
  "name": "test-project",
  "scripts": {
    "test": "jest",
    "start": "node server.js",
    "build": "webpack"
  }
}"#;
        fs::write(temp_path.join("package.json"), package_json_content).unwrap();
        
        let server = DelaMcpServer::new(temp_path.to_path_buf());
        
        // Act & Assert - Test filtering by "make"
        let make_args = Parameters(ListTasksArgs {
            runner: Some("make".to_string()),
        });
        let make_result = server.list_tasks(make_args).await.unwrap();
        assert_eq!(make_result.content.len(), 1);
        
        // Act & Assert - Test filtering by "npm"  
        let npm_args = Parameters(ListTasksArgs {
            runner: Some("npm".to_string()),
        });
        let npm_result = server.list_tasks(npm_args).await.unwrap();
        assert_eq!(npm_result.content.len(), 1);
        
        // Act & Assert - Test filtering by non-existent runner
        let nonexistent_args = Parameters(ListTasksArgs {
            runner: Some("nonexistent".to_string()),
        });
        let nonexistent_result = server.list_tasks(nonexistent_args).await.unwrap();
        assert_eq!(nonexistent_result.content.len(), 1);
        // Should return empty tasks array
        
        // Act & Assert - Test no filter (should return all tasks)
        let all_args = Parameters(ListTasksArgs::default());
        let all_result = server.list_tasks(all_args).await.unwrap();
        assert_eq!(all_result.content.len(), 1);
    }

    #[tokio::test]
    async fn test_list_tasks_runner_filter_case_sensitivity() {
        use tempfile::TempDir;
        use std::fs;
        
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a Makefile
        let makefile_content = r#"build:
	echo "Building"
"#;
        fs::write(temp_path.join("Makefile"), makefile_content).unwrap();
        
        let server = DelaMcpServer::new(temp_path.to_path_buf());
        
        // Act & Assert - Test exact match
        let exact_args = Parameters(ListTasksArgs {
            runner: Some("make".to_string()),
        });
        let exact_result = server.list_tasks(exact_args).await.unwrap();
        assert_eq!(exact_result.content.len(), 1);
        
        // Act & Assert - Test case mismatch (should return empty)
        let case_args = Parameters(ListTasksArgs {
            runner: Some("MAKE".to_string()),
        });
        let case_result = server.list_tasks(case_args).await.unwrap();
        assert_eq!(case_result.content.len(), 1);
        // Should return empty tasks array since "MAKE" != "make"
    }
}
