//! Deterministic MCP transport test double.

use async_trait::async_trait;
use gateway_mcp::{
    McpConnectionContext, McpError, McpHealth, McpHealthStatus, McpSession, McpTransport,
};
use gateway_types::{GatewayMcpInvocation, GatewayMcpResult, GatewayMcpTool, McpContentPart};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct MockMcpTransport {
    pub tools: Arc<Vec<GatewayMcpTool>>,
    pub results: Arc<HashMap<String, GatewayMcpResult>>,
    pub calls: Arc<AtomicU64>,
    pub delay: Option<Duration>,
    pub fail: bool,
}

#[async_trait]
impl McpTransport for MockMcpTransport {
    async fn initialize(&self, context: McpConnectionContext) -> Result<McpSession, McpError> {
        Ok(McpSession {
            session_id: Uuid::now_v7(),
            server_id: context.server.server_id,
            protocol_version: "2025-11-25".into(),
            remote_session_id: None,
            created_at: chrono::Utc::now(),
            expires_at: None,
        })
    }
    async fn list_tools(&self, _: &McpSession) -> Result<Vec<GatewayMcpTool>, McpError> {
        Ok(self.tools.as_ref().clone())
    }
    async fn invoke_tool(
        &self,
        _: &McpSession,
        invocation: GatewayMcpInvocation,
    ) -> Result<GatewayMcpResult, McpError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        if self.fail {
            return Err(McpError::Transport);
        }
        Ok(self
            .results
            .get(&invocation.tool_name)
            .cloned()
            .unwrap_or(GatewayMcpResult {
                content: vec![McpContentPart::Text { text: "ok".into() }],
                is_error: false,
                metadata: Default::default(),
            }))
    }
    async fn health_check(&self) -> Result<McpHealth, McpError> {
        if self.fail {
            Err(McpError::Transport)
        } else {
            Ok(McpHealth {
                status: McpHealthStatus::Healthy,
                latency_ms: Some(0),
                checked_at: chrono::Utc::now(),
            })
        }
    }
    async fn shutdown(&self) -> Result<(), McpError> {
        Ok(())
    }
}
