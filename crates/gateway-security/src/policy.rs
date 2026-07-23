use std::collections::HashMap;

use gateway_types::{SecurityAction, SecurityCategory, SecuritySeverity};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub enabled: bool,
    pub fail_open: bool,
    pub inspect_llm_input: bool,
    pub inspect_llm_output: bool,
    pub inspect_mcp_arguments: bool,
    pub inspect_mcp_results: bool,
    pub create_incidents: bool,
    pub maximum_content_bytes: usize,
    #[serde(default)]
    pub actions: HashMap<SecurityCategory, HashMap<SecuritySeverity, SecurityAction>>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            fail_open: false,
            inspect_llm_input: true,
            inspect_llm_output: false,
            inspect_mcp_arguments: true,
            inspect_mcp_results: true,
            create_incidents: true,
            maximum_content_bytes: 1_048_576,
            actions: HashMap::new(),
        }
    }
}

impl SecurityPolicy {
    pub fn action(
        &self,
        category: SecurityCategory,
        severity: SecuritySeverity,
        recommended: SecurityAction,
    ) -> SecurityAction {
        self.actions
            .get(&category)
            .and_then(|items| items.get(&severity))
            .copied()
            .unwrap_or(recommended)
    }

    pub fn restrict_with(&self, child: &Self) -> Self {
        let mut result = self.clone();
        result.enabled |= child.enabled;
        result.fail_open &= child.fail_open;
        result.inspect_llm_input |= child.inspect_llm_input;
        result.inspect_llm_output |= child.inspect_llm_output;
        result.inspect_mcp_arguments |= child.inspect_mcp_arguments;
        result.inspect_mcp_results |= child.inspect_mcp_results;
        result.create_incidents |= child.create_incidents;
        result.maximum_content_bytes = result
            .maximum_content_bytes
            .min(child.maximum_content_bytes);
        for (category, severities) in &child.actions {
            for (severity, action) in severities {
                let current = result
                    .actions
                    .entry(*category)
                    .or_default()
                    .entry(*severity)
                    .or_insert(SecurityAction::Allow);
                if action.priority() > current.priority() {
                    *current = *action;
                }
            }
        }
        result
    }
}

pub fn resolve_security_hierarchy(
    policies: impl IntoIterator<Item = SecurityPolicy>,
) -> SecurityPolicy {
    let mut policies = policies.into_iter();
    policies
        .next()
        .map_or_else(SecurityPolicy::default, |first| {
            policies.fold(first, |current, next| current.restrict_with(&next))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn children_only_tighten_security() {
        let parent = SecurityPolicy {
            fail_open: true,
            inspect_llm_output: false,
            ..Default::default()
        };
        let child = SecurityPolicy {
            fail_open: false,
            inspect_llm_output: true,
            maximum_content_bytes: 100,
            ..parent.clone()
        };
        let result = resolve_security_hierarchy([parent, child]);
        assert!(!result.fail_open);
        assert!(result.inspect_llm_output);
        assert_eq!(result.maximum_content_bytes, 100)
    }
}
