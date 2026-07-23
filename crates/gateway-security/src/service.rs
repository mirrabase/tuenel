use std::sync::Arc;

use chrono::Utc;
use gateway_incidents::IncidentService;
use gateway_types::{InspectionContent, InspectionContext, SecurityAction};
use tracing::Instrument;
use uuid::Uuid;

use crate::{
    SecurityDecision, SecurityError, SecurityEvent, SecurityPipeline, SecurityRepository, redact,
};

#[derive(Clone)]
pub struct SecurityEnforcer {
    pipeline: SecurityPipeline,
    repository: Arc<dyn SecurityRepository>,
    incidents: IncidentService,
    enabled: bool,
}

impl SecurityEnforcer {
    pub fn new(
        pipeline: SecurityPipeline,
        repository: Arc<dyn SecurityRepository>,
        incidents: IncidentService,
    ) -> Self {
        Self {
            pipeline,
            repository,
            incidents,
            enabled: true,
        }
    }
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub async fn inspect(
        &self,
        context: InspectionContext,
        content: InspectionContent,
    ) -> Result<(InspectionContent, SecurityDecision), SecurityError> {
        let (content, decision) = self.assess(context, content).await?;
        match decision.action {
            SecurityAction::Block => {
                tracing::info_span!("gateway.security.block", security_action = "block")
                    .in_scope(|| {});
                Err(block_error(&decision.findings))
            }
            SecurityAction::RequireApproval => Err(SecurityError::ApprovalRequired),
            _ => Ok((content, decision)),
        }
    }

    /// Inspect and persist a decision while allowing approval-aware callers to handle it.
    pub async fn assess(
        &self,
        context: InspectionContext,
        content: InspectionContent,
    ) -> Result<(InspectionContent, SecurityDecision), SecurityError> {
        if !self.enabled {
            let policy = crate::SecurityPolicy {
                enabled: false,
                ..Default::default()
            };
            return Ok((content, crate::decide(&policy, Vec::new(), false)));
        }
        let policy = self.repository.resolved_security_policy(&context).await?;
        if !stage_enabled(&policy, &context.stage) {
            return Ok((content, crate::decide(&policy, Vec::new(), false)));
        }
        let decision = self.pipeline.inspect(&policy, &context, &content).instrument(tracing::info_span!("gateway.security.inspect",request_id=%context.request_id,tenant_id=%context.tenant_id,project_id=?context.project_id,principal_id=%context.principal_id)).await;
        tracing::info_span!("gateway.security.decide",request_id=%context.request_id,security_action=?decision.action,risk_score=decision.risk_score).in_scope(||{});
        let metrics = gateway_observability::metrics();
        metrics.security_inspections.inc();
        for finding in &decision.findings {
            metrics
                .security_findings
                .with_label_values(&[
                    &format!("{:?}", finding.category).to_ascii_lowercase(),
                    &format!("{:?}", finding.severity).to_ascii_lowercase(),
                ])
                .inc();
        }
        match decision.action {
            SecurityAction::Block => metrics.security_blocks.inc(),
            SecurityAction::Redact => metrics.security_redactions.inc(),
            SecurityAction::Warn => metrics.security_warnings.inc(),
            _ => {}
        }
        self.repository
            .insert_findings(&context, &decision.findings)
            .await?;
        self.repository.insert_security_event(SecurityEvent { event_id: Uuid::now_v7(), idempotency_key: format!("{}:{}:{}", context.request_id, context.stage, decision.action.priority()), tenant_id: context.tenant_id.clone(), project_id: context.project_id.clone(), principal_id: Some(context.principal_id.clone()), request_id: context.request_id, event_type: event_name(decision.action).into(), action: decision.action, risk_score: decision.risk_score, metadata: serde_json::json!({"finding_count":decision.findings.len(),"inspection_failed":decision.inspection_failed}), created_at: Utc::now() }).await?;
        if policy.create_incidents
            && matches!(
                decision.action,
                SecurityAction::Block | SecurityAction::RequireApproval
            )
        {
            if let Some(finding) = decision
                .findings
                .iter()
                .max_by_key(|finding| finding.severity)
            {
                self.incidents.create(&context, finding, decision.risk_score).instrument(tracing::info_span!("gateway.security.create_incident",request_id=%context.request_id,risk_score=decision.risk_score)).await.map_err(|_| SecurityError::InspectionFailed)?;
                metrics.security_incidents.inc();
            }
        }
        match decision.action {
            SecurityAction::Redact => {
                let _span =
                    tracing::info_span!("gateway.security.redact",request_id=%context.request_id)
                        .entered();
                let evidence = decision
                    .findings
                    .iter()
                    .flat_map(|finding| finding.evidence.clone())
                    .collect::<Vec<_>>();
                Ok((redact(&content, &evidence), decision))
            }
            SecurityAction::Allow
            | SecurityAction::Warn
            | SecurityAction::Block
            | SecurityAction::RequireApproval => Ok((content, decision)),
        }
    }

    pub async fn enabled_for(&self, context: &InspectionContext) -> Result<bool, SecurityError> {
        if !self.enabled {
            return Ok(false);
        };
        Ok(stage_enabled(
            &self.repository.resolved_security_policy(context).await?,
            &context.stage,
        ))
    }
}

fn event_name(action: SecurityAction) -> &'static str {
    match action {
        SecurityAction::Allow => "security.inspection.completed",
        SecurityAction::Warn => "security.request.warned",
        SecurityAction::Redact => "security.request.redacted",
        SecurityAction::RequireApproval => "security.request.approval_required",
        SecurityAction::Block => "security.request.blocked",
    }
}
fn block_error(findings: &[gateway_types::SecurityFinding]) -> SecurityError {
    if findings.iter().any(|finding| {
        matches!(
            finding.category,
            gateway_types::SecurityCategory::PromptInjection
                | gateway_types::SecurityCategory::JailbreakAttempt
        )
    }) {
        SecurityError::PromptInjectionDetected
    } else if findings.iter().any(|finding| {
        matches!(
            finding.category,
            gateway_types::SecurityCategory::SecretExposure
                | gateway_types::SecurityCategory::CredentialExposure
                | gateway_types::SecurityCategory::SourceCodeSecret
        )
    }) {
        SecurityError::SecretExposureDetected
    } else if findings.iter().any(|finding| {
        matches!(
            finding.category,
            gateway_types::SecurityCategory::SensitivePersonalData
                | gateway_types::SecurityCategory::FinancialData
        )
    }) {
        SecurityError::SensitiveDataDetected
    } else {
        SecurityError::Blocked
    }
}
fn stage_enabled(policy: &crate::SecurityPolicy, stage: &str) -> bool {
    if stage.starts_with("llm_output") {
        policy.inspect_llm_output
    } else if stage.starts_with("mcp_argument") {
        policy.inspect_mcp_arguments
    } else if stage.starts_with("mcp_result") {
        policy.inspect_mcp_results
    } else {
        policy.inspect_llm_input
    }
}
