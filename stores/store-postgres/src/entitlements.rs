use async_trait::async_trait;
use gateway_entitlements::{
    Capability, EntitlementError, EntitlementGrant, EntitlementReason, EntitlementStateRepository,
};
use sqlx::Row;
use uuid::Uuid;

use crate::PostgresStore;

#[async_trait]
impl EntitlementStateRepository for PostgresStore {
    async fn installation_id(&self) -> Result<Uuid, EntitlementError> {
        sqlx::query_scalar("SELECT installation_id FROM installation_state WHERE singleton = true")
            .fetch_one(&self.pool)
            .await
            .map_err(|_| EntitlementError::Unavailable)
    }

    async fn grant(
        &self,
        tenant_id: Option<&str>,
        capability: Capability,
    ) -> Result<Option<EntitlementGrant>, EntitlementError> {
        sqlx::query(
            "SELECT tenant_id, capability, enabled, valid_until, reason_code, source, \
             provider_updated_at FROM entitlement_grants \
             WHERE tenant_id IS NOT DISTINCT FROM $1 AND capability = $2",
        )
        .bind(tenant_id)
        .bind(capability.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| EntitlementError::Unavailable)?
        .map(|row| {
            Ok(EntitlementGrant {
                tenant_id: row
                    .try_get("tenant_id")
                    .map_err(|_| EntitlementError::Invalid)?,
                capability: parse_capability(
                    &row.try_get::<String, _>("capability")
                        .map_err(|_| EntitlementError::Invalid)?,
                )?,
                enabled: row
                    .try_get("enabled")
                    .map_err(|_| EntitlementError::Invalid)?,
                valid_until: row
                    .try_get("valid_until")
                    .map_err(|_| EntitlementError::Invalid)?,
                reason_code: parse_reason(
                    &row.try_get::<String, _>("reason_code")
                        .map_err(|_| EntitlementError::Invalid)?,
                )?,
                source: row
                    .try_get("source")
                    .map_err(|_| EntitlementError::Invalid)?,
                provider_updated_at: row
                    .try_get("provider_updated_at")
                    .map_err(|_| EntitlementError::Invalid)?,
            })
        })
        .transpose()
    }

    async fn upsert_grant(&self, grant: &EntitlementGrant) -> Result<(), EntitlementError> {
        sqlx::query(
            "INSERT INTO entitlement_grants \
             (grant_id, tenant_id, capability, enabled, valid_until, reason_code, source, provider_updated_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8) \
             ON CONFLICT ((COALESCE(tenant_id, '')), capability) DO UPDATE SET \
             enabled=EXCLUDED.enabled, valid_until=EXCLUDED.valid_until, \
             reason_code=EXCLUDED.reason_code, source=EXCLUDED.source, \
             provider_updated_at=EXCLUDED.provider_updated_at, updated_at=now()",
        )
        .bind(Uuid::now_v7())
        .bind(&grant.tenant_id)
        .bind(grant.capability.as_str())
        .bind(grant.enabled)
        .bind(grant.valid_until)
        .bind(grant.reason_code.as_str())
        .bind(&grant.source)
        .bind(grant.provider_updated_at)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|_| EntitlementError::Unavailable)
    }
}

fn parse_capability(value: &str) -> Result<Capability, EntitlementError> {
    match value {
        "browser_sso" => Ok(Capability::BrowserSso),
        "audit_export" => Ok(Capability::AuditExport),
        _ => Err(EntitlementError::Invalid),
    }
}

fn parse_reason(value: &str) -> Result<EntitlementReason, EntitlementError> {
    match value {
        "entitled" => Ok(EntitlementReason::Entitled),
        "community_edition" => Ok(EntitlementReason::CommunityEdition),
        "expired" => Ok(EntitlementReason::Expired),
        "disabled" => Ok(EntitlementReason::Disabled),
        "not_configured" => Ok(EntitlementReason::NotConfigured),
        "unavailable" => Ok(EntitlementReason::Unavailable),
        _ => Err(EntitlementError::Invalid),
    }
}
