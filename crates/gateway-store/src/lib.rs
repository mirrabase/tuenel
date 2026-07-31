//! Persistence boundary for gateway v0.1.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_types::{QuotaReservation, UsageEvent, VirtualKeyRecord};
use thiserror::Error;
use uuid::Uuid;

/// Tenant configuration required by authentication and quota enforcement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantRecord {
    /// Tenant identifier asserted by JWTs and keys.
    pub id: String,
    /// Daily token limit for JWT-authenticated traffic.
    pub daily_token_limit: u64,
}

/// Complete persistence contract used by the v0.1 application service.
#[async_trait]
pub trait GatewayStore: Send + Sync {
    /// Check backing-store health.
    async fn ping(&self) -> Result<(), StoreError>;
    /// Insert a tenant, primarily for provisioning and tests.
    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<(), StoreError>;
    /// Find a tenant.
    async fn find_tenant(&self, tenant_id: &str) -> Result<Option<TenantRecord>, StoreError>;
    /// Managed tenant-wide RPM ceiling. Self-hosted tenants return `None`.
    async fn plan_requests_per_minute(&self, tenant_id: &str) -> Result<Option<u64>, StoreError>;
    /// Current usage and ceiling for a managed resource, if this tenant is managed.
    async fn plan_resource_usage(
        &self,
        tenant_id: &str,
        resource: &str,
    ) -> Result<Option<(u64, u64)>, StoreError>;
    /// Managed feature decision. Self-hosted tenants return `None`.
    async fn plan_feature_enabled(
        &self,
        tenant_id: &str,
        feature: &str,
    ) -> Result<Option<bool>, StoreError>;
    /// Insert prepared Virtual Key metadata and hash.
    async fn insert_virtual_key(&self, key: VirtualKeyRecord) -> Result<(), StoreError>;
    /// Look up a Virtual Key by its non-secret prefix.
    async fn find_virtual_key_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<VirtualKeyRecord>, StoreError>;
    /// Record successful use without exposing or rewriting credential material.
    async fn touch_virtual_key(&self, key_id: Uuid) -> Result<(), StoreError>;
    /// Revoke a key owned by a tenant. Returns whether a matching key exists.
    async fn revoke_virtual_key(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        key_id: Uuid,
    ) -> Result<bool, StoreError>;
    /// Atomically reserve quota. Returns false when the daily limit would be exceeded.
    async fn reserve_quota(&self, reservation: QuotaReservation) -> Result<bool, StoreError>;
    /// Atomically append usage and finalize its reservation. Duplicate request IDs are idempotent.
    async fn finalize_usage(
        &self,
        reservation_id: Uuid,
        event: UsageEvent,
    ) -> Result<(), StoreError>;
    /// Release a reservation when no provider usage occurred.
    async fn release_reservation(&self, reservation_id: Uuid) -> Result<(), StoreError>;
    /// List expired reservations for conservative idempotent reconciliation.
    async fn expired_reservations(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<QuotaReservation>, StoreError>;
    /// Read an immutable event by request ID.
    async fn usage_by_request(&self, request_id: Uuid) -> Result<Option<UsageEvent>, StoreError>;
}

/// Sanitized persistence failure.
#[derive(Clone, Debug, Error)]
pub enum StoreError {
    /// A unique or state constraint was violated.
    #[error("persistence conflict")]
    Conflict,
    /// Required data was not found.
    #[error("persistence record not found")]
    NotFound,
    /// The backing store is unavailable or returned an unexpected error.
    #[error("persistence unavailable")]
    Unavailable,
}
