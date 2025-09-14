mod server;
mod dto;

pub use server::DelaMcpServer;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use rmcp::ServerHandler;

    #[tokio::test]
    async fn test_server_info() {
        let server = DelaMcpServer::new(PathBuf::from("."));
        let info = server.get_info();
        
        assert_eq!(info.server_info.name, "dela-mcp");
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.logging.is_some());
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
