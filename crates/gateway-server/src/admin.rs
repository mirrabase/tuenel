use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use gateway_admin::{AdminError, AdminService, ListQuery, Mutation, OperationalView, ResourceKind};
use gateway_providers::{ProviderError, ProviderHealthStatus};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{ApiError, AppState, RequestId, admin_principal};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/projects", get(projects).post(create_project))
        .route(
            "/admin/projects/{id}",
            patch(update_project).delete(retire_project),
        )
        .route("/admin/providers", get(providers).post(create_provider))
        .route(
            "/admin/providers/{id}/models",
            get(provider_models).post(refresh_provider_models),
        )
        .route(
            "/admin/providers/{id}",
            patch(update_provider).delete(retire_provider),
        )
        .route("/admin/providers/{id}/check", post(check_provider))
        .route("/admin/model-routes", get(model_routes).post(create_route))
        .route(
            "/admin/model-routes/{id}",
            patch(update_route).delete(retire_route),
        )
        .route("/admin/model-prices", get(prices).post(create_price))
        .route(
            "/admin/model-prices/{id}",
            patch(update_price).delete(retire_price),
        )
        .route("/admin/policies", get(policies).post(create_policy))
        .route(
            "/admin/policies/{id}",
            patch(update_policy).delete(retire_policy),
        )
        .route("/admin/quota-limits", get(quotas).post(create_quota))
        .route(
            "/admin/quota-limits/{id}",
            patch(update_quota).delete(retire_quota),
        )
        .route("/admin/tenants", get(tenants))
        .route("/auth/tenants/{tenant_id}/members", get(members))
        .route("/admin/virtual-keys", get(virtual_keys))
        .route("/admin/usage/summary", get(usage_summary))
        .route("/admin/usage/series", get(usage_series))
        .route("/admin/usage/events", get(usage_events))
        .route("/admin/usage/breakdowns", get(usage_breakdowns))
        .route("/admin/provider-health", get(provider_health))
        .route("/admin/usage/reservations", get(reservations))
        .route("/admin/usage/mcp-invocations", get(mcp_invocations))
        .route("/admin/audit-events", get(audit_events))
        .route("/admin/summary", get(summary))
        .route("/admin/system", get(system))
        .route("/admin/billing/webhooks", get(billing_webhooks))
        .route("/admin/billing/outbox", get(billing_outbox))
        .route("/admin/billing/overview", get(billing_overview))
        .route("/admin/billing/invoices", get(billing_invoices))
        .route(
            "/admin/billing/outbox/{event_id}/retry",
            post(retry_billing),
        )
}

macro_rules! resources {
    ($list:ident,$create:ident,$update:ident,$retire:ident,$kind:expr) => {
        async fn $list(
            State(state): State<AppState>,
            headers: HeaderMap,
            Query(query): Query<ListQuery>,
        ) -> Result<Json<gateway_admin::Page>, ApiError> {
            let principal = admin_principal(&state, &headers).await?;
            admin(&state)?
                .list(&principal, $kind, query)
                .await
                .map(Json)
                .map_err(map_admin)
        }

        async fn $create(
            State(state): State<AppState>,
            axum::Extension(request_id): axum::Extension<RequestId>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> Result<Response, ApiError> {
            let principal = admin_principal(&state, &headers).await?;
            let result = admin(&state)?
                .create(&principal, $kind, body, request_id.0)
                .await
                .map_err(map_admin)?;
            mutation_response(StatusCode::CREATED, result)
        }

        async fn $update(
            State(state): State<AppState>,
            axum::Extension(request_id): axum::Extension<RequestId>,
            Path(id): Path<String>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> Result<Response, ApiError> {
            let principal = admin_principal(&state, &headers).await?;
            let result = admin(&state)?
                .update(
                    &principal,
                    $kind,
                    &id,
                    if_match(&headers)?,
                    body,
                    request_id.0,
                )
                .await
                .map_err(map_admin)?;
            mutation_response(StatusCode::OK, result)
        }

        async fn $retire(
            State(state): State<AppState>,
            axum::Extension(request_id): axum::Extension<RequestId>,
            Path(id): Path<String>,
            headers: HeaderMap,
        ) -> Result<Response, ApiError> {
            let principal = admin_principal(&state, &headers).await?;
            let result = admin(&state)?
                .retire(&principal, $kind, &id, if_match(&headers)?, request_id.0)
                .await
                .map_err(map_admin)?;
            mutation_response(StatusCode::OK, result)
        }
    };
}

resources!(
    projects,
    create_project,
    update_project,
    retire_project,
    ResourceKind::Projects
);
resources!(
    providers,
    create_provider,
    update_provider,
    retire_provider,
    ResourceKind::Providers
);
resources!(
    model_routes,
    create_route,
    update_route,
    retire_route,
    ResourceKind::ModelRoutes
);
resources!(
    prices,
    create_price,
    update_price,
    retire_price,
    ResourceKind::ModelPrices
);
resources!(
    policies,
    create_policy,
    update_policy,
    retire_policy,
    ResourceKind::Policies
);
resources!(
    quotas,
    create_quota,
    update_quota,
    retire_quota,
    ResourceKind::QuotaLimits
);

macro_rules! operational {
    ($handler:ident,$view:expr) => {
        async fn $handler(
            State(state): State<AppState>,
            headers: HeaderMap,
            Query(query): Query<ListQuery>,
        ) -> Result<Json<Value>, ApiError> {
            let principal = admin_principal(&state, &headers).await?;
            admin(&state)?
                .operational(&principal, $view, query)
                .await
                .map(Json)
                .map_err(map_admin)
        }
    };
}

operational!(tenants, OperationalView::Tenants);
operational!(virtual_keys, OperationalView::VirtualKeys);
operational!(usage_summary, OperationalView::UsageSummary);
operational!(usage_series, OperationalView::UsageSeries);
operational!(usage_events, OperationalView::UsageEvents);
operational!(usage_breakdowns, OperationalView::UsageBreakdowns);
operational!(provider_health, OperationalView::ProviderHealth);
operational!(reservations, OperationalView::Reservations);
operational!(mcp_invocations, OperationalView::McpInvocations);
operational!(audit_events, OperationalView::AuditEvents);
operational!(summary, OperationalView::Summary);
operational!(billing_webhooks, OperationalView::BillingWebhooks);
operational!(billing_outbox, OperationalView::BillingOutbox);
operational!(billing_overview, OperationalView::BillingOverview);
operational!(billing_invoices, OperationalView::BillingInvoices);

async fn system(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let mut value = admin(&state)?
        .operational(&principal, OperationalView::System, query)
        .await
        .map_err(map_admin)?;
    value["runtime"] =
        serde_json::to_value(state.gateway.status()).map_err(|_| ApiError::internal())?;
    value["otel_enabled"] = Value::Bool(std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_some());
    value["deployment_mode"] = Value::String(state.deployment_mode.clone());
    Ok(Json(value))
}

async fn members(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
    headers: HeaderMap,
    Query(mut query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    query.tenant_id = Some(tenant_id);
    let principal = admin_principal(&state, &headers).await?;
    admin(&state)?
        .operational(&principal, OperationalView::Members, query)
        .await
        .map(Json)
        .map_err(map_admin)
}

async fn check_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    admin_principal(&state, &headers).await?;
    let repository = state
        .provider_health
        .as_deref()
        .ok_or_else(|| ApiError::service_unavailable("provider health unavailable"))?;
    let health = state
        .gateway
        .check_provider(&id, repository)
        .await
        .map_err(|error| match error {
            ProviderError::Unavailable => {
                ApiError::not_found("provider is not active in the gateway runtime")
            }
            _ => ApiError::service_unavailable("provider health check unavailable"),
        })?;
    Ok(Json(json!({
        "provider_id": id,
        "status": match health.status {
            ProviderHealthStatus::Healthy => "healthy",
            ProviderHealthStatus::Degraded => "degraded",
            ProviderHealthStatus::Unhealthy => "unhealthy",
            ProviderHealthStatus::Unknown => "unknown",
        },
        "consecutive_failures": health.consecutive_failures,
        "latest_success_at": health.latest_success_at,
        "latest_failure_at": health.latest_failure_at,
        "updated_at": chrono::Utc::now(),
    })))
}

async fn provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let models = state
        .gateway
        .list_provider_models(&id)
        .await
        .map_err(|error| match error {
            ProviderError::Unavailable => {
                ApiError::not_found("provider is not active in the gateway runtime")
            }
            ProviderError::Timeout => ApiError::new(
                StatusCode::GATEWAY_TIMEOUT,
                "server_error",
                "upstream_timeout",
                "provider model discovery timed out",
            ),
            _ => ApiError::service_unavailable("provider model discovery unavailable"),
        })?;
    let mut models = models;
    models.sort();
    models.dedup();
    admin(&state)?
        .cache_provider_models(&principal, &id, &models)
        .await
        .map_err(map_admin)?;
    Ok(Json(json!({
        "data": models.into_iter().map(|id| json!({ "id": id })).collect::<Vec<_>>(),
        "source": "upstream",
        "synced_at": chrono::Utc::now(),
    })))
}

async fn refresh_provider_models(
    state: State<AppState>,
    path: Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    provider_models(state, path, headers).await
}

async fn retry_billing(
    State(state): State<AppState>,
    axum::Extension(request_id): axum::Extension<RequestId>,
    Path(event_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let result = admin(&state)?
        .retry_billing(&principal, event_id, request_id.0)
        .await
        .map_err(map_admin)?;
    mutation_response(StatusCode::ACCEPTED, result)
}

fn admin(state: &AppState) -> Result<&AdminService, ApiError> {
    state
        .admin
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("administration unavailable"))
}

fn if_match(headers: &HeaderMap) -> Result<u64, ApiError> {
    headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim_matches('"').parse().ok())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::PRECONDITION_REQUIRED,
                "invalid_request_error",
                "if_match_required",
                "If-Match resource version is required",
            )
        })
}

fn mutation_response(status: StatusCode, result: Mutation) -> Result<Response, ApiError> {
    let version = result
        .resource
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(ApiError::internal)?;
    let mut response = (status, Json(result)).into_response();
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{version}\"")).map_err(|_| ApiError::internal())?,
    );
    Ok(response)
}

fn map_admin(error: AdminError) -> ApiError {
    match error {
        AdminError::Invalid => ApiError::invalid("invalid administration request"),
        AdminError::Forbidden => ApiError::forbidden("administration operation is not permitted"),
        AdminError::NotFound => ApiError::not_found("administration resource not found"),
        AdminError::Conflict => ApiError::conflict("administration resource version conflict"),
        AdminError::PlanLimit => {
            ApiError::plan_limit("managed plan resource limit exceeded; see /billing")
        }
        AdminError::Unavailable => {
            ApiError::service_unavailable("administration repository unavailable")
        }
    }
}
