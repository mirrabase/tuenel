use async_trait::async_trait;
use gateway_security::{
    SecurityError, SecurityEvent, SecurityFindingRecord, SecurityPolicy, SecurityPolicyRecord,
    SecurityRepository,
};
use gateway_types::{
    FindingId, InspectionContext, SanitizedEvidence, SecurityAction, SecurityCategory,
    SecurityFinding, SecurityPolicyId, SecuritySeverity,
};
use sqlx::Row;
use std::collections::HashMap;

use super::PostgresStore;

#[async_trait]
impl SecurityRepository for PostgresStore {
    async fn resolved_security_policy(
        &self,
        context: &InspectionContext,
    ) -> Result<SecurityPolicy, SecurityError> {
        let virtual_key_id = context
            .principal_id
            .strip_prefix("virtual-key:")
            .unwrap_or("");
        let rows = sqlx::query("SELECT p.rules FROM security_policies p JOIN security_policy_bindings b ON b.policy_id=p.policy_id WHERE p.tenant_id=$1 AND p.enabled=true AND ((b.scope_kind='global') OR (b.scope_kind='tenant' AND b.scope_id=$1) OR (b.scope_kind='project' AND b.scope_id=$2) OR (b.scope_kind='principal' AND b.scope_id=$3) OR (b.scope_kind='virtual_key' AND b.scope_id=$4)) ORDER BY CASE b.scope_kind WHEN 'global' THEN 1 WHEN 'tenant' THEN 2 WHEN 'project' THEN 3 WHEN 'principal' THEN 4 ELSE 5 END").bind(&context.tenant_id).bind(context.project_id.as_deref().unwrap_or("")).bind(&context.principal_id).bind(virtual_key_id).fetch_all(&self.pool).await.map_err(|_| SecurityError::InspectionFailed)?;
        let policies = rows
            .into_iter()
            .map(|row| {
                serde_json::from_value(
                    row.try_get("rules")
                        .map_err(|_| SecurityError::InspectionFailed)?,
                )
                .map_err(|_| SecurityError::InspectionFailed)
            })
            .collect::<Result<Vec<SecurityPolicy>, SecurityError>>()?;
        let policy = gateway_security::resolve_security_hierarchy(policies);
        let tier = sqlx::query_scalar::<_, String>(
            "SELECT CASE WHEN valid_until IS NOT NULL AND valid_until<=now() THEN 'free' ELSE tier END \
             FROM tenant_plan_profiles WHERE tenant_id=$1",
        )
        .bind(&context.tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| SecurityError::InspectionFailed)?;
        Ok(apply_managed_plan_security(policy, tier.as_deref()))
    }
    async fn insert_security_policy(
        &self,
        record: SecurityPolicyRecord,
        scope_kind: &str,
        scope_id: &str,
    ) -> Result<(), SecurityError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| SecurityError::InspectionFailed)?;
        sqlx::query("INSERT INTO security_policies (policy_id,tenant_id,name,enabled,rules,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7)").bind(record.policy_id.0).bind(&record.tenant_id).bind(record.name).bind(record.enabled).bind(serde_json::to_value(record.policy).map_err(|_|SecurityError::InspectionFailed)?).bind(record.created_at).bind(record.updated_at).execute(&mut *transaction).await.map_err(|_|SecurityError::InspectionFailed)?;
        sqlx::query("INSERT INTO security_policy_bindings (binding_id,tenant_id,policy_id,scope_kind,scope_id) VALUES ($1,$2,$3,$4,$5)").bind(uuid::Uuid::now_v7()).bind(record.tenant_id).bind(record.policy_id.0).bind(scope_kind).bind(scope_id).execute(&mut *transaction).await.map_err(|_|SecurityError::InspectionFailed)?;
        transaction
            .commit()
            .await
            .map_err(|_| SecurityError::InspectionFailed)
    }
    async fn security_policies(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<SecurityPolicyRecord>, SecurityError> {
        sqlx::query("SELECT p.policy_id,p.tenant_id,p.name,p.enabled,p.rules,p.created_at,p.updated_at,b.scope_kind,b.scope_id FROM security_policies p JOIN security_policy_bindings b ON b.policy_id=p.policy_id WHERE p.tenant_id=$1 ORDER BY p.name").bind(tenant_id).fetch_all(&self.pool).await.map_err(|_|SecurityError::InspectionFailed)?.into_iter().map(policy_from_row).collect()
    }
    async fn update_security_policy(
        &self,
        record: SecurityPolicyRecord,
        scope_kind: &str,
        scope_id: &str,
    ) -> Result<(), SecurityError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| SecurityError::InspectionFailed)?;
        sqlx::query("UPDATE security_policies SET name=$3,enabled=$4,rules=$5,updated_at=now() WHERE tenant_id=$1 AND policy_id=$2").bind(&record.tenant_id).bind(record.policy_id.0).bind(record.name).bind(record.enabled).bind(serde_json::to_value(record.policy).map_err(|_|SecurityError::InspectionFailed)?).execute(&mut *transaction).await.map_err(|_|SecurityError::InspectionFailed)?;
        sqlx::query("UPDATE security_policy_bindings SET scope_kind=$3,scope_id=$4 WHERE tenant_id=$1 AND policy_id=$2").bind(record.tenant_id).bind(record.policy_id.0).bind(scope_kind).bind(scope_id).execute(&mut *transaction).await.map_err(|_|SecurityError::InspectionFailed)?;
        transaction
            .commit()
            .await
            .map_err(|_| SecurityError::InspectionFailed)
    }
    async fn delete_security_policy(
        &self,
        tenant_id: &str,
        policy_id: SecurityPolicyId,
    ) -> Result<bool, SecurityError> {
        sqlx::query("DELETE FROM security_policies WHERE tenant_id=$1 AND policy_id=$2")
            .bind(tenant_id)
            .bind(policy_id.0)
            .execute(&self.pool)
            .await
            .map_err(|_| SecurityError::InspectionFailed)
            .map(|result| result.rows_affected() > 0)
    }
    async fn insert_findings(
        &self,
        context: &InspectionContext,
        findings: &[SecurityFinding],
    ) -> Result<(), SecurityError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| SecurityError::InspectionFailed)?;
        for finding in findings {
            sqlx::query("INSERT INTO security_findings (finding_id,tenant_id,project_id,principal_id,request_id,inspector_id,category,severity,confidence,evidence,recommended_action,metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT (finding_id) DO NOTHING").bind(finding.finding_id.0).bind(&context.tenant_id).bind(&context.project_id).bind(&context.principal_id).bind(context.request_id).bind(&finding.inspector_id).bind(category_name(finding.category)).bind(severity_name(finding.severity)).bind(finding.confidence).bind(serde_json::to_value(&finding.evidence).map_err(|_|SecurityError::InspectionFailed)?).bind(action_name(finding.recommended_action)).bind(serde_json::to_value(&finding.metadata).map_err(|_|SecurityError::InspectionFailed)?).execute(&mut *transaction).await.map_err(|_|SecurityError::InspectionFailed)?;
            sqlx::query("INSERT INTO security_events(event_id,idempotency_key,tenant_id,project_id,principal_id,request_id,event_type,action,risk_score,metadata) VALUES($1,$2,$3,$4,$5,$6,'security.finding.created',$7,$8,$9) ON CONFLICT(idempotency_key) DO NOTHING").bind(finding.finding_id.0).bind(format!("security.finding.created:{}",finding.finding_id)).bind(&context.tenant_id).bind(&context.project_id).bind(&context.principal_id).bind(context.request_id).bind(action_name(finding.recommended_action)).bind(match finding.severity{SecuritySeverity::Low=>15_i16,SecuritySeverity::Medium=>45,SecuritySeverity::High=>75,SecuritySeverity::Critical=>95}).bind(serde_json::json!({"category":category_name(finding.category)})).execute(&mut *transaction).await.map_err(|_|SecurityError::InspectionFailed)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| SecurityError::InspectionFailed)
    }
    async fn findings(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> Result<Vec<SecurityFindingRecord>, SecurityError> {
        sqlx::query(
            "SELECT * FROM security_findings WHERE tenant_id=$1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(tenant_id)
        .bind(i64::from(limit.min(200)))
        .fetch_all(&self.pool)
        .await
        .map_err(|_| SecurityError::InspectionFailed)?
        .into_iter()
        .map(finding_from_row)
        .collect()
    }
    async fn insert_security_event(&self, event: SecurityEvent) -> Result<(), SecurityError> {
        sqlx::query("INSERT INTO security_events (event_id,idempotency_key,tenant_id,project_id,principal_id,request_id,event_type,action,risk_score,metadata,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (idempotency_key) DO NOTHING").bind(event.event_id).bind(event.idempotency_key).bind(event.tenant_id).bind(event.project_id).bind(event.principal_id).bind(event.request_id).bind(event.event_type).bind(action_name(event.action)).bind(i16::from(event.risk_score)).bind(event.metadata).bind(event.created_at).execute(&self.pool).await.map_err(|_|SecurityError::InspectionFailed).map(|_|())
    }
    async fn security_events(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> Result<Vec<SecurityEvent>, SecurityError> {
        sqlx::query(
            "SELECT * FROM security_events WHERE tenant_id=$1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(tenant_id)
        .bind(i64::from(limit.min(200)))
        .fetch_all(&self.pool)
        .await
        .map_err(|_| SecurityError::InspectionFailed)?
        .into_iter()
        .map(event_from_row)
        .collect()
    }
    async fn custom_patterns(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<gateway_security::SecurityCustomPattern>, SecurityError> {
        sqlx::query("SELECT p.pattern_id,p.tenant_id,p.name,p.category,p.pattern,p.enabled AND NOT EXISTS (SELECT 1 FROM plan_resource_suspensions s WHERE s.tenant_id=p.tenant_id AND s.resource_kind='security_patterns' AND s.resource_id=p.pattern_id::text AND s.restored_at IS NULL) effective_enabled,p.version,p.created_at,p.updated_at FROM security_custom_patterns p WHERE p.tenant_id=$1 ORDER BY p.name").bind(tenant_id).fetch_all(&self.pool).await.map_err(|_|SecurityError::InspectionFailed)?.into_iter().map(pattern_from_row).collect()
    }
    async fn insert_custom_pattern(
        &self,
        pattern: gateway_security::SecurityCustomPattern,
    ) -> Result<(), SecurityError> {
        sqlx::query("INSERT INTO security_custom_patterns(pattern_id,tenant_id,name,category,pattern,enabled,version,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(pattern.pattern_id).bind(pattern.tenant_id).bind(pattern.name)
            .bind(category_name(pattern.category)).bind(pattern.pattern).bind(pattern.enabled)
            .bind(i64::try_from(pattern.version).map_err(|_|SecurityError::InspectionFailed)?)
            .bind(pattern.created_at).bind(pattern.updated_at).execute(&self.pool).await
            .map_err(|_|SecurityError::InspectionFailed).map(|_|())
    }
    async fn update_custom_pattern(
        &self,
        pattern: gateway_security::SecurityCustomPattern,
        expected_version: u64,
    ) -> Result<bool, SecurityError> {
        sqlx::query("UPDATE security_custom_patterns SET name=$3,category=$4,pattern=$5,enabled=$6,version=version+1,updated_at=now() WHERE tenant_id=$1 AND pattern_id=$2 AND version=$7")
            .bind(pattern.tenant_id).bind(pattern.pattern_id).bind(pattern.name)
            .bind(category_name(pattern.category)).bind(pattern.pattern).bind(pattern.enabled)
            .bind(i64::try_from(expected_version).map_err(|_|SecurityError::InspectionFailed)?)
            .execute(&self.pool).await.map_err(|_|SecurityError::InspectionFailed)
            .map(|result|result.rows_affected()==1)
    }
    async fn delete_custom_pattern(
        &self,
        tenant_id: &str,
        pattern_id: uuid::Uuid,
        expected_version: u64,
    ) -> Result<bool, SecurityError> {
        sqlx::query("UPDATE security_custom_patterns SET enabled=false,version=version+1,updated_at=now() WHERE tenant_id=$1 AND pattern_id=$2 AND version=$3")
            .bind(tenant_id).bind(pattern_id)
            .bind(i64::try_from(expected_version).map_err(|_|SecurityError::InspectionFailed)?)
            .execute(&self.pool).await.map_err(|_|SecurityError::InspectionFailed)
            .map(|result|result.rows_affected()==1)
    }
}

fn apply_managed_plan_security(mut policy: SecurityPolicy, tier: Option<&str>) -> SecurityPolicy {
    match tier {
        // Free security is deliberately useful but not configurable: inspect
        // model input and surface every supported finding as a warning.
        Some("free") => SecurityPolicy {
            enabled: true,
            fail_open: true,
            inspect_llm_input: true,
            inspect_llm_output: false,
            inspect_mcp_arguments: false,
            inspect_mcp_results: false,
            create_incidents: false,
            maximum_content_bytes: policy.maximum_content_bytes,
            actions: fixed_warn_actions(),
        },
        Some("core") => {
            policy.enabled = true;
            policy.inspect_llm_input = true;
            policy.inspect_llm_output = false;
            policy.inspect_mcp_arguments = true;
            policy.inspect_mcp_results = false;
            policy.create_incidents = false;
            for actions in policy.actions.values_mut() {
                for action in actions.values_mut() {
                    if matches!(*action, SecurityAction::RequireApproval) {
                        *action = SecurityAction::Block;
                    }
                }
            }
            policy
        }
        // Pro and installations without a managed profile retain the resolved
        // policy. The latter keeps self-hosted behavior edition-neutral.
        Some("pro") | None => policy,
        Some(_) => SecurityPolicy {
            enabled: true,
            fail_open: true,
            inspect_llm_input: true,
            inspect_llm_output: false,
            inspect_mcp_arguments: false,
            inspect_mcp_results: false,
            create_incidents: false,
            maximum_content_bytes: policy.maximum_content_bytes,
            actions: fixed_warn_actions(),
        },
    }
}

fn fixed_warn_actions() -> HashMap<SecurityCategory, HashMap<SecuritySeverity, SecurityAction>> {
    const CATEGORIES: [SecurityCategory; 11] = [
        SecurityCategory::PromptInjection,
        SecurityCategory::JailbreakAttempt,
        SecurityCategory::SecretExposure,
        SecurityCategory::CredentialExposure,
        SecurityCategory::SensitivePersonalData,
        SecurityCategory::FinancialData,
        SecurityCategory::SourceCodeSecret,
        SecurityCategory::DataExfiltrationAttempt,
        SecurityCategory::PolicyViolation,
        SecurityCategory::SuspiciousToolArgument,
        SecurityCategory::SuspiciousToolResult,
    ];
    const SEVERITIES: [SecuritySeverity; 4] = [
        SecuritySeverity::Low,
        SecuritySeverity::Medium,
        SecuritySeverity::High,
        SecuritySeverity::Critical,
    ];
    CATEGORIES
        .into_iter()
        .map(|category| {
            (
                category,
                SEVERITIES
                    .into_iter()
                    .map(|severity| (severity, SecurityAction::Warn))
                    .collect(),
            )
        })
        .collect()
}

fn pattern_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<gateway_security::SecurityCustomPattern, SecurityError> {
    Ok(gateway_security::SecurityCustomPattern {
        pattern_id: row
            .try_get("pattern_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        name: row
            .try_get("name")
            .map_err(|_| SecurityError::InspectionFailed)?,
        category: parse_category(
            &row.try_get::<String, _>("category")
                .map_err(|_| SecurityError::InspectionFailed)?,
        ),
        pattern: row
            .try_get("pattern")
            .map_err(|_| SecurityError::InspectionFailed)?,
        enabled: row
            .try_get("effective_enabled")
            .map_err(|_| SecurityError::InspectionFailed)?,
        version: row
            .try_get::<i64, _>("version")
            .map_err(|_| SecurityError::InspectionFailed)?
            .try_into()
            .map_err(|_| SecurityError::InspectionFailed)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| SecurityError::InspectionFailed)?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|_| SecurityError::InspectionFailed)?,
    })
}

fn policy_from_row(row: sqlx::postgres::PgRow) -> Result<SecurityPolicyRecord, SecurityError> {
    Ok(SecurityPolicyRecord {
        policy_id: SecurityPolicyId(
            row.try_get("policy_id")
                .map_err(|_| SecurityError::InspectionFailed)?,
        ),
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        name: row
            .try_get("name")
            .map_err(|_| SecurityError::InspectionFailed)?,
        enabled: row
            .try_get("enabled")
            .map_err(|_| SecurityError::InspectionFailed)?,
        policy: serde_json::from_value(
            row.try_get("rules")
                .map_err(|_| SecurityError::InspectionFailed)?,
        )
        .map_err(|_| SecurityError::InspectionFailed)?,
        scope_kind: row
            .try_get("scope_kind")
            .map_err(|_| SecurityError::InspectionFailed)?,
        scope_id: row
            .try_get("scope_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| SecurityError::InspectionFailed)?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|_| SecurityError::InspectionFailed)?,
    })
}
fn finding_from_row(row: sqlx::postgres::PgRow) -> Result<SecurityFindingRecord, SecurityError> {
    Ok(SecurityFindingRecord {
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        principal_id: row
            .try_get("principal_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        request_id: row
            .try_get("request_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| SecurityError::InspectionFailed)?,
        finding: SecurityFinding {
            finding_id: FindingId(
                row.try_get("finding_id")
                    .map_err(|_| SecurityError::InspectionFailed)?,
            ),
            inspector_id: row
                .try_get("inspector_id")
                .map_err(|_| SecurityError::InspectionFailed)?,
            category: parse_category(
                &row.try_get::<String, _>("category")
                    .map_err(|_| SecurityError::InspectionFailed)?,
            ),
            severity: parse_severity(
                &row.try_get::<String, _>("severity")
                    .map_err(|_| SecurityError::InspectionFailed)?,
            ),
            confidence: row
                .try_get("confidence")
                .map_err(|_| SecurityError::InspectionFailed)?,
            evidence: serde_json::from_value::<Vec<SanitizedEvidence>>(
                row.try_get("evidence")
                    .map_err(|_| SecurityError::InspectionFailed)?,
            )
            .map_err(|_| SecurityError::InspectionFailed)?,
            recommended_action: parse_action(
                &row.try_get::<String, _>("recommended_action")
                    .map_err(|_| SecurityError::InspectionFailed)?,
            ),
            metadata: serde_json::from_value(
                row.try_get("metadata")
                    .map_err(|_| SecurityError::InspectionFailed)?,
            )
            .map_err(|_| SecurityError::InspectionFailed)?,
        },
    })
}
fn event_from_row(row: sqlx::postgres::PgRow) -> Result<SecurityEvent, SecurityError> {
    Ok(SecurityEvent {
        event_id: row
            .try_get("event_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        idempotency_key: row
            .try_get("idempotency_key")
            .map_err(|_| SecurityError::InspectionFailed)?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        principal_id: row
            .try_get("principal_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        request_id: row
            .try_get("request_id")
            .map_err(|_| SecurityError::InspectionFailed)?,
        event_type: row
            .try_get("event_type")
            .map_err(|_| SecurityError::InspectionFailed)?,
        action: parse_action(
            &row.try_get::<String, _>("action")
                .map_err(|_| SecurityError::InspectionFailed)?,
        ),
        risk_score: row
            .try_get::<i16, _>("risk_score")
            .map_err(|_| SecurityError::InspectionFailed)?
            .try_into()
            .map_err(|_| SecurityError::InspectionFailed)?,
        metadata: row
            .try_get("metadata")
            .map_err(|_| SecurityError::InspectionFailed)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| SecurityError::InspectionFailed)?,
    })
}
fn action_name(value: SecurityAction) -> &'static str {
    match value {
        SecurityAction::Allow => "allow",
        SecurityAction::Warn => "warn",
        SecurityAction::Redact => "redact",
        SecurityAction::RequireApproval => "require_approval",
        SecurityAction::Block => "block",
    }
}
fn parse_action(value: &str) -> SecurityAction {
    match value {
        "warn" => SecurityAction::Warn,
        "redact" => SecurityAction::Redact,
        "require_approval" => SecurityAction::RequireApproval,
        "block" => SecurityAction::Block,
        _ => SecurityAction::Allow,
    }
}
fn severity_name(value: SecuritySeverity) -> &'static str {
    match value {
        SecuritySeverity::Low => "low",
        SecuritySeverity::Medium => "medium",
        SecuritySeverity::High => "high",
        SecuritySeverity::Critical => "critical",
    }
}
fn parse_severity(value: &str) -> SecuritySeverity {
    match value {
        "medium" => SecuritySeverity::Medium,
        "high" => SecuritySeverity::High,
        "critical" => SecuritySeverity::Critical,
        _ => SecuritySeverity::Low,
    }
}
fn category_name(value: SecurityCategory) -> &'static str {
    match value {
        SecurityCategory::PromptInjection => "prompt_injection",
        SecurityCategory::JailbreakAttempt => "jailbreak_attempt",
        SecurityCategory::SecretExposure => "secret_exposure",
        SecurityCategory::CredentialExposure => "credential_exposure",
        SecurityCategory::SensitivePersonalData => "sensitive_personal_data",
        SecurityCategory::FinancialData => "financial_data",
        SecurityCategory::SourceCodeSecret => "source_code_secret",
        SecurityCategory::DataExfiltrationAttempt => "data_exfiltration_attempt",
        SecurityCategory::PolicyViolation => "policy_violation",
        SecurityCategory::SuspiciousToolArgument => "suspicious_tool_argument",
        SecurityCategory::SuspiciousToolResult => "suspicious_tool_result",
    }
}
fn parse_category(value: &str) -> SecurityCategory {
    match value {
        "prompt_injection" => SecurityCategory::PromptInjection,
        "jailbreak_attempt" => SecurityCategory::JailbreakAttempt,
        "credential_exposure" => SecurityCategory::CredentialExposure,
        "sensitive_personal_data" => SecurityCategory::SensitivePersonalData,
        "financial_data" => SecurityCategory::FinancialData,
        "source_code_secret" => SecurityCategory::SourceCodeSecret,
        "data_exfiltration_attempt" => SecurityCategory::DataExfiltrationAttempt,
        "policy_violation" => SecurityCategory::PolicyViolation,
        "suspicious_tool_argument" => SecurityCategory::SuspiciousToolArgument,
        "suspicious_tool_result" => SecurityCategory::SuspiciousToolResult,
        _ => SecurityCategory::SecretExposure,
    }
}

#[cfg(test)]
mod managed_plan_tests {
    use super::*;

    #[test]
    fn free_is_input_only_and_fixed_warn() {
        let policy = SecurityPolicy {
            inspect_llm_output: true,
            create_incidents: true,
            ..SecurityPolicy::default()
        };
        let result = apply_managed_plan_security(policy, Some("free"));
        assert!(result.inspect_llm_input);
        assert!(result.fail_open);
        assert!(!result.inspect_llm_output);
        assert!(!result.inspect_mcp_arguments);
        assert!(!result.create_incidents);
        assert_eq!(
            result.action(
                SecurityCategory::PromptInjection,
                SecuritySeverity::Critical,
                SecurityAction::Block
            ),
            SecurityAction::Warn
        );
    }

    #[test]
    fn core_disables_pro_stages_and_approval() {
        let mut policy = SecurityPolicy {
            inspect_llm_output: true,
            inspect_mcp_results: true,
            create_incidents: true,
            ..SecurityPolicy::default()
        };
        policy
            .actions
            .entry(SecurityCategory::PolicyViolation)
            .or_default()
            .insert(SecuritySeverity::High, SecurityAction::RequireApproval);
        let result = apply_managed_plan_security(policy, Some("core"));
        assert!(result.inspect_llm_input);
        assert!(result.inspect_mcp_arguments);
        assert!(!result.inspect_llm_output);
        assert!(!result.inspect_mcp_results);
        assert!(!result.create_incidents);
        assert_eq!(
            result.action(
                SecurityCategory::PolicyViolation,
                SecuritySeverity::High,
                SecurityAction::Warn
            ),
            SecurityAction::Block
        );
    }
}
