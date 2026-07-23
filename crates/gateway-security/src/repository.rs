use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_types::{
    FindingId, InspectionContext, SecurityAction, SecurityFinding, SecurityPolicyId,
};
use uuid::Uuid;

use crate::{SecurityError, SecurityPolicy};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SecurityPolicyRecord {
    pub policy_id: SecurityPolicyId,
    pub tenant_id: String,
    pub name: String,
    pub enabled: bool,
    pub policy: SecurityPolicy,
    pub scope_kind: String,
    pub scope_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SecurityFindingRecord {
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub principal_id: Option<String>,
    pub request_id: Uuid,
    pub finding: SecurityFinding,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SecurityEvent {
    pub event_id: Uuid,
    pub idempotency_key: String,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub principal_id: Option<String>,
    pub request_id: Uuid,
    pub event_type: String,
    pub action: SecurityAction,
    pub risk_score: u8,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SecurityCustomPattern {
    pub pattern_id: Uuid,
    pub tenant_id: String,
    pub name: String,
    pub category: gateway_types::SecurityCategory,
    pub pattern: String,
    pub enabled: bool,
    pub version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn validate_custom_pattern(name: &str, pattern: &str) -> Result<(), SecurityError> {
    if name.trim().is_empty()
        || name.len() > 255
        || pattern.is_empty()
        || pattern.len() > 4096
        || regex::Regex::new(pattern).is_err()
    {
        Err(SecurityError::InvalidPattern)
    } else {
        Ok(())
    }
}

#[async_trait]
pub trait SecurityRepository: Send + Sync {
    async fn resolved_security_policy(
        &self,
        context: &InspectionContext,
    ) -> Result<SecurityPolicy, SecurityError>;
    async fn insert_security_policy(
        &self,
        record: SecurityPolicyRecord,
        scope_kind: &str,
        scope_id: &str,
    ) -> Result<(), SecurityError>;
    async fn security_policies(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<SecurityPolicyRecord>, SecurityError>;
    async fn update_security_policy(
        &self,
        record: SecurityPolicyRecord,
        scope_kind: &str,
        scope_id: &str,
    ) -> Result<(), SecurityError>;
    async fn delete_security_policy(
        &self,
        tenant_id: &str,
        policy_id: SecurityPolicyId,
    ) -> Result<bool, SecurityError>;
    async fn insert_findings(
        &self,
        context: &InspectionContext,
        findings: &[SecurityFinding],
    ) -> Result<(), SecurityError>;
    async fn findings(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> Result<Vec<SecurityFindingRecord>, SecurityError>;
    async fn insert_security_event(&self, event: SecurityEvent) -> Result<(), SecurityError>;
    async fn security_events(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> Result<Vec<SecurityEvent>, SecurityError>;
    async fn custom_patterns(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<SecurityCustomPattern>, SecurityError>;
    async fn insert_custom_pattern(
        &self,
        pattern: SecurityCustomPattern,
    ) -> Result<(), SecurityError>;
    async fn update_custom_pattern(
        &self,
        pattern: SecurityCustomPattern,
        expected_version: u64,
    ) -> Result<bool, SecurityError>;
    async fn delete_custom_pattern(
        &self,
        tenant_id: &str,
        pattern_id: Uuid,
        expected_version: u64,
    ) -> Result<bool, SecurityError>;
}

pub fn finding_id(record: &SecurityFindingRecord) -> FindingId {
    record.finding.finding_id
}
