use crate::McpError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_types::{McpUsageDetails, Principal};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct McpUsageEvent {
    pub event_id: Uuid,
    pub request_id: Uuid,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub principal_id: String,
    pub details: McpUsageDetails,
    pub status: String,
    pub occurred_at: DateTime<Utc>,
}

impl McpUsageEvent {
    pub fn new(
        request_id: Uuid,
        principal: &Principal,
        details: McpUsageDetails,
        status: impl Into<String>,
    ) -> Self {
        Self {
            event_id: Uuid::now_v7(),
            request_id,
            tenant_id: principal.tenant_id.clone(),
            project_id: principal.project_id.clone(),
            principal_id: principal.principal_id.clone(),
            details,
            status: status.into(),
            occurred_at: Utc::now(),
        }
    }
}

#[async_trait]
pub trait McpUsageRepository: Send + Sync {
    async fn record_mcp_usage(&self, event: McpUsageEvent) -> Result<(), McpError>;
}

#[derive(Clone, Debug)]
pub struct McpQuotaReservation {
    pub key: String,
}

#[async_trait]
pub trait McpQuota: Send + Sync {
    async fn reserve(
        &self,
        principal: &Principal,
        server_id: gateway_types::McpServerId,
        tool_name: &str,
        policy: &crate::McpPolicy,
    ) -> Result<McpQuotaReservation, McpError>;
    async fn release(&self, reservation: McpQuotaReservation) -> Result<(), McpError>;
}
