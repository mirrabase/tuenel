use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use gateway_types::{GatewayMcpTool, McpServerId, Principal, SecurityAction, ToolRiskLevel};
use serde::{Deserialize, Serialize};

use crate::McpError;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct McpPolicyRecord {
    pub policy_id: gateway_types::McpPolicyId,
    pub tenant_id: String,
    pub name: String,
    pub policy: McpPolicy,
    pub scope_kind: String,
    pub scope_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct McpPolicy {
    #[serde(default)]
    pub allowed_servers: HashSet<McpServerId>,
    #[serde(default)]
    pub denied_servers: HashSet<McpServerId>,
    #[serde(default)]
    pub allowed_tools: HashSet<String>,
    #[serde(default)]
    pub denied_tools: HashSet<String>,
    #[serde(default)]
    pub tool_actions: HashMap<String, SecurityAction>,
    #[serde(default)]
    pub risk_overrides: HashMap<String, ToolRiskLevel>,
    #[serde(default)]
    pub argument_restrictions: HashMap<String, HashMap<String, Vec<serde_json::Value>>>,
    pub maximum_calls_per_minute: Option<u64>,
    pub maximum_calls_per_day: Option<u64>,
    pub maximum_server_concurrent_calls: Option<u64>,
    pub maximum_tool_concurrent_calls: Option<u64>,
    pub maximum_request_bytes: Option<u64>,
    pub maximum_response_bytes: Option<u64>,
    pub maximum_execution_ms: Option<u64>,
    pub default_mutating_action: Option<SecurityAction>,
}

impl McpPolicy {
    pub fn restrict_with(&self, child: &Self) -> Self {
        let mut actions = self.tool_actions.clone();
        for (tool, action) in &child.tool_actions {
            actions
                .entry(tool.clone())
                .and_modify(|current| {
                    if action.priority() > current.priority() {
                        *current = *action;
                    }
                })
                .or_insert(*action);
        }
        let mut restrictions = self.argument_restrictions.clone();
        for (tool, values) in &child.argument_restrictions {
            restrictions
                .entry(tool.clone())
                .or_default()
                .extend(values.clone());
        }
        let mut risk_overrides = self.risk_overrides.clone();
        for (tool, risk) in &child.risk_overrides {
            risk_overrides
                .entry(tool.clone())
                .and_modify(|current| {
                    if risk_priority(*risk) > risk_priority(*current) {
                        *current = *risk
                    }
                })
                .or_insert(*risk);
        }
        Self {
            allowed_servers: intersect(&self.allowed_servers, &child.allowed_servers),
            denied_servers: self
                .denied_servers
                .union(&child.denied_servers)
                .copied()
                .collect(),
            allowed_tools: intersect(&self.allowed_tools, &child.allowed_tools),
            denied_tools: self
                .denied_tools
                .union(&child.denied_tools)
                .cloned()
                .collect(),
            tool_actions: actions,
            risk_overrides,
            argument_restrictions: restrictions,
            maximum_calls_per_minute: minimum(
                self.maximum_calls_per_minute,
                child.maximum_calls_per_minute,
            ),
            maximum_calls_per_day: minimum(self.maximum_calls_per_day, child.maximum_calls_per_day),
            maximum_server_concurrent_calls: minimum(
                self.maximum_server_concurrent_calls,
                child.maximum_server_concurrent_calls,
            ),
            maximum_tool_concurrent_calls: minimum(
                self.maximum_tool_concurrent_calls,
                child.maximum_tool_concurrent_calls,
            ),
            maximum_request_bytes: minimum(self.maximum_request_bytes, child.maximum_request_bytes),
            maximum_response_bytes: minimum(
                self.maximum_response_bytes,
                child.maximum_response_bytes,
            ),
            maximum_execution_ms: minimum(self.maximum_execution_ms, child.maximum_execution_ms),
            default_mutating_action: stricter(
                self.default_mutating_action,
                child.default_mutating_action,
            ),
        }
    }

    pub fn authorize(
        &self,
        server_id: McpServerId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<(), McpError> {
        if self.denied_servers.contains(&server_id)
            || (!self.allowed_servers.is_empty() && !self.allowed_servers.contains(&server_id))
        {
            return Err(McpError::ServerNotAllowed);
        }
        if self.denied_tools.contains(tool_name)
            || (!self.allowed_tools.is_empty() && !self.allowed_tools.contains(tool_name))
        {
            return Err(McpError::ToolNotAllowed);
        }
        if let Some(restrictions) = self.argument_restrictions.get(tool_name) {
            for (pointer, allowed) in restrictions {
                if arguments
                    .pointer(pointer)
                    .is_some_and(|value| !allowed.contains(value))
                {
                    return Err(McpError::ToolNotAllowed);
                }
            }
        }
        Ok(())
    }

    pub fn action(&self, tool: &GatewayMcpTool, risk: ToolRiskLevel) -> SecurityAction {
        self.tool_actions
            .get(&tool.tool_name)
            .copied()
            .unwrap_or(match risk {
                ToolRiskLevel::ReadOnly => SecurityAction::Allow,
                ToolRiskLevel::Mutating => self
                    .default_mutating_action
                    .unwrap_or(SecurityAction::RequireApproval),
                ToolRiskLevel::Destructive | ToolRiskLevel::Privileged | ToolRiskLevel::Unknown => {
                    SecurityAction::RequireApproval
                }
            })
    }
}

pub fn resolve_mcp_hierarchy(policies: impl IntoIterator<Item = McpPolicy>) -> McpPolicy {
    policies
        .into_iter()
        .fold(McpPolicy::default(), |current, next| {
            current.restrict_with(&next)
        })
}

pub fn classify_tool(tool: &GatewayMcpTool) -> ToolRiskLevel {
    if let Some(risk) = tool.annotations.administrator_risk {
        return risk;
    }
    if tool.annotations.destructive_hint == Some(true) {
        return ToolRiskLevel::Destructive;
    }
    if tool.annotations.read_only_hint == Some(true) {
        return ToolRiskLevel::ReadOnly;
    }
    let text = format!(
        "{} {}",
        tool.tool_name,
        tool.description.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();
    if ["delete", "destroy", "drop", "remove", "purge", "terminate"]
        .iter()
        .any(|word| text.contains(word))
    {
        ToolRiskLevel::Destructive
    } else if ["admin", "sudo", "permission", "credential", "secret"]
        .iter()
        .any(|word| text.contains(word))
    {
        ToolRiskLevel::Privileged
    } else if [
        "create", "update", "write", "send", "merge", "execute", "invoke",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        ToolRiskLevel::Mutating
    } else if ["get", "list", "read", "search", "find", "show", "describe"]
        .iter()
        .any(|word| text.contains(word))
    {
        ToolRiskLevel::ReadOnly
    } else {
        ToolRiskLevel::Unknown
    }
}

#[async_trait]
pub trait McpPolicyRepository: Send + Sync {
    async fn resolved_policy(&self, principal: &Principal) -> Result<McpPolicy, McpError>;
}

#[async_trait]
pub trait McpPolicyAdministration: Send + Sync {
    async fn insert_mcp_policy(&self, record: McpPolicyRecord) -> Result<(), McpError>;
    async fn mcp_policies(&self, tenant_id: &str) -> Result<Vec<McpPolicyRecord>, McpError>;
    async fn update_mcp_policy(&self, record: McpPolicyRecord) -> Result<(), McpError>;
    async fn delete_mcp_policy(
        &self,
        tenant_id: &str,
        policy_id: gateway_types::McpPolicyId,
    ) -> Result<bool, McpError>;
}

fn minimum<T: Ord + Copy>(left: Option<T>, right: Option<T>) -> Option<T> {
    left.into_iter().chain(right).min()
}
fn stricter(left: Option<SecurityAction>, right: Option<SecurityAction>) -> Option<SecurityAction> {
    left.into_iter()
        .chain(right)
        .max_by_key(|action| action.priority())
}
fn intersect<T: Eq + std::hash::Hash + Clone>(left: &HashSet<T>, right: &HashSet<T>) -> HashSet<T> {
    if left.is_empty() {
        right.clone()
    } else if right.is_empty() {
        left.clone()
    } else {
        left.intersection(right).cloned().collect()
    }
}
fn risk_priority(value: ToolRiskLevel) -> u8 {
    match value {
        ToolRiskLevel::ReadOnly => 0,
        ToolRiskLevel::Mutating => 1,
        ToolRiskLevel::Unknown => 2,
        ToolRiskLevel::Destructive => 3,
        ToolRiskLevel::Privileged => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_types::ToolAnnotations;
    fn tool(name: &str) -> GatewayMcpTool {
        GatewayMcpTool {
            server_id: McpServerId::new(),
            tool_name: name.into(),
            description: None,
            input_schema: serde_json::json!({"type":"object"}),
            annotations: ToolAnnotations::default(),
        }
    }
    #[test]
    fn authorization_and_risk_are_conservative() {
        let destructive = tool("delete_repository");
        assert_eq!(classify_tool(&destructive), ToolRiskLevel::Destructive);
        assert_eq!(
            McpPolicy::default().action(&destructive, ToolRiskLevel::Destructive),
            SecurityAction::RequireApproval
        );
        let mut policy = McpPolicy::default();
        policy.denied_tools.insert("delete_repository".into());
        assert_eq!(
            policy.authorize(
                destructive.server_id,
                &destructive.tool_name,
                &serde_json::json!({})
            ),
            Err(McpError::ToolNotAllowed)
        );
    }
    #[test]
    fn child_policy_cannot_expand_access() {
        let mut parent = McpPolicy::default();
        parent.allowed_tools.insert("read".into());
        parent.maximum_calls_per_day = Some(10);
        let mut child = McpPolicy::default();
        child.allowed_tools.extend(["read".into(), "write".into()]);
        child.maximum_calls_per_day = Some(20);
        let result = parent.restrict_with(&child);
        assert_eq!(result.allowed_tools, HashSet::from(["read".into()]));
        assert_eq!(result.maximum_calls_per_day, Some(10));
    }
}
