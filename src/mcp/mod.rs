use rmcp::{tool, tool_router, ServerHandler, model::*, service::{RequestContext, RoleServer}};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

mod server;
pub use server::DelaMcpServer;

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::service::ServerInfo;

    #[tokio::test]
    async fn test_server_info() {
        let server = DelaMcpServer::new(PathBuf::from("."));
        let info = server.get_info();
        
        assert_eq!(info.server_info.name, "dela-mcp");
        assert!(info.capabilities.tools);
        assert!(info.capabilities.resources);
        assert!(info.capabilities.logging);
        assert!(!info.capabilities.elicitation);
    }

    #[tokio::test]
    async fn test_server_root_path() {
        let test_path = PathBuf::from("/test/path");
        let server = DelaMcpServer::new(test_path.clone());
        assert_eq!(server.root(), &test_path);
    }
}
