//! Append-only, idempotent gateway audit events.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_types::Principal;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AuditEvent {
    pub event_id: Uuid,
    pub idempotency_key: String,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub principal_id: Option<String>,
    pub request_id: Option<Uuid>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Error)]
pub enum EventError {
    #[error("audit persistence unavailable")]
    Unavailable,
}

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn append(&self, event: AuditEvent) -> Result<(), EventError>;
}

#[derive(Clone)]
pub struct AuditService {
    repository: Arc<dyn AuditRepository>,
}

impl AuditService {
    pub fn new(repository: Arc<dyn AuditRepository>) -> Self {
        Self { repository }
    }
    pub async fn emit(
        &self,
        idempotency_key: impl Into<String>,
        event_type: impl Into<String>,
        principal: &Principal,
        request_id: Option<Uuid>,
        payload: serde_json::Value,
    ) -> Result<(), EventError> {
        self.repository
            .append(AuditEvent {
                event_id: Uuid::now_v7(),
                idempotency_key: idempotency_key.into(),
                tenant_id: principal.tenant_id.clone(),
                project_id: principal.project_id.clone(),
                principal_id: Some(principal.principal_id.clone()),
                request_id,
                event_type: event_type.into(),
                payload,
                occurred_at: Utc::now(),
            })
            .await
    }
}
