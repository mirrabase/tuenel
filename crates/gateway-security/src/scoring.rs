use gateway_types::{SecurityFinding, SecuritySeverity};

pub fn risk_score(findings: &[SecurityFinding]) -> u8 {
    findings
        .iter()
        .fold(0_f32, |score, finding| {
            let (floor, band) = match finding.severity {
                SecuritySeverity::Low => (5.0, 24.0),
                SecuritySeverity::Medium => (30.0, 29.0),
                SecuritySeverity::High => (60.0, 19.0),
                SecuritySeverity::Critical => (80.0, 20.0),
            };
            score + floor + band * finding.confidence.clamp(0.0, 1.0)
        })
        .min(100.0)
        .round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_types::*;
    #[test]
    fn severity_maps_to_documented_band() {
        let finding = SecurityFinding {
            finding_id: FindingId::new(),
            inspector_id: "x".into(),
            category: SecurityCategory::PromptInjection,
            severity: SecuritySeverity::High,
            confidence: 0.5,
            evidence: vec![],
            recommended_action: SecurityAction::Block,
            metadata: Default::default(),
        };
        assert!((60..=79).contains(&risk_score(&[finding])));
    }
}
