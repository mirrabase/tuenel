use chrono::{DateTime, Utc};
use gateway_types::{McpServerId, McpTransportType, Metadata, SecretRef};

#[derive(Clone, Debug)]
pub struct McpServerRecord {
    pub server_id: McpServerId,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub transport_type: McpTransportType,
    pub endpoint: Option<String>,
    pub command: Option<String>,
    pub arguments: Vec<String>,
    pub environment_secret_refs: Vec<SecretRef>,
    pub credential_ref: Option<SecretRef>,
    pub enabled: bool,
    pub metadata: Metadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl McpServerRecord {
    pub fn owned_by(&self, tenant_id: &str, project_id: Option<&str>) -> bool {
        self.tenant_id == tenant_id
            && self
                .project_id
                .as_deref()
                .is_none_or(|owner| Some(owner) == project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ownership_requires_tenant_and_project() {
        let now = Utc::now();
        let server = McpServerRecord {
            server_id: McpServerId::new(),
            tenant_id: "a".into(),
            project_id: Some("p".into()),
            name: "x".into(),
            description: None,
            transport_type: McpTransportType::Stdio,
            endpoint: None,
            command: Some("node".into()),
            arguments: vec![],
            environment_secret_refs: vec![],
            credential_ref: None,
            enabled: true,
            metadata: Default::default(),
            created_at: now,
            updated_at: now,
        };
        assert!(server.owned_by("a", Some("p")));
        assert!(!server.owned_by("a", Some("q")));
        assert!(!server.owned_by("b", Some("p")));
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SafeMcpServer {
    pub server_id: McpServerId,
    pub name: String,
    pub description: Option<String>,
    pub transport_type: McpTransportType,
    pub enabled: bool,
    pub health: Option<McpHealth>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct McpHealth {
    pub status: McpHealthStatus,
    pub latency_ms: Option<u64>,
    pub checked_at: DateTime<Utc>,
}
