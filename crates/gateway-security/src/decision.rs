use gateway_types::{SecurityAction, SecurityFinding};

use crate::{SecurityPolicy, risk_score};

#[derive(Clone, Debug)]
pub struct SecurityDecision {
    pub action: SecurityAction,
    pub risk_score: u8,
    pub findings: Vec<SecurityFinding>,
    pub inspection_failed: bool,
}

pub fn decide(
    policy: &SecurityPolicy,
    findings: Vec<SecurityFinding>,
    inspection_failed: bool,
) -> SecurityDecision {
    let action = if inspection_failed {
        if policy.fail_open {
            SecurityAction::Warn
        } else {
            SecurityAction::Block
        }
    } else {
        findings
            .iter()
            .map(|finding| {
                policy.action(
                    finding.category,
                    finding.severity,
                    finding.recommended_action,
                )
            })
            .max_by_key(|action| action.priority())
            .unwrap_or(SecurityAction::Allow)
    };
    SecurityDecision {
        action,
        risk_score: risk_score(&findings),
        findings,
        inspection_failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_types::*;
    fn finding(action: SecurityAction) -> SecurityFinding {
        SecurityFinding {
            finding_id: FindingId::new(),
            inspector_id: "test".into(),
            category: SecurityCategory::PolicyViolation,
            severity: SecuritySeverity::High,
            confidence: 1.0,
            evidence: vec![],
            recommended_action: action,
            metadata: Default::default(),
        }
    }
    #[test]
    fn action_precedence_and_failure_mode() {
        let policy = SecurityPolicy::default();
        assert_eq!(
            decide(
                &policy,
                vec![
                    finding(SecurityAction::Warn),
                    finding(SecurityAction::Block)
                ],
                false
            )
            .action,
            SecurityAction::Block
        );
        assert_eq!(decide(&policy, vec![], true).action, SecurityAction::Block);
        let mut open = policy;
        open.fail_open = true;
        assert_eq!(decide(&open, vec![], true).action, SecurityAction::Warn);
    }
}
