//! Provider-neutral capability and commercial-extension contracts.
//!
//! This crate intentionally sits outside `gateway-core`: inference does not
//! depend on an edition, a billing provider, or online license availability.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Product topology metadata. It does not itself grant a capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    Community,
    Enterprise,
    Managed,
}

/// Capabilities that extensions may grant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    BrowserSso,
    AuditExport,
}

impl Capability {
    pub const ALL: [Self; 2] = [Self::BrowserSso, Self::AuditExport];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BrowserSso => "browser_sso",
            Self::AuditExport => "audit_export",
        }
    }
}

/// Scope used when resolving a capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitlementContext {
    pub tenant_id: Option<String>,
    pub installation_id: Uuid,
}

/// Stable, non-sensitive reason exposed to callers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementReason {
    Entitled,
    CommunityEdition,
    Expired,
    Disabled,
    NotConfigured,
    Unavailable,
}

impl EntitlementReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entitled => "entitled",
            Self::CommunityEdition => "community_edition",
            Self::Expired => "expired",
            Self::Disabled => "disabled",
            Self::NotConfigured => "not_configured",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Result of an authoritative capability check.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EntitlementDecision {
    pub enabled: bool,
    pub valid_until: Option<DateTime<Utc>>,
    pub reason_code: EntitlementReason,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EntitlementError {
    #[error("feature is not entitled")]
    FeatureNotEntitled,
    #[error("entitlement state is unavailable")]
    Unavailable,
    #[error("invalid entitlement state")]
    Invalid,
}

/// Edition-neutral boundary injected into application services and transports.
#[async_trait]
pub trait EntitlementProvider: Send + Sync {
    async fn decision(
        &self,
        context: &EntitlementContext,
        capability: Capability,
    ) -> Result<EntitlementDecision, EntitlementError>;
}

/// Application service used by premium operations before doing any work.
#[derive(Clone)]
pub struct EntitlementService {
    provider: Arc<dyn EntitlementProvider>,
    installation_id: Uuid,
}

impl EntitlementService {
    pub fn new(provider: Arc<dyn EntitlementProvider>, installation_id: Uuid) -> Self {
        Self {
            provider,
            installation_id,
        }
    }

    pub async fn require(
        &self,
        tenant_id: Option<String>,
        capability: Capability,
    ) -> Result<EntitlementDecision, EntitlementError> {
        let decision = self
            .provider
            .decision(
                &EntitlementContext {
                    tenant_id,
                    installation_id: self.installation_id,
                },
                capability,
            )
            .await?;
        decision
            .enabled
            .then_some(decision)
            .ok_or(EntitlementError::FeatureNotEntitled)
    }
}

/// Offline Community implementation. It never performs I/O or a network call.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommunityEntitlements;

#[async_trait]
impl EntitlementProvider for CommunityEntitlements {
    async fn decision(
        &self,
        _context: &EntitlementContext,
        _capability: Capability,
    ) -> Result<EntitlementDecision, EntitlementError> {
        Ok(EntitlementDecision {
            enabled: false,
            valid_until: None,
            reason_code: EntitlementReason::CommunityEdition,
        })
    }
}

/// Durable, provider-neutral grant used by managed and enterprise adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitlementGrant {
    pub tenant_id: Option<String>,
    pub capability: Capability,
    pub enabled: bool,
    pub valid_until: Option<DateTime<Utc>>,
    pub reason_code: EntitlementReason,
    pub source: String,
    pub provider_updated_at: Option<DateTime<Utc>>,
}

/// Public persistence boundary required by entitlement providers.
#[async_trait]
pub trait EntitlementStateRepository: Send + Sync {
    async fn installation_id(&self) -> Result<Uuid, EntitlementError>;

    async fn grant(
        &self,
        tenant_id: Option<&str>,
        capability: Capability,
    ) -> Result<Option<EntitlementGrant>, EntitlementError>;

    async fn upsert_grant(&self, grant: &EntitlementGrant) -> Result<(), EntitlementError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn community_denies_every_premium_capability_offline() {
        let provider = Arc::new(CommunityEntitlements);
        let context = EntitlementContext {
            tenant_id: Some(Uuid::nil().to_string()),
            installation_id: Uuid::nil(),
        };
        for capability in Capability::ALL {
            assert_eq!(
                provider.decision(&context, capability).await.unwrap(),
                EntitlementDecision {
                    enabled: false,
                    valid_until: None,
                    reason_code: EntitlementReason::CommunityEdition,
                }
            );
        }
        let service = EntitlementService::new(provider, context.installation_id);
        assert_eq!(
            service
                .require(context.tenant_id, Capability::BrowserSso)
                .await,
            Err(EntitlementError::FeatureNotEntitled)
        );
    }
}
