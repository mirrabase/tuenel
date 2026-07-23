use async_trait::async_trait;
use gateway_types::{GatewayMcpInvocation, GatewayMcpResult, GatewayMcpTool, SecretRef};

use crate::{McpError, McpHealth, McpServerRecord, McpSession};

#[derive(Clone)]
pub struct McpConnectionContext {
    pub server: McpServerRecord,
    pub credential: Option<SecretValue>,
    pub environment: Vec<(String, SecretValue)>,
}

#[derive(Clone)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: String) -> Self {
        Self(value)
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn initialize(&self, context: McpConnectionContext) -> Result<McpSession, McpError>;
    async fn list_tools(&self, session: &McpSession) -> Result<Vec<GatewayMcpTool>, McpError>;
    async fn invoke_tool(
        &self,
        session: &McpSession,
        invocation: GatewayMcpInvocation,
    ) -> Result<GatewayMcpResult, McpError>;
    async fn health_check(&self) -> Result<McpHealth, McpError>;
    async fn shutdown(&self) -> Result<(), McpError>;
}

#[async_trait]
pub trait McpTransportResolver: Send + Sync {
    async fn validate(&self, server: &McpServerRecord) -> Result<(), McpError>;
    async fn resolve(
        &self,
        server: &McpServerRecord,
    ) -> Result<(std::sync::Arc<dyn McpTransport>, McpConnectionContext), McpError>;
}

#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve_secret(
        &self,
        tenant_id: &str,
        secret_ref: &SecretRef,
    ) -> Result<SecretValue, McpError>;
    async fn resolve_environment(
        &self,
        tenant_id: &str,
        refs: &[SecretRef],
    ) -> Result<Vec<(String, SecretValue)>, McpError>;
}
