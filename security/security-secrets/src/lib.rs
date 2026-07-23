//! Deterministic credential and secret detection.

use async_trait::async_trait;
use gateway_security::{SecurityError, SecurityInspector};
use gateway_types::{
    FindingId, InspectionContent, InspectionContext, SanitizedEvidence, SecurityAction,
    SecurityCategory, SecurityFinding, SecuritySeverity,
};
use regex::Regex;
use security_regex::{find, safe_match};

pub struct SecretInspector {
    patterns: Vec<Regex>,
}

impl SecretInspector {
    pub fn new(custom_patterns: &[String]) -> Result<Self, regex::Error> {
        let mut patterns = vec![
            Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{16,}")?,
            Regex::new(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b")?,
            Regex::new(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----")?,
            Regex::new(r"\bAKIA[0-9A-Z]{16}\b")?,
            Regex::new(r"\bgh(?:p|o|u|s|r)_[A-Za-z0-9]{30,255}\b")?,
            Regex::new(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b")?,
            Regex::new(
                r"(?i)\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis)://[^\s:@/]+:[^\s@/]+@[^\s]+",
            )?,
            Regex::new(
                r#"(?i)\b(?:api[_-]?key|client[_-]?secret|access[_-]?token)\s*[:=]\s*['"]?[A-Za-z0-9_./+=-]{16,}"#,
            )?,
        ];
        patterns.extend(security_regex::compile(custom_patterns)?);
        Ok(Self { patterns })
    }

    fn inspect_text(&self, text: &str) -> Vec<SanitizedEvidence> {
        let mut values = find(&self.patterns, text, 32)
            .into_iter()
            .map(evidence)
            .collect::<Vec<_>>();
        for candidate in text.split(|character: char| {
            character.is_whitespace() || matches!(character, '"' | '\'' | ',' | ';')
        }) {
            if candidate.len() >= 32 && candidate.len() <= 256 && entropy(candidate) >= 4.2 {
                if let Some(start) = text.find(candidate) {
                    values.push(evidence(safe_match(text, start, start + candidate.len())));
                }
            }
            if values.len() >= 32 {
                break;
            }
        }
        values.sort_by_key(|item| item.start);
        values.dedup_by_key(|item| (item.start, item.end));
        values
    }
}

#[async_trait]
impl SecurityInspector for SecretInspector {
    fn id(&self) -> &str {
        "secrets"
    }

    async fn inspect(
        &self,
        _: &InspectionContext,
        content: &InspectionContent,
    ) -> Result<Vec<SecurityFinding>, SecurityError> {
        let text = content_text(content);
        let evidence = self.inspect_text(&text);
        Ok((!evidence.is_empty())
            .then(|| SecurityFinding {
                finding_id: FindingId::new(),
                inspector_id: self.id().into(),
                category: SecurityCategory::SecretExposure,
                severity: SecuritySeverity::High,
                confidence: 0.95,
                evidence,
                recommended_action: SecurityAction::Block,
                metadata: Default::default(),
            })
            .into_iter()
            .collect())
    }
}

fn content_text(content: &InspectionContent) -> String {
    match content {
        InspectionContent::PromptText(value) | InspectionContent::ModelOutput(value) => {
            value.clone()
        }
        InspectionContent::StructuredInput(value)
        | InspectionContent::ToolArguments(value)
        | InspectionContent::ToolResult(value) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn evidence(value: security_regex::SafeMatch) -> SanitizedEvidence {
    SanitizedEvidence {
        redacted: value.redacted,
        sha256: value.sha256,
        start: Some(value.start),
        end: Some(value.end),
    }
}

fn entropy(value: &str) -> f64 {
    let mut counts = [0_u16; 256];
    for byte in value.bytes() {
        counts[usize::from(byte)] += 1;
    }
    let length = value.len() as f64;
    counts
        .into_iter()
        .filter(|count| *count > 0)
        .map(|count| {
            let probability = f64::from(count) / length;
            -probability * probability.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn detects_without_retaining_secret() {
        let inspector = SecretInspector::new(&[]).unwrap();
        let secret = "sk-proj-0123456789abcdefghijklmnop";
        let context = InspectionContext {
            request_id: uuid::Uuid::now_v7(),
            tenant_id: "t".into(),
            project_id: None,
            principal_id: "p".into(),
            stage: "llm_input".into(),
            tool_risk: None,
        };
        let findings = inspector
            .inspect(&context, &InspectionContent::PromptText(secret.into()))
            .await
            .unwrap();
        assert_eq!(findings.len(), 1);
        let encoded = serde_json::to_string(&findings).unwrap();
        assert!(!encoded.contains(secret));
        assert!(encoded.contains("REDACTED"));
    }
}
