//! Provider-neutral control-plane application service.

use std::{fmt, sync::Arc};

use async_trait::async_trait;
use chrono::DateTime;
use gateway_types::Principal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_LIMIT: u8 = 50;
const MAX_LIMIT: u8 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Projects,
    Providers,
    ModelRoutes,
    ModelPrices,
    Policies,
    QuotaLimits,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Projects => "projects",
            Self::Providers => "providers",
            Self::ModelRoutes => "model_routes",
            Self::ModelPrices => "model_prices",
            Self::Policies => "policies",
            Self::QuotaLimits => "quota_limits",
        }
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ListQuery {
    pub tenant_id: Option<String>,
    pub project_id: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub status: Option<String>,
    pub query: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u8>,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl ListQuery {
    pub fn limit(&self) -> u8 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Page {
    pub data: Vec<Value>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Mutation {
    #[serde(flatten)]
    pub resource: Value,
    pub audit_id: Uuid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalView {
    Tenants,
    Members,
    VirtualKeys,
    UsageSummary,
    UsageSeries,
    UsageEvents,
    UsageBreakdowns,
    ProviderHealth,
    Reservations,
    McpInvocations,
    AuditEvents,
    Summary,
    System,
    BillingWebhooks,
    BillingOutbox,
    BillingOverview,
    BillingInvoices,
}

#[derive(Clone, Debug)]
pub struct MutationContext {
    pub actor: String,
    pub tenant_id: Option<String>,
    pub request_id: Uuid,
    pub gateway_admin: bool,
}

#[async_trait]
pub trait AdminRepository: Send + Sync {
    async fn resource_in_scope(
        &self,
        kind: ResourceKind,
        id: &str,
        tenant_id: Option<&str>,
    ) -> Result<bool, AdminError>;
    async fn resource_secret_ref(
        &self,
        kind: ResourceKind,
        id: &str,
    ) -> Result<Option<(String, gateway_types::SecretRef)>, AdminError>;
    async fn list_resources(
        &self,
        kind: ResourceKind,
        query: &ListQuery,
    ) -> Result<Page, AdminError>;
    async fn create_resource(
        &self,
        kind: ResourceKind,
        body: Value,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError>;
    async fn update_resource(
        &self,
        kind: ResourceKind,
        id: &str,
        version: u64,
        body: Value,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError>;
    async fn retire_resource(
        &self,
        kind: ResourceKind,
        id: &str,
        version: u64,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError>;
    async fn operational(
        &self,
        view: OperationalView,
        query: &ListQuery,
    ) -> Result<Value, AdminError>;
    async fn retry_billing(
        &self,
        event_id: Uuid,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError>;
    async fn cache_provider_models(
        &self,
        provider_id: &str,
        tenant_id: &str,
        models: &[String],
    ) -> Result<(), AdminError>;
}

#[derive(Clone)]
pub struct AdminService {
    repository: Arc<dyn AdminRepository>,
    gateway_admin_role: String,
    secrets: Option<gateway_secrets::SecretService>,
}

impl AdminService {
    pub fn new(
        repository: Arc<dyn AdminRepository>,
        gateway_admin_role: impl Into<String>,
    ) -> Self {
        Self {
            repository,
            gateway_admin_role: gateway_admin_role.into(),
            secrets: None,
        }
    }

    pub fn with_secrets(mut self, secrets: gateway_secrets::SecretService) -> Self {
        self.secrets = Some(secrets);
        self
    }

    pub async fn authorize_resource(
        &self,
        principal: &Principal,
        kind: ResourceKind,
        id: &str,
    ) -> Result<(), AdminError> {
        self.authorize(principal, Some(&principal.tenant_id), false)?;
        if self.is_gateway_admin(principal) {
            return Ok(());
        }
        self.repository
            .resource_in_scope(kind, id, Some(&principal.tenant_id))
            .await?
            .then_some(())
            .ok_or(AdminError::NotFound)
    }

    pub async fn cache_provider_models(
        &self,
        principal: &Principal,
        provider_id: &str,
        models: &[String],
    ) -> Result<(), AdminError> {
        self.authorize(principal, Some(&principal.tenant_id), false)?;
        self.repository
            .cache_provider_models(provider_id, &principal.tenant_id, models)
            .await
    }

    pub async fn list(
        &self,
        principal: &Principal,
        kind: ResourceKind,
        mut query: ListQuery,
    ) -> Result<Page, AdminError> {
        self.authorize(principal, query.tenant_id.as_deref(), false)?;
        bind_tenant(principal, &mut query, self.is_gateway_admin(principal));
        self.repository
            .list_resources(kind, &query)
            .await
            .map(|mut page| {
                for resource in &mut page.data {
                    redact(resource);
                }
                page
            })
    }

    pub async fn create(
        &self,
        principal: &Principal,
        kind: ResourceKind,
        mut body: Value,
        request_id: Uuid,
    ) -> Result<Mutation, AdminError> {
        let tenant = body
            .get("tenant_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.authorize(principal, tenant.as_deref(), true)?;
        validate_resource(kind, &body)?;
        self.validate_provider_scope(principal, kind, tenant.as_deref(), &body)
            .await?;
        let mut stored_secret = None;
        if kind == ResourceKind::Providers {
            let object = body.as_object_mut().ok_or(AdminError::Invalid)?;
            let id = object
                .entry("id")
                .or_insert_with(|| Value::String(Uuid::now_v7().to_string()))
                .as_str()
                .ok_or(AdminError::Invalid)?
                .to_owned();
            if let Some(credential) = object.remove("credential") {
                let credential = credential
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .ok_or(AdminError::Invalid)?;
                let tenant_id = tenant.as_deref().unwrap_or(&principal.tenant_id);
                let secret_ref = self
                    .secrets
                    .as_ref()
                    .ok_or(AdminError::Unavailable)?
                    .store(
                        tenant_id,
                        &format!("provider:{id}:credential"),
                        credential.as_bytes(),
                    )
                    .await
                    .map_err(|_| AdminError::Unavailable)?;
                object.insert("secret_ref".into(), Value::String(secret_ref.0.clone()));
                object.insert(
                    "secret_tenant_id".into(),
                    Value::String(tenant_id.to_owned()),
                );
                stored_secret = Some((tenant_id.to_owned(), secret_ref));
            }
        }
        let result = self
            .repository
            .create_resource(
                kind,
                body,
                &context(
                    principal,
                    tenant.as_deref(),
                    request_id,
                    self.is_gateway_admin(principal),
                ),
            )
            .await;
        if result.is_err() {
            if let (Some(secrets), Some((tenant_id, secret_ref))) = (&self.secrets, stored_secret) {
                let _ = secrets.delete(&tenant_id, &secret_ref).await;
            }
        }
        result.map(sanitize)
    }

    pub async fn update(
        &self,
        principal: &Principal,
        kind: ResourceKind,
        id: &str,
        version: u64,
        mut body: Value,
        request_id: Uuid,
    ) -> Result<Mutation, AdminError> {
        let tenant = body
            .get("tenant_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.authorize(principal, tenant.as_deref(), true)?;
        validate_resource(kind, &body)?;
        self.validate_provider_scope(principal, kind, tenant.as_deref(), &body)
            .await?;
        let previous = if kind == ResourceKind::Providers {
            self.repository.resource_secret_ref(kind, id).await?
        } else {
            None
        };
        let mut stored_secret = None;
        if kind == ResourceKind::Providers {
            let object = body.as_object_mut().ok_or(AdminError::Invalid)?;
            if let Some(credential) = object.remove("credential") {
                let credential = credential
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .ok_or(AdminError::Invalid)?;
                let tenant_id = tenant
                    .as_deref()
                    .or(previous.as_ref().map(|item| item.0.as_str()))
                    .unwrap_or(&principal.tenant_id);
                let secret_ref = self
                    .secrets
                    .as_ref()
                    .ok_or(AdminError::Unavailable)?
                    .store(
                        tenant_id,
                        &format!("provider:{id}:credential"),
                        credential.as_bytes(),
                    )
                    .await
                    .map_err(|_| AdminError::Unavailable)?;
                object.insert("secret_ref".into(), Value::String(secret_ref.0.clone()));
                object.insert(
                    "secret_tenant_id".into(),
                    Value::String(tenant_id.to_owned()),
                );
                stored_secret = Some((tenant_id.to_owned(), secret_ref));
            }
        }
        let result = self
            .repository
            .update_resource(
                kind,
                id,
                version,
                body,
                &context(
                    principal,
                    tenant.as_deref(),
                    request_id,
                    self.is_gateway_admin(principal),
                ),
            )
            .await;
        match result {
            Ok(result) => {
                if stored_secret.is_some() {
                    if let (Some(secrets), Some((tenant_id, secret_ref))) =
                        (&self.secrets, previous)
                    {
                        let _ = secrets.delete(&tenant_id, &secret_ref).await;
                    }
                }
                Ok(sanitize(result))
            }
            Err(error) => {
                if let (Some(secrets), Some((tenant_id, secret_ref))) =
                    (&self.secrets, stored_secret)
                {
                    let _ = secrets.delete(&tenant_id, &secret_ref).await;
                }
                Err(error)
            }
        }
    }

    pub async fn retire(
        &self,
        principal: &Principal,
        kind: ResourceKind,
        id: &str,
        version: u64,
        request_id: Uuid,
    ) -> Result<Mutation, AdminError> {
        self.authorize(principal, None, true)?;
        let previous = if kind == ResourceKind::Providers {
            self.repository.resource_secret_ref(kind, id).await?
        } else {
            None
        };
        let result = self
            .repository
            .retire_resource(
                kind,
                id,
                version,
                &context(
                    principal,
                    None,
                    request_id,
                    self.is_gateway_admin(principal),
                ),
            )
            .await?;
        if let (Some(secrets), Some((tenant_id, secret_ref))) = (&self.secrets, previous) {
            let _ = secrets.delete(&tenant_id, &secret_ref).await;
        }
        Ok(sanitize(result))
    }

    pub async fn operational(
        &self,
        principal: &Principal,
        view: OperationalView,
        mut query: ListQuery,
    ) -> Result<Value, AdminError> {
        let from = query
            .from
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| AdminError::Invalid)?;
        let to = query
            .to
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| AdminError::Invalid)?;
        if from.zip(to).is_some_and(|(from, to)| from > to) {
            return Err(AdminError::Invalid);
        }
        for filter in [&mut query.provider_id, &mut query.model] {
            *filter = filter
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            if filter.as_ref().is_some_and(|value| value.len() > 255) {
                return Err(AdminError::Invalid);
            }
        }
        let cross_tenant = matches!(
            view,
            OperationalView::Tenants | OperationalView::System | OperationalView::Summary
        );
        if cross_tenant && !self.is_gateway_admin(principal) {
            query.tenant_id = Some(principal.tenant_id.clone());
        }
        self.authorize(principal, query.tenant_id.as_deref(), false)?;
        bind_tenant(principal, &mut query, self.is_gateway_admin(principal));
        self.repository.operational(view, &query).await
    }

    pub async fn retry_billing(
        &self,
        principal: &Principal,
        event_id: Uuid,
        request_id: Uuid,
    ) -> Result<Mutation, AdminError> {
        self.authorize(principal, None, true)?;
        self.repository
            .retry_billing(
                event_id,
                &context(
                    principal,
                    None,
                    request_id,
                    self.is_gateway_admin(principal),
                ),
            )
            .await
    }

    fn authorize(
        &self,
        principal: &Principal,
        requested_tenant: Option<&str>,
        write: bool,
    ) -> Result<(), AdminError> {
        if self.is_gateway_admin(principal) {
            return Ok(());
        }
        if requested_tenant.is_some_and(|tenant| tenant != principal.tenant_id) {
            return Err(AdminError::Forbidden);
        }
        let allowed = principal.roles.iter().any(|role| {
            if write {
                matches!(role.as_str(), "owner" | "admin")
            } else {
                matches!(role.as_str(), "owner" | "admin" | "engineer")
            }
        });
        allowed.then_some(()).ok_or(AdminError::Forbidden)
    }

    async fn validate_provider_scope(
        &self,
        principal: &Principal,
        kind: ResourceKind,
        requested_tenant: Option<&str>,
        body: &Value,
    ) -> Result<(), AdminError> {
        if kind != ResourceKind::ModelRoutes {
            return Ok(());
        }
        let provider_id = body
            .get("provider")
            .and_then(Value::as_str)
            .ok_or(AdminError::Invalid)?;
        let tenant_id = requested_tenant.or_else(|| {
            (!self.is_gateway_admin(principal)).then_some(principal.tenant_id.as_str())
        });
        self.repository
            .resource_in_scope(ResourceKind::Providers, provider_id, tenant_id)
            .await?
            .then_some(())
            .ok_or(AdminError::NotFound)
    }

    fn is_gateway_admin(&self, principal: &Principal) -> bool {
        principal
            .roles
            .iter()
            .any(|role| role == &self.gateway_admin_role)
    }
}

fn bind_tenant(principal: &Principal, query: &mut ListQuery, gateway_admin: bool) {
    if !gateway_admin {
        query.tenant_id = Some(principal.tenant_id.clone());
    }
}

fn context(
    principal: &Principal,
    tenant: Option<&str>,
    request_id: Uuid,
    gateway_admin: bool,
) -> MutationContext {
    MutationContext {
        actor: principal.principal_id.clone(),
        tenant_id: Some(tenant.unwrap_or(&principal.tenant_id).to_owned()),
        request_id,
        gateway_admin,
    }
}

fn sanitize(mut mutation: Mutation) -> Mutation {
    redact(&mut mutation.resource);
    mutation
}

fn redact(resource: &mut Value) {
    if let Some(object) = resource.as_object_mut() {
        if object.contains_key("secret_ref") {
            object.insert("credential_configured".into(), Value::Bool(true));
        }
        object.remove("credential");
        object.remove("secret_ref");
        object.remove("secret_tenant_id");
    }
}

fn validate_resource(kind: ResourceKind, body: &Value) -> Result<(), AdminError> {
    let object = body.as_object().ok_or(AdminError::Invalid)?;
    let text = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty() && value.len() <= 255)
            .ok_or(AdminError::Invalid)
    };
    let valid_scope = || {
        if !matches!(
            text("scope_kind")?,
            "global" | "tenant" | "project" | "principal" | "virtual_key"
        ) {
            return Err(AdminError::Invalid);
        }
        text("scope_id")
    };
    match kind {
        ResourceKind::Projects => {
            text("name")?;
        }
        ResourceKind::Providers => {
            text("name").or_else(|_| text("id"))?;
            if !matches!(
                text("provider_type")?,
                "openai" | "openai_compatible" | "anthropic" | "gemini"
            ) || text("base_url")?.parse::<url::Url>().is_err()
            {
                return Err(AdminError::Invalid);
            }
        }
        ResourceKind::ModelRoutes => {
            text("provider")?;
            text("requested_model")?;
            text("upstream_model")?;
            if !object
                .get("priority")
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0 && value <= u64::from(u32::MAX))
            {
                return Err(AdminError::Invalid);
            }
        }
        ResourceKind::ModelPrices => {
            text("provider_id")?;
            text("upstream_model")?;
            for field in ["input_cost_per_million", "output_cost_per_million"] {
                if !object.get(field).is_some_and(|value| {
                    value.as_f64().is_some_and(|number| number >= 0.0)
                        || value
                            .as_str()
                            .and_then(|number| number.parse::<f64>().ok())
                            .is_some_and(|number| number >= 0.0)
                }) {
                    return Err(AdminError::Invalid);
                }
            }
            text("effective_from")?;
        }
        ResourceKind::Policies => {
            valid_scope()?;
        }
        ResourceKind::QuotaLimits => {
            valid_scope()?;
            if !matches!(text("period")?, "minute" | "day" | "month") {
                return Err(AdminError::Invalid);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AdminError {
    #[error("invalid administration request")]
    Invalid,
    #[error("administration operation is forbidden")]
    Forbidden,
    #[error("administration resource not found")]
    NotFound,
    #[error("administration resource version conflict")]
    Conflict,
    #[error("managed plan resource limit exceeded")]
    PlanLimit,
    #[error("administration repository unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::ListQuery;

    #[test]
    fn pagination_defaults_and_caps() {
        assert_eq!(ListQuery::default().limit(), 50);
        assert_eq!(
            ListQuery {
                limit: Some(0),
                ..Default::default()
            }
            .limit(),
            1
        );
        assert_eq!(
            ListQuery {
                limit: Some(255),
                ..Default::default()
            }
            .limit(),
            100
        );
    }
}
