use std::sync::Arc;

use async_trait::async_trait;
use gateway_types::{
    GatewayMcpInvocation, GatewayMcpResult, GatewayMcpTool, McpServerId, Principal,
};
use tracing::Instrument;

use crate::{
    McpError, McpHealth, McpHealthRepository, McpServerRecord, McpTransportResolver, SafeMcpServer,
    SchemaLimits, SessionManager, ToolCache, validate_tools,
};

#[async_trait]
pub trait McpRepository: McpHealthRepository + Send + Sync {
    async fn insert_server(&self, server: McpServerRecord) -> Result<(), McpError>;
    async fn update_server(&self, server: McpServerRecord) -> Result<(), McpError>;
    async fn delete_server(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
    ) -> Result<bool, McpError>;
    async fn server(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
    ) -> Result<Option<McpServerRecord>, McpError>;
    async fn servers(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
    ) -> Result<Vec<McpServerRecord>, McpError>;
    async fn replace_tools(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
        tools: Vec<(GatewayMcpTool, String)>,
    ) -> Result<(), McpError>;
    async fn tools(
        &self,
        tenant_id: &str,
        server_id: Option<McpServerId>,
    ) -> Result<Vec<GatewayMcpTool>, McpError>;
}

#[derive(Clone)]
pub struct McpRegistry {
    repository: Arc<dyn McpRepository>,
    resolver: Arc<dyn McpTransportResolver>,
    sessions: SessionManager,
    cache: ToolCache,
    schema_limits: SchemaLimits,
}

impl McpRegistry {
    pub fn new(
        repository: Arc<dyn McpRepository>,
        resolver: Arc<dyn McpTransportResolver>,
        cache: ToolCache,
        schema_limits: SchemaLimits,
    ) -> Self {
        Self {
            repository,
            resolver,
            sessions: SessionManager::default(),
            cache,
            schema_limits,
        }
    }

    pub async fn validate(&self, server: &McpServerRecord) -> Result<(), McpError> {
        self.resolver.validate(server).await
    }
    pub async fn register(&self, server: McpServerRecord) -> Result<(), McpError> {
        self.validate(&server).await?;
        self.repository.insert_server(server).await?;
        gateway_observability::metrics().mcp_servers.inc();
        Ok(())
    }
    pub async fn update(&self, server: McpServerRecord) -> Result<(), McpError> {
        self.validate(&server).await?;
        self.cache.invalidate(server.server_id).await;
        if let Some(transport) = self.sessions.remove(server.server_id).await {
            let _ = transport.shutdown().await;
        }
        self.repository.update_server(server).await
    }
    pub async fn delete(&self, tenant_id: &str, server_id: McpServerId) -> Result<bool, McpError> {
        self.cache.invalidate(server_id).await;
        if let Some(transport) = self.sessions.remove(server_id).await {
            let _ = transport.shutdown().await;
        }
        self.repository.delete_server(tenant_id, server_id).await
    }

    pub async fn server_for(
        &self,
        principal: &Principal,
        server_id: McpServerId,
    ) -> Result<McpServerRecord, McpError> {
        self.repository
            .server(&principal.tenant_id, server_id)
            .await?
            .filter(|server| {
                server.enabled
                    && server.owned_by(&principal.tenant_id, principal.project_id.as_deref())
            })
            .ok_or(McpError::ServerNotFound)
    }
    pub async fn admin_server_for(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
    ) -> Result<McpServerRecord, McpError> {
        self.repository
            .server(tenant_id, server_id)
            .await?
            .ok_or(McpError::ServerNotFound)
    }

    pub async fn admin_safe_servers(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<SafeMcpServer>, McpError> {
        let servers = self.repository.servers(tenant_id, None).await?;
        let mut output = Vec::with_capacity(servers.len());
        for server in servers {
            let health = self
                .repository
                .latest_health(tenant_id, server.server_id)
                .await?;
            output.push(SafeMcpServer {
                server_id: server.server_id,
                name: server.name,
                description: server.description,
                transport_type: server.transport_type,
                enabled: server.enabled,
                health,
            })
        }
        Ok(output)
    }

    pub async fn safe_servers(
        &self,
        principal: &Principal,
    ) -> Result<Vec<SafeMcpServer>, McpError> {
        let servers = self
            .repository
            .servers(&principal.tenant_id, principal.project_id.as_deref())
            .await?;
        let mut output = Vec::with_capacity(servers.len());
        for server in servers.into_iter().filter(|server| {
            server.enabled && server.owned_by(&principal.tenant_id, principal.project_id.as_deref())
        }) {
            let health = self
                .repository
                .latest_health(&principal.tenant_id, server.server_id)
                .await?;
            output.push(SafeMcpServer {
                server_id: server.server_id,
                name: server.name,
                description: server.description,
                transport_type: server.transport_type,
                enabled: server.enabled,
                health,
            });
        }
        Ok(output)
    }

    pub async fn refresh(
        &self,
        principal: &Principal,
        server_id: McpServerId,
    ) -> Result<Vec<GatewayMcpTool>, McpError> {
        let server = self.server_for(principal, server_id).await?;
        let (session, transport) = self.session(&server).await?;
        let tools = transport.list_tools(&session).instrument(tracing::info_span!("gateway.mcp.discover_tools",tenant_id=%principal.tenant_id,mcp_server_id=%server_id)).await?;
        let validated = validate_tools(&tools, self.schema_limits)?;
        let safe = validated
            .iter()
            .map(|(tool, _)| tool.clone())
            .collect::<Vec<_>>();
        self.repository
            .replace_tools(&principal.tenant_id, server_id, validated)
            .await?;
        self.cache.put(server_id, safe.clone()).await;
        Ok(safe)
    }

    pub async fn tools(
        &self,
        principal: &Principal,
        server_id: Option<McpServerId>,
    ) -> Result<Vec<GatewayMcpTool>, McpError> {
        if let Some(server_id) = server_id {
            self.server_for(principal, server_id).await?;
            if let Some(tools) = self.cache.get(server_id).await {
                return Ok(tools);
            }
        }
        let tools = self
            .repository
            .tools(&principal.tenant_id, server_id)
            .await?;
        if server_id.is_some() {
            return Ok(tools);
        }
        let allowed = self
            .repository
            .servers(&principal.tenant_id, principal.project_id.as_deref())
            .await?
            .into_iter()
            .filter(|server| {
                server.enabled
                    && server.owned_by(&principal.tenant_id, principal.project_id.as_deref())
            })
            .map(|server| server.server_id)
            .collect::<std::collections::HashSet<_>>();
        Ok(tools
            .into_iter()
            .filter(|tool| allowed.contains(&tool.server_id))
            .collect())
    }

    pub async fn health(
        &self,
        principal: &Principal,
        server_id: McpServerId,
    ) -> Result<McpHealth, McpError> {
        let server = self.server_for(principal, server_id).await?;
        let (transport, _) = self.resolver.resolve(&server).await?;
        let health = transport.health_check().await?;
        self.repository
            .record_health(&principal.tenant_id, server_id, health.clone())
            .await?;
        health_metric(health.status);
        Ok(health)
    }

    pub async fn invoke(
        &self,
        principal: &Principal,
        invocation: GatewayMcpInvocation,
    ) -> Result<GatewayMcpResult, McpError> {
        let server = self.server_for(principal, invocation.server_id).await?;
        let (session, transport) = self.session(&server).await?;
        let started = std::time::Instant::now();
        let result = transport.invoke_tool(&session, invocation).await;
        let status = if result.is_ok() {
            crate::McpHealthStatus::Healthy
        } else {
            crate::McpHealthStatus::Unhealthy
        };
        health_metric(status);
        if let Err(error) = self
            .repository
            .record_health(
                &principal.tenant_id,
                server.server_id,
                crate::McpHealth {
                    status,
                    latency_ms: Some(started.elapsed().as_millis() as u64),
                    checked_at: chrono::Utc::now(),
                },
            )
            .await
        {
            tracing::warn!(server_id=%server.server_id, error=%error, "failed to persist MCP health");
        }
        result
    }

    async fn session(
        &self,
        server: &McpServerRecord,
    ) -> Result<(crate::McpSession, Arc<dyn crate::McpTransport>), McpError> {
        if let Some(value) = self.sessions.get(server.server_id).await {
            return Ok(value);
        }
        let (transport, context) = self.resolver.resolve(server).await?;
        let session = transport.initialize(context).await?;
        self.sessions
            .insert(session.clone(), transport.clone())
            .await;
        Ok((session, transport))
    }
}

fn health_metric(status: crate::McpHealthStatus) {
    let metrics = gateway_observability::metrics();
    for value in ["healthy", "degraded", "unhealthy", "unknown"] {
        metrics.mcp_health.with_label_values(&[value]).set(0)
    }
    let value = match status {
        crate::McpHealthStatus::Healthy => "healthy",
        crate::McpHealthStatus::Degraded => "degraded",
        crate::McpHealthStatus::Unhealthy => "unhealthy",
        crate::McpHealthStatus::Unknown => "unknown",
    };
    metrics.mcp_health.with_label_values(&[value]).set(1)
}
