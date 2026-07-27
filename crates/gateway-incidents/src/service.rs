use std::sync::Arc;

use chrono::Utc;
use gateway_types::{
    IncidentId, IncidentStatus, InspectionContext, SecurityFinding, SecurityIncident,
};

use crate::{IncidentError, IncidentRepository, IncidentTimelineEntry};

#[derive(Clone)]
pub struct IncidentService {
    repository: Arc<dyn IncidentRepository>,
}

impl IncidentService {
    pub fn new(repository: Arc<dyn IncidentRepository>) -> Self {
        Self { repository }
    }

    pub async fn create(
        &self,
        context: &InspectionContext,
        finding: &SecurityFinding,
        risk_score: u8,
    ) -> Result<SecurityIncident, IncidentError> {
        let incident = SecurityIncident {
            incident_id: IncidentId::new(),
            tenant_id: context.tenant_id.clone(),
            project_id: context.project_id.clone(),
            principal_id: Some(context.principal_id.clone()),
            request_id: context.request_id,
            category: finding.category,
            severity: finding.severity,
            status: IncidentStatus::Open,
            risk_score,
            sanitized_summary: format!(
                "{} finding from {}",
                category_name(finding.category),
                finding.inspector_id
            ),
            created_at: Utc::now(),
            resolved_at: None,
        };
        self.repository.insert_incident(incident.clone()).await?;
        Ok(incident)
    }

    pub async fn get(
        &self,
        tenant_id: &str,
        incident_id: IncidentId,
    ) -> Result<SecurityIncident, IncidentError> {
        self.repository
            .incident(tenant_id, incident_id)
            .await?
            .ok_or(IncidentError::NotFound)
    }
    pub async fn list(
        &self,
        tenant_id: &str,
        status: Option<IncidentStatus>,
        limit: u32,
    ) -> Result<Vec<SecurityIncident>, IncidentError> {
        self.repository
            .list_incidents(tenant_id, status, limit.min(200))
            .await
    }
    pub async fn update(
        &self,
        tenant_id: &str,
        incident_id: IncidentId,
        status: IncidentStatus,
        actor: String,
        note: Option<String>,
    ) -> Result<SecurityIncident, IncidentError> {
        self.repository
            .update_incident(
                tenant_id,
                IncidentTimelineEntry {
                    entry_id: uuid::Uuid::now_v7(),
                    incident_id,
                    status,
                    actor,
                    sanitized_note: note.map(|value| value.chars().take(512).collect()),
                    occurred_at: Utc::now(),
                },
            )
            .await
    }
    pub async fn timeline(
        &self,
        tenant_id: &str,
        incident_id: IncidentId,
    ) -> Result<Vec<IncidentTimelineEntry>, IncidentError> {
        self.repository
            .incident_timeline(tenant_id, incident_id)
            .await
    }
}

fn category_name(category: gateway_types::SecurityCategory) -> &'static str {
    use gateway_types::SecurityCategory::*;
    match category {
        PromptInjection => "prompt injection",
        JailbreakAttempt => "jailbreak",
        SecretExposure => "secret exposure",
        CredentialExposure => "credential exposure",
        SensitivePersonalData => "sensitive personal data",
        FinancialData => "financial data",
        SourceCodeSecret => "source-code secret",
        DataExfiltrationAttempt => "data exfiltration",
        PolicyViolation => "policy violation",
        SuspiciousToolArgument => "suspicious tool argument",
        SuspiciousToolResult => "suspicious tool result",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use gateway_types::*;
    use tokio::sync::Mutex;
    struct Repo(Mutex<Option<SecurityIncident>>);
    #[async_trait]
    impl IncidentRepository for Repo {
        async fn insert_incident(&self, value: SecurityIncident) -> Result<(), IncidentError> {
            *self.0.lock().await = Some(value);
            Ok(())
        }
        async fn incident(
            &self,
            tenant: &str,
            id: IncidentId,
        ) -> Result<Option<SecurityIncident>, IncidentError> {
            Ok(self
                .0
                .lock()
                .await
                .clone()
                .filter(|value| value.tenant_id == tenant && value.incident_id == id))
        }
        async fn list_incidents(
            &self,
            _: &str,
            _: Option<IncidentStatus>,
            _: u32,
        ) -> Result<Vec<SecurityIncident>, IncidentError> {
            Ok(vec![])
        }
        async fn update_incident(
            &self,
            _: &str,
            _: IncidentTimelineEntry,
        ) -> Result<SecurityIncident, IncidentError> {
            Err(IncidentError::NotFound)
        }
        async fn incident_timeline(
            &self,
            _: &str,
            _: IncidentId,
        ) -> Result<Vec<IncidentTimelineEntry>, IncidentError> {
            Ok(vec![])
        }
    }
    #[tokio::test]
    async fn creates_only_sanitized_summary() {
        let service = IncidentService::new(Arc::new(Repo(Mutex::new(None))));
        let context = InspectionContext {
            request_id: uuid::Uuid::now_v7(),
            tenant_id: "t".into(),
            project_id: None,
            principal_id: "p".into(),
            stage: "llm_input".into(),
            tool_risk: None,
        };
        let finding = SecurityFinding {
            finding_id: FindingId::new(),
            inspector_id: "secrets".into(),
            category: SecurityCategory::SecretExposure,
            severity: SecuritySeverity::High,
            confidence: 1.0,
            evidence: vec![SanitizedEvidence {
                redacted: "sk-a...[REDACTED]".into(),
                sha256: "hash".into(),
                start: Some(0),
                end: Some(30),
            }],
            recommended_action: SecurityAction::Block,
            metadata: Default::default(),
        };
        let incident = service.create(&context, &finding, 75).await.unwrap();
        assert!(!incident.sanitized_summary.contains("sk-a"));
        assert_eq!(incident.status, IncidentStatus::Open);
    }
}
