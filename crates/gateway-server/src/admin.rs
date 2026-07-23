use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use gateway_admin::{AdminError, AdminService, ListQuery, Mutation, OperationalView, ResourceKind};
use serde_json::Value;
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
        .route("/admin/usage/reservations", get(reservations))
        .route("/admin/usage/mcp-invocations", get(mcp_invocations))
        .route("/admin/audit-events", get(audit_events))
        .route("/admin/summary", get(summary))
        .route("/admin/system", get(system))
        .route("/admin/billing/webhooks", get(billing_webhooks))
        .route("/admin/billing/outbox", get(billing_outbox))
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
operational!(reservations, OperationalView::Reservations);
operational!(mcp_invocations, OperationalView::McpInvocations);
operational!(audit_events, OperationalView::AuditEvents);
operational!(summary, OperationalView::Summary);
operational!(billing_webhooks, OperationalView::BillingWebhooks);
operational!(billing_outbox, OperationalView::BillingOutbox);

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
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let mut value = admin(&state)?
        .operational(&principal, OperationalView::System, query)
        .await
        .map_err(map_admin)?;
    let provider = value
        .get_mut("providers")
        .and_then(Value::as_array_mut)
        .and_then(|items| {
            items
                .iter()
                .position(|item| item.get("provider_id").and_then(Value::as_str) == Some(&id))
                .map(|index| items.remove(index))
        })
        .ok_or_else(|| ApiError::not_found("provider not found"))?;
    Ok(Json(provider))
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
        AdminError::Unavailable => {
            ApiError::service_unavailable("administration repository unavailable")
        }
    }
}
