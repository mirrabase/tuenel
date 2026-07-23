use async_trait::async_trait;
use gateway_types::{
    FindingId, InspectionContent, InspectionContext, SanitizedEvidence, SecurityAction,
    SecurityFinding, SecuritySeverity,
};
use std::sync::Arc;

use crate::SecurityError;

#[async_trait]
pub trait SecurityInspector: Send + Sync {
    fn id(&self) -> &str;
    async fn inspect(
        &self,
        context: &InspectionContext,
        content: &InspectionContent,
    ) -> Result<Vec<SecurityFinding>, SecurityError>;
}

pub struct CustomPatternInspector {
    repository: Arc<dyn crate::SecurityRepository>,
}
impl CustomPatternInspector {
    pub fn new(repository: Arc<dyn crate::SecurityRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl SecurityInspector for CustomPatternInspector {
    fn id(&self) -> &str {
        "custom_patterns"
    }
    async fn inspect(
        &self,
        context: &InspectionContext,
        content: &InspectionContent,
    ) -> Result<Vec<SecurityFinding>, SecurityError> {
        let text = match content {
            InspectionContent::PromptText(value) | InspectionContent::ModelOutput(value) => {
                value.clone()
            }
            InspectionContent::StructuredInput(value)
            | InspectionContent::ToolArguments(value)
            | InspectionContent::ToolResult(value) => {
                serde_json::to_string(value).map_err(|_| SecurityError::InspectionFailed)?
            }
        };
        let mut findings = Vec::new();
        // ponytail: compile per inspection; add a tenant-versioned cache only when custom-pattern volume warrants it.
        for item in self.repository.custom_patterns(&context.tenant_id).await? {
            let pattern =
                regex::Regex::new(&item.pattern).map_err(|_| SecurityError::InspectionFailed)?;
            let evidence = security_regex::find(std::slice::from_ref(&pattern), &text, 32)
                .into_iter()
                .map(|value| SanitizedEvidence {
                    redacted: value.redacted,
                    sha256: value.sha256,
                    start: Some(value.start),
                    end: Some(value.end),
                })
                .collect::<Vec<_>>();
            if !evidence.is_empty() {
                findings.push(SecurityFinding {
                    finding_id: FindingId::new(),
                    inspector_id: self.id().into(),
                    category: item.category,
                    severity: SecuritySeverity::High,
                    confidence: 1.0,
                    evidence,
                    recommended_action: SecurityAction::Block,
                    metadata: Default::default(),
                });
            }
        }
        Ok(findings)
    }
}
