mod allowlist;
mod dto;
mod errors;
mod job_manager;
mod server;

pub use dto::{
    ListTasksArgs, StartResultDto, TaskOutputArgs, TaskStartArgs, TaskStatusArgs, TaskStopArgs,
};
pub use errors::DelaError;
pub use server::DelaMcpServer;

/// Convenience runner for the CLI subcommand to ensure we actually
/// serve MCP over stdio (no stdout noise).
pub async fn run_stdio_server(root: std::path::PathBuf) -> Result<(), rmcp::model::ErrorData> {
    DelaMcpServer::new(root).serve_stdio().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::ServerHandler;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_server_info() {
        let server = DelaMcpServer::new(PathBuf::from("."));
        let info = server.get_info();

        assert_eq!(info.server_info.name, "dela-mcp");
        assert!(info.capabilities.tools.is_some());
        // Logging disabled in Phase 10A
        assert!(info.capabilities.logging.is_none());
        // Resources disabled in Phase 10A
        assert!(info.capabilities.resources.is_none());
    }

    #[tokio::test]
    async fn test_server_root_path() {
        let test_path = PathBuf::from("/test/path");
        let server = DelaMcpServer::new(test_path.clone());
        assert_eq!(server.root(), &test_path);
    }
}
