use crate::McpTransport;
use chrono::{DateTime, Utc};
use gateway_types::McpServerId;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct McpSession {
    pub session_id: Uuid,
    pub server_id: McpServerId,
    pub protocol_version: String,
    pub remote_session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Default)]
pub struct SessionManager {
    #[allow(clippy::type_complexity)]
    sessions: Arc<RwLock<HashMap<McpServerId, (McpSession, Arc<dyn McpTransport>)>>>,
}

impl SessionManager {
    pub async fn get(&self, server_id: McpServerId) -> Option<(McpSession, Arc<dyn McpTransport>)> {
        self.sessions
            .read()
            .await
            .get(&server_id)
            .cloned()
            .filter(|(session, _)| {
                session
                    .expires_at
                    .is_none_or(|expires| expires > Utc::now())
            })
    }
    pub async fn insert(&self, session: McpSession, transport: Arc<dyn McpTransport>) {
        self.sessions
            .write()
            .await
            .insert(session.server_id, (session, transport));
    }
    pub async fn remove(&self, server_id: McpServerId) -> Option<Arc<dyn McpTransport>> {
        self.sessions
            .write()
            .await
            .remove(&server_id)
            .map(|(_, transport)| transport)
    }
}
