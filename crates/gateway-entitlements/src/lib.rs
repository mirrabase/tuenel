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

/// Provider-neutral managed product tier. Community and enterprise runtimes may
/// ignore this profile; managed adapters project their billing provider into it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductTier {
    Free,
    Core,
    Pro,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingInterval {
    Monthly,
    Annual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PlanLimits {
    /// `None` is serialized as `null` and means that the plan has no cap.
    pub projects: Option<u64>,
    pub members: Option<u64>,
    pub active_api_keys: Option<u64>,
    pub providers: Option<u64>,
    pub routed_tokens_per_month: Option<u64>,
    pub requests_per_minute: Option<u64>,
    pub history_days: Option<u64>,
    pub fallback_targets: Option<u64>,
    pub mcp_servers: Option<u64>,
    pub budget_rules: Option<u64>,
    pub security_patterns: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PlanFeatures {
    pub audit_export: bool,
    pub browser_sso: bool,
    pub custom_domain: bool,
    pub output_inspection: bool,
    pub mcp_argument_inspection: bool,
    pub mcp_result_inspection: bool,
    pub custom_security_policy: bool,
    pub human_approval: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TenantPlanProfile {
    pub tier: ProductTier,
    pub interval: Option<BillingInterval>,
    pub limits: PlanLimits,
    pub features: PlanFeatures,
}

impl TenantPlanProfile {
    pub const fn for_tier(tier: ProductTier, interval: Option<BillingInterval>) -> Self {
        let (limits, features) = match tier {
            ProductTier::Free => (
                PlanLimits {
                    projects: Some(1),
                    members: Some(1),
                    active_api_keys: Some(2),
                    providers: Some(1),
                    routed_tokens_per_month: Some(100_000),
                    requests_per_minute: Some(10),
                    history_days: Some(7),
                    fallback_targets: Some(0),
                    mcp_servers: Some(0),
                    budget_rules: Some(0),
                    security_patterns: Some(0),
                },
                PlanFeatures {
                    audit_export: false,
                    browser_sso: false,
                    custom_domain: false,
                    output_inspection: false,
                    mcp_argument_inspection: false,
                    mcp_result_inspection: false,
                    custom_security_policy: false,
                    human_approval: false,
                },
            ),
            ProductTier::Core => (
                PlanLimits {
                    projects: Some(5),
                    members: Some(5),
                    active_api_keys: None,
                    providers: Some(5),
                    routed_tokens_per_month: None,
                    requests_per_minute: Some(120),
                    history_days: Some(30),
                    fallback_targets: Some(3),
                    mcp_servers: Some(5),
                    budget_rules: Some(25),
                    security_patterns: Some(25),
                },
                PlanFeatures {
                    audit_export: true,
                    browser_sso: false,
                    custom_domain: false,
                    output_inspection: false,
                    mcp_argument_inspection: true,
                    mcp_result_inspection: false,
                    custom_security_policy: true,
                    human_approval: false,
                },
            ),
            ProductTier::Pro => (
                PlanLimits {
                    projects: Some(25),
                    members: Some(25),
                    active_api_keys: None,
                    providers: Some(25),
                    routed_tokens_per_month: None,
                    requests_per_minute: Some(600),
                    history_days: Some(365),
                    fallback_targets: Some(10),
                    mcp_servers: Some(25),
                    budget_rules: Some(100),
                    security_patterns: Some(250),
                },
                PlanFeatures {
                    audit_export: true,
                    browser_sso: true,
                    custom_domain: false,
                    output_inspection: true,
                    mcp_argument_inspection: true,
                    mcp_result_inspection: true,
                    custom_security_policy: true,
                    human_approval: true,
                },
            ),
        };
        Self {
            tier,
            interval: if matches!(tier, ProductTier::Free) {
                None
            } else {
                interval
            },
            limits,
            features,
        }
    }
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

    #[test]
    fn managed_plan_profiles_match_the_product_contract() {
        let free = TenantPlanProfile::for_tier(ProductTier::Free, Some(BillingInterval::Annual));
        assert_eq!(free.interval, None);
        assert_eq!(free.limits.routed_tokens_per_month, Some(100_000));
        let core = TenantPlanProfile::for_tier(ProductTier::Core, Some(BillingInterval::Monthly));
        assert_eq!(core.limits.projects, Some(5));
        assert_eq!(core.limits.active_api_keys, None);
        assert_eq!(core.limits.routed_tokens_per_month, None);
        assert_eq!(core.limits.requests_per_minute, Some(120));
        assert!(core.features.audit_export);
        assert!(!core.features.browser_sso);
        assert!(!core.features.custom_domain);
        let pro = TenantPlanProfile::for_tier(ProductTier::Pro, Some(BillingInterval::Annual));
        assert_eq!(pro.limits.requests_per_minute, Some(600));
        assert_eq!(pro.limits.history_days, Some(365));
        assert!(pro.features.human_approval);
    }
}
