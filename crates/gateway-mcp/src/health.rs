use crate::{McpError, McpHealth};
use async_trait::async_trait;
use gateway_types::McpServerId;

#[async_trait]
pub trait McpHealthRepository: Send + Sync {
    async fn record_health(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
        health: McpHealth,
    ) -> Result<(), McpError>;
    async fn latest_health(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
    ) -> Result<Option<McpHealth>, McpError>;
}
