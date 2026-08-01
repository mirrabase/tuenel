//! Model alias routing and ordered fallback selection.

use gateway_providers::ProviderError;

use gateway_types::ModelRoute;
use thiserror::Error;

/// Single-route model registry.
#[derive(Clone, Debug)]
pub struct StaticRouter {
    route: ModelRoute,
}

/// One ordered provider target for a model alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteTarget {
    pub tenant_id: Option<String>,
    pub project_id: Option<String>,
    pub provider: String,
    pub requested_model: String,
    pub upstream_model: String,
    pub priority: u32,
    pub enabled: bool,
}

/// Ordered route plan. The first compatible enabled target is attempted first.
#[derive(Clone, Debug, Default)]
pub struct RoutePlan {
    targets: Vec<RouteTarget>,
}

impl RoutePlan {
    pub fn new(mut targets: Vec<RouteTarget>) -> Result<Self, RoutingError> {
        if targets.is_empty() {
            return Err(RoutingError::NoTargets);
        }
        targets.sort_by_key(|target| target.priority);
        Ok(Self { targets })
    }

    pub fn targets(&self) -> impl Iterator<Item = &RouteTarget> {
        self.targets.iter().filter(|target| target.enabled)
    }

    pub fn route(&self, model: &str) -> Result<Vec<ModelRoute>, RoutingError> {
        self.route_for(None, None, model)
    }

    pub fn route_for(
        &self,
        tenant_id: Option<&str>,
        project_id: Option<&str>,
        model: &str,
    ) -> Result<Vec<ModelRoute>, RoutingError> {
        let rank = |target: &RouteTarget| {
            if target.tenant_id.as_deref() == tenant_id
                && target.project_id.as_deref() == project_id
                && project_id.is_some()
            {
                2
            } else if target.tenant_id.as_deref() == tenant_id
                && target.project_id.is_none()
                && tenant_id.is_some()
            {
                1
            } else if target.tenant_id.is_none() && target.project_id.is_none() {
                0
            } else {
                -1
            }
        };
        let selected_rank = self
            .targets()
            .filter(|target| target.requested_model == model)
            .map(rank)
            .max()
            .unwrap_or(-1);
        let routes = self
            .targets()
            .filter(|target| target.requested_model == model && rank(target) == selected_rank)
            .map(|target| ModelRoute {
                provider: target.provider.clone(),
                requested_model: target.requested_model.clone(),
                upstream_model: target.upstream_model.clone(),
            })
            .collect::<Vec<_>>();
        (!routes.is_empty())
            .then_some(routes)
            .ok_or(RoutingError::UnknownModel)
    }
}

/// Only transient provider failures may trigger an ordered fallback.
pub fn retryable(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::Timeout
            | ProviderError::Transport
            | ProviderError::RateLimited
            | ProviderError::Upstream(500..=599)
            | ProviderError::UpstreamRejected {
                status: 500..=599,
                ..
            }
    )
}

impl StaticRouter {
    /// Construct a router for one public alias.
    pub fn new(provider: String, requested_model: String, upstream_model: String) -> Self {
        Self {
            route: ModelRoute {
                provider,
                requested_model,
                upstream_model,
            },
        }
    }

    /// Resolve a public model alias.
    pub fn resolve(&self, model: &str) -> Result<ModelRoute, RoutingError> {
        (model == self.route.requested_model)
            .then(|| self.route.clone())
            .ok_or(RoutingError::UnknownModel)
    }

    /// Return the sole public route.
    pub fn route(&self) -> &ModelRoute {
        &self.route
    }
}

/// Routing failure.
#[derive(Clone, Debug, Error)]
pub enum RoutingError {
    /// Client requested an unknown model alias.
    #[error("model is not available")]
    UnknownModel,
    #[error("model route has no enabled targets")]
    NoTargets,
}

#[cfg(test)]
mod tests {
    use super::{RoutePlan, RouteTarget, retryable};
    use gateway_providers::ProviderError;

    #[test]
    fn orders_enabled_targets_and_rejects_unknown_alias() {
        let plan = RoutePlan::new(vec![
            RouteTarget {
                tenant_id: None,
                project_id: None,
                provider: "backup".into(),
                requested_model: "fast".into(),
                upstream_model: "b".into(),
                priority: 2,
                enabled: true,
            },
            RouteTarget {
                tenant_id: None,
                project_id: None,
                provider: "primary".into(),
                requested_model: "fast".into(),
                upstream_model: "a".into(),
                priority: 1,
                enabled: true,
            },
        ])
        .unwrap();
        let routes = plan.route("fast").unwrap();
        assert_eq!(routes[0].provider, "primary");
        assert!(plan.route("missing").is_err());
    }

    #[test]
    fn project_targets_override_tenant_and_global_targets() {
        let make = |provider: &str, tenant: Option<&str>, project: Option<&str>| RouteTarget {
            tenant_id: tenant.map(str::to_owned),
            project_id: project.map(str::to_owned),
            provider: provider.into(),
            requested_model: "alias".into(),
            upstream_model: provider.into(),
            priority: 1,
            enabled: true,
        };
        let plan = RoutePlan::new(vec![
            make("global", None, None),
            make("tenant", Some("t"), None),
            make("project", Some("t"), Some("p")),
        ])
        .unwrap();
        assert_eq!(
            plan.route_for(Some("t"), Some("p"), "alias").unwrap()[0].provider,
            "project"
        );
        assert_eq!(
            plan.route_for(Some("t"), Some("other"), "alias").unwrap()[0].provider,
            "tenant"
        );
    }

    #[test]
    fn fallback_only_accepts_transient_failures() {
        assert!(retryable(&ProviderError::Timeout));
        assert!(retryable(&ProviderError::Upstream(503)));
        assert!(!retryable(&ProviderError::Upstream(400)));
        assert!(!retryable(&ProviderError::Unsupported));
    }
}
