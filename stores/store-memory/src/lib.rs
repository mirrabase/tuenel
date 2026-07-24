//! Deterministic in-memory GatewayStore for tests and local unit composition.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use gateway_store::{GatewayStore, StoreError, TenantRecord};
use gateway_types::{QuotaReservation, UsageEvent, VirtualKeyRecord};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Non-durable store with the same observable contracts as PostgreSQL.
#[derive(Default)]
pub struct MemoryStore {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    tenants: HashMap<String, TenantRecord>,
    keys: HashMap<String, VirtualKeyRecord>,
    reservations: HashMap<Uuid, QuotaReservation>,
    usage: HashMap<Uuid, UsageEvent>,
}

impl MemoryStore {
    /// Construct an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl GatewayStore for MemoryStore {
    async fn ping(&self) -> Result<(), StoreError> {
        Ok(())
    }

    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<(), StoreError> {
        self.state
            .lock()
            .await
            .tenants
            .insert(tenant.id.clone(), tenant);
        Ok(())
    }

    async fn find_tenant(&self, tenant_id: &str) -> Result<Option<TenantRecord>, StoreError> {
        Ok(self.state.lock().await.tenants.get(tenant_id).cloned())
    }

    async fn insert_virtual_key(&self, key: VirtualKeyRecord) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        if state.keys.contains_key(&key.lookup_prefix) {
            return Err(StoreError::Conflict);
        }
        state.keys.insert(key.lookup_prefix.clone(), key);
        Ok(())
    }

    async fn find_virtual_key_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<VirtualKeyRecord>, StoreError> {
        Ok(self.state.lock().await.keys.get(prefix).cloned())
    }

    async fn touch_virtual_key(&self, _: Uuid) -> Result<(), StoreError> {
        Ok(())
    }

    async fn revoke_virtual_key(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        key_id: Uuid,
    ) -> Result<bool, StoreError> {
        let mut state = self.state.lock().await;
        let Some(key) = state.keys.values_mut().find(|key| {
            key.id == key_id
                && key.tenant_id == tenant_id
                && project_id.is_none_or(|project_id| key.project_id.as_deref() == Some(project_id))
        }) else {
            return Ok(false);
        };
        key.revoked_at.get_or_insert_with(Utc::now);
        Ok(true)
    }

    async fn reserve_quota(&self, reservation: QuotaReservation) -> Result<bool, StoreError> {
        let mut state = self.state.lock().await;
        let limit = match &reservation.owner {
            gateway_types::QuotaOwner::Tenant(id) => {
                state.tenants.get(id).map(|tenant| tenant.daily_token_limit)
            }
            gateway_types::QuotaOwner::VirtualKey(id) => state
                .keys
                .values()
                .find(|key| key.id == *id)
                .map(|key| key.daily_token_limit),
        }
        .ok_or(StoreError::NotFound)?;
        let now = Utc::now();
        let used: u64 = state
            .usage
            .values()
            .filter(|event| {
                event.tenant_id == reservation.tenant_id
                    && same_utc_day(event.occurred_at, now)
                    && match &reservation.owner {
                        gateway_types::QuotaOwner::Tenant(_) => {
                            !event.principal_id.starts_with("virtual-key:")
                        }
                        gateway_types::QuotaOwner::VirtualKey(id) => {
                            event.principal_id == format!("virtual-key:{id}")
                        }
                    }
            })
            .map(|event| event.usage.total_tokens())
            .sum();
        let pending: u64 = state
            .reservations
            .values()
            .filter(|pending| pending.owner == reservation.owner)
            .map(QuotaReservation::reserved_tokens)
            .sum();
        if used
            .saturating_add(pending)
            .saturating_add(reservation.reserved_tokens())
            > limit
        {
            return Ok(false);
        }
        state
            .reservations
            .insert(reservation.reservation_id, reservation);
        Ok(true)
    }

    async fn finalize_usage(
        &self,
        reservation_id: Uuid,
        event: UsageEvent,
    ) -> Result<(), StoreError> {
        let mut state = self.state.lock().await;
        state.reservations.remove(&reservation_id);
        state.usage.entry(event.request_id).or_insert(event);
        Ok(())
    }

    async fn release_reservation(&self, reservation_id: Uuid) -> Result<(), StoreError> {
        self.state.lock().await.reservations.remove(&reservation_id);
        Ok(())
    }

    async fn expired_reservations(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<QuotaReservation>, StoreError> {
        let state = self.state.lock().await;
        Ok(state
            .reservations
            .values()
            .filter(|reservation| reservation.expires_at <= now)
            .cloned()
            .collect())
    }

    async fn usage_by_request(&self, request_id: Uuid) -> Result<Option<UsageEvent>, StoreError> {
        Ok(self.state.lock().await.usage.get(&request_id).cloned())
    }
}

fn same_utc_day(left: DateTime<Utc>, right: DateTime<Utc>) -> bool {
    left.year() == right.year() && left.ordinal() == right.ordinal()
}
