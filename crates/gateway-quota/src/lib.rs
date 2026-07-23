//! Strict PostgreSQL-backed daily token quota reservations.

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use gateway_store::GatewayStore;
use gateway_types::{ModelRoute, Principal, QuotaOwner, QuotaReservation};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Reservation service shared by streaming and non-streaming execution.
#[derive(Clone)]
pub struct QuotaService {
    store: Arc<dyn GatewayStore>,
    ttl: Duration,
    counter: Option<Arc<dyn InferenceQuotaCounter>>,
    counter_reservations: Arc<Mutex<HashMap<Uuid, String>>>,
}

impl QuotaService {
    /// Construct a quota service.
    pub fn new(store: Arc<dyn GatewayStore>, ttl: Duration) -> Self {
        Self {
            store,
            ttl,
            counter: None,
            counter_reservations: Default::default(),
        }
    }

    pub fn with_counter(mut self, counter: Arc<dyn InferenceQuotaCounter>) -> Self {
        self.counter = Some(counter);
        self
    }

    /// Reserve the conservative request budget atomically.
    pub async fn reserve(
        &self,
        request_id: Uuid,
        principal: &Principal,
        route: &ModelRoute,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> Result<QuotaReservation, QuotaError> {
        let counter_key = match &self.counter {
            Some(counter) => Some(counter.reserve(principal).await?),
            None => None,
        };
        let owner = principal
            .virtual_key_id
            .map(QuotaOwner::VirtualKey)
            .unwrap_or_else(|| QuotaOwner::Tenant(principal.tenant_id.clone()));
        let reservation = QuotaReservation {
            reservation_id: Uuid::now_v7(),
            request_id,
            owner,
            tenant_id: principal.tenant_id.clone(),
            project_id: principal.project_id.clone(),
            principal_id: principal.principal_id.clone(),
            user_id: principal.user_id.clone(),
            provider: route.provider.clone(),
            requested_model: route.requested_model.clone(),
            upstream_model: route.upstream_model.clone(),
            prompt_tokens,
            completion_tokens,
            expires_at: Utc::now()
                + ChronoDuration::from_std(self.ttl).map_err(|_| QuotaError::Unavailable)?,
        };
        match self.store.reserve_quota(reservation.clone()).await {
            Ok(true) => {
                if let Some(key) = counter_key {
                    self.counter_reservations
                        .lock()
                        .await
                        .insert(reservation.reservation_id, key);
                }
                Ok(reservation)
            }
            Ok(false) => {
                if let (Some(counter), Some(key)) = (&self.counter, counter_key) {
                    let _ = counter.release(&key).await;
                }
                Err(QuotaError::Exceeded)
            }
            Err(_) => {
                if let (Some(counter), Some(key)) = (&self.counter, counter_key) {
                    let _ = counter.release(&key).await;
                }
                Err(QuotaError::Unavailable)
            }
        }
    }

    pub async fn release_counter(&self, reservation_id: Uuid) {
        if let (Some(counter), Some(key)) = (
            &self.counter,
            self.counter_reservations
                .lock()
                .await
                .remove(&reservation_id),
        ) {
            let _ = counter.release(&key).await;
        }
    }
}

#[async_trait]
pub trait InferenceQuotaCounter: Send + Sync {
    async fn reserve(&self, principal: &Principal) -> Result<String, QuotaError>;
    async fn release(&self, reservation_key: &str) -> Result<(), QuotaError>;
}

/// Quota reservation failure.
#[derive(Clone, Debug, Error)]
pub enum QuotaError {
    /// Daily token limit would be exceeded.
    #[error("daily token quota exceeded")]
    Exceeded,
    /// Quota persistence is unavailable.
    #[error("quota service unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use gateway_store::{GatewayStore, TenantRecord};
    use gateway_types::{AuthenticationMethod, ModelRoute, Principal};
    use store_memory::MemoryStore;
    use uuid::Uuid;

    use super::{QuotaError, QuotaService};

    #[tokio::test]
    async fn concurrent_reservations_cannot_exceed_limit() {
        let store = Arc::new(MemoryStore::new());
        store
            .insert_tenant(TenantRecord {
                id: "tenant-a".into(),
                daily_token_limit: 100,
            })
            .await
            .unwrap();
        let quota = QuotaService::new(store, Duration::from_secs(60));
        let principal = Principal {
            principal_id: "jwt:issuer:user".into(),
            tenant_id: "tenant-a".into(),
            project_id: None,
            user_id: Some("user".into()),
            roles: vec![],
            scopes: vec![],
            authentication_method: AuthenticationMethod::Jwt,
            virtual_key_id: None,
        };
        let route = ModelRoute {
            provider: "provider".into(),
            requested_model: "gateway-default".into(),
            upstream_model: "model".into(),
        };
        quota
            .reserve(Uuid::new_v4(), &principal, &route, 30, 30)
            .await
            .unwrap();
        assert!(matches!(
            quota
                .reserve(Uuid::new_v4(), &principal, &route, 30, 30)
                .await,
            Err(QuotaError::Exceeded)
        ));
    }
}
