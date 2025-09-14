use rmcp::{tool, tool_router, ServerHandler, model::*, handler::server::wrapper::Parameters};
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

    #[tool(description = "List all running tasks with PIDs")]
    pub async fn status(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - Phase 10A returns empty array
        Ok(CallToolResult::success(vec![Content::json(&serde_json::json!({
            "running": []
        })).expect("Failed to serialize JSON")]))
    }

    #[tool(description = "Start a task (≤1s capture, then background)")]
    pub async fn task_start(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with DTKT-163
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }

    #[tool(description = "Status for a single unique_name (may have multiple PIDs)")]
    pub async fn task_status(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with Phase 10B
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }

    #[tool(description = "Tail last N lines for a PID")]
    pub async fn task_output(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with Phase 10B
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }

    #[tool(description = "Stop a PID with graceful timeout")]
    pub async fn task_stop(&self) -> Result<CallToolResult, ErrorData> {
        // Stub implementation - will be filled in with Phase 10B
        Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, "Not implemented", None))
    }
}

impl ServerHandler for DelaMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_logging()
                .build(),
            server_info: Implementation {
                name: "dela-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(
                "List tasks, start them (≤1s capture then background), and manage running tasks via PID; all execution gated by an MCP allowlist."
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
        
        assert!(server.task_start().await.is_err());
        assert!(server.task_status().await.is_err());
        assert!(server.task_output().await.is_err());
        assert!(server.task_stop().await.is_err());
        
        // Status should work (returns empty array in Phase 10A)
        assert!(server.status().await.is_ok());
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
