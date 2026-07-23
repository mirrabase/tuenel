//! Deterministic sensitive-data inspection with validated card detection.

use async_trait::async_trait;
use gateway_security::{SecurityError, SecurityInspector};
use gateway_types::{
    FindingId, InspectionContent, InspectionContext, SanitizedEvidence, SecurityAction,
    SecurityCategory, SecurityFinding, SecuritySeverity,
};
use regex::Regex;

pub struct SensitiveDataInspector {
    patterns: Vec<(SecurityCategory, Regex)>,
    ip_pattern: Regex,
}

impl SensitiveDataInspector {
    pub fn new(custom_patterns: &[(SecurityCategory, String)]) -> Result<Self, regex::Error> {
        let mut patterns = vec![
            (
                SecurityCategory::SensitivePersonalData,
                Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b")?,
            ),
            (
                SecurityCategory::SensitivePersonalData,
                Regex::new(
                    r"(?x)\b(?:\+?[1-9]\d{0,2}[ .-]?)?(?:\(?\d{2,4}\)?[ .-]?)?\d{3,4}[ .-]?\d{4}\b",
                )?,
            ),
        ];
        for (category, pattern) in custom_patterns {
            if pattern.len() <= 4096 {
                patterns.push((*category, Regex::new(pattern)?));
            }
        }
        Ok(Self {
            patterns,
            ip_pattern: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")?,
        })
    }
}

#[async_trait]
impl SecurityInspector for SensitiveDataInspector {
    fn id(&self) -> &str {
        "sensitive_data"
    }

    async fn inspect(
        &self,
        _: &InspectionContext,
        content: &InspectionContent,
    ) -> Result<Vec<SecurityFinding>, SecurityError> {
        let text = match content {
            InspectionContent::PromptText(value) | InspectionContent::ModelOutput(value) => {
                value.clone()
            }
            InspectionContent::StructuredInput(value)
            | InspectionContent::ToolArguments(value)
            | InspectionContent::ToolResult(value) => {
                serde_json::to_string(value).unwrap_or_default()
            }
        };
        let mut grouped =
            std::collections::HashMap::<SecurityCategory, Vec<SanitizedEvidence>>::new();
        for (category, pattern) in &self.patterns {
            for found in security_regex::find(std::slice::from_ref(pattern), &text, 32) {
                grouped
                    .entry(*category)
                    .or_default()
                    .push(to_evidence(found));
            }
        }
        for found in self
            .ip_pattern
            .find_iter(&text)
            .filter(|found| found.as_str().parse::<std::net::IpAddr>().is_ok())
        {
            grouped
                .entry(SecurityCategory::SensitivePersonalData)
                .or_default()
                .push(to_evidence(security_regex::safe_match(
                    &text,
                    found.start(),
                    found.end(),
                )))
        }
        let card =
            Regex::new(r"\b(?:\d[ -]*?){13,19}\b").map_err(|_| SecurityError::InspectionFailed)?;
        for found in card.find_iter(&text).filter(|found| luhn(found.as_str())) {
            grouped
                .entry(SecurityCategory::FinancialData)
                .or_default()
                .push(to_evidence(security_regex::safe_match(
                    &text,
                    found.start(),
                    found.end(),
                )));
        }
        Ok(grouped
            .into_iter()
            .map(|(category, evidence)| SecurityFinding {
                finding_id: FindingId::new(),
                inspector_id: self.id().into(),
                category,
                severity: SecuritySeverity::Medium,
                confidence: 0.9,
                evidence,
                recommended_action: SecurityAction::Redact,
                metadata: Default::default(),
            })
            .collect())
    }
}

fn luhn(value: &str) -> bool {
    let digits = value
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|byte| byte - b'0')
        .collect::<Vec<_>>();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    digits
        .iter()
        .rev()
        .enumerate()
        .map(|(index, digit)| {
            if index % 2 == 1 {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                *digit
            }
        })
        .map(u32::from)
        .sum::<u32>()
        % 10
        == 0
}

fn to_evidence(value: security_regex::SafeMatch) -> SanitizedEvidence {
    SanitizedEvidence {
        redacted: value.redacted,
        sha256: value.sha256,
        start: Some(value.start),
        end: Some(value.end),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn detects_email_valid_card_and_valid_ip() {
        let inspector = SensitiveDataInspector::new(&[]).unwrap();
        let context = InspectionContext {
            request_id: uuid::Uuid::now_v7(),
            tenant_id: "t".into(),
            project_id: None,
            principal_id: "p".into(),
            stage: "llm_input".into(),
            tool_risk: None,
        };
        let findings = inspector
            .inspect(
                &context,
                &InspectionContent::PromptText(
                    "alice@example.com 4111 1111 1111 1111 192.0.2.1 999.999.999.999".into(),
                ),
            )
            .await
            .unwrap();
        assert!(
            findings
                .iter()
                .any(|value| value.category == SecurityCategory::FinancialData)
        );
        assert!(
            findings
                .iter()
                .any(|value| value.category == SecurityCategory::SensitivePersonalData)
        );
    }
}
