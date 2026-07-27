//! Authorization and hierarchical policy evaluation.

use async_trait::async_trait;
use gateway_types::Principal;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Policy {
    pub allowed_models: Vec<String>,
    pub denied_models: Vec<String>,
    pub allowed_operations: Vec<String>,
    pub max_output_tokens: Option<u32>,
    pub concurrent_requests: Option<u32>,
    pub daily_token_limit: Option<u64>,
    pub monthly_token_limit: Option<u64>,
}

impl Policy {
    pub fn allows_model(&self, model: &str) -> bool {
        !self.denied_models.iter().any(|item| item == model)
            && (self.allowed_models.is_empty()
                || self.allowed_models.iter().any(|item| item == model))
    }

    pub fn allows_operation(&self, operation: &str) -> bool {
        self.allowed_operations.is_empty()
            || self.allowed_operations.iter().any(|item| item == operation)
    }

    pub fn authorize(
        &self,
        model: &str,
        operation: &str,
        output_tokens: u32,
    ) -> Result<(), PolicyError> {
        if !self.allows_model(model) || !self.allows_operation(operation) {
            return Err(PolicyError::Denied);
        }
        if self
            .max_output_tokens
            .is_some_and(|limit| output_tokens > limit)
        {
            return Err(PolicyError::Denied);
        }
        Ok(())
    }

    pub fn restrict_with(&self, child: &Self) -> Self {
        Self {
            allowed_models: intersect_allowlists(&self.allowed_models, &child.allowed_models),
            denied_models: [self.denied_models.clone(), child.denied_models.clone()].concat(),
            allowed_operations: intersect_allowlists(
                &self.allowed_operations,
                &child.allowed_operations,
            ),
            max_output_tokens: min_opt(self.max_output_tokens, child.max_output_tokens),
            concurrent_requests: min_opt(self.concurrent_requests, child.concurrent_requests),
            daily_token_limit: min_opt(self.daily_token_limit, child.daily_token_limit),
            monthly_token_limit: min_opt(self.monthly_token_limit, child.monthly_token_limit),
        }
    }
}

pub fn resolve_hierarchy(
    global: &Policy,
    tenant: &Policy,
    project: Option<&Policy>,
    principal: Option<&Policy>,
    key: Option<&Policy>,
) -> Policy {
    [Some(global), Some(tenant), project, principal, key]
        .into_iter()
        .flatten()
        .fold(Policy::default(), |current, next| {
            current.restrict_with(next)
        })
}

fn min_opt<T: Ord + Copy>(left: Option<T>, right: Option<T>) -> Option<T> {
    left.into_iter().chain(right).min()
}
fn intersect_allowlists(left: &[String], right: &[String]) -> Vec<String> {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => Vec::new(),
        (true, false) => right.to_vec(),
        (false, true) => left.to_vec(),
        (false, false) => left
            .iter()
            .filter(|item| right.contains(item))
            .cloned()
            .collect(),
    }
}

pub fn principal_scope(principal: &Principal) -> (&str, Option<&str>, Option<&str>) {
    (
        &principal.tenant_id,
        principal.project_id.as_deref(),
        principal.user_id.as_deref(),
    )
}

#[async_trait]
pub trait PolicyResolver: Send + Sync {
    async fn resolve(&self, principal: &Principal) -> Result<Policy, PolicyError>;
}

#[derive(Clone, Default)]
pub struct AllowAllPolicyResolver;

#[async_trait]
impl PolicyResolver for AllowAllPolicyResolver {
    async fn resolve(&self, _: &Principal) -> Result<Policy, PolicyError> {
        Ok(Policy::default())
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PolicyError {
    #[error("request is not permitted by policy")]
    Denied,
    #[error("policy service unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::{Policy, resolve_hierarchy};

    #[test]
    fn child_policy_only_restricts_parent() {
        let global = Policy {
            allowed_models: vec!["fast".into(), "safe".into()],
            max_output_tokens: Some(4096),
            ..Default::default()
        };
        let tenant = Policy {
            allowed_models: vec!["fast".into()],
            max_output_tokens: Some(1024),
            ..Default::default()
        };
        let result = resolve_hierarchy(&global, &tenant, None, None, None);
        assert!(result.allows_model("fast"));
        assert!(!result.allows_model("safe"));
        assert_eq!(result.max_output_tokens, Some(1024));
    }
}
