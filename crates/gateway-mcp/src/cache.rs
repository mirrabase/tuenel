use chrono::{DateTime, Utc};
use gateway_types::{GatewayMcpTool, McpServerId};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ToolCache {
    ttl: Duration,
    #[allow(clippy::type_complexity)]
    values: Arc<RwLock<HashMap<McpServerId, (DateTime<Utc>, Vec<GatewayMcpTool>)>>>,
}
impl ToolCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            values: Default::default(),
        }
    }
    pub async fn get(&self, server_id: McpServerId) -> Option<Vec<GatewayMcpTool>> {
        self.values
            .read()
            .await
            .get(&server_id)
            .filter(|(stored, _)| {
                Utc::now()
                    .signed_duration_since(*stored)
                    .to_std()
                    .is_ok_and(|age| age < self.ttl)
            })
            .map(|(_, tools)| tools.clone())
    }
    pub async fn put(&self, server_id: McpServerId, tools: Vec<GatewayMcpTool>) {
        self.values
            .write()
            .await
            .insert(server_id, (Utc::now(), tools));
    }
    pub async fn invalidate(&self, server_id: McpServerId) {
        self.values.write().await.remove(&server_id);
    }
}
