use std::collections::HashMap;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use chrono::Utc;
use gateway_mcp::{ApprovalReference, McpError, McpPolicy, McpPolicyRecord, McpServerRecord};
use gateway_security::{
    SecurityCustomPattern, SecurityPolicy, SecurityPolicyRecord, validate_custom_pattern,
};
use gateway_types::{
    ApprovalId, ApprovalStatus, GatewayMcpInvocation, IncidentId, IncidentStatus, McpPolicyId,
    McpServerId, McpTransportType, SecurityAction, SecurityCategory, SecurityPolicyId,
    ToolAnnotations,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    ApiError, AppState, RequestId, admin_principal, authenticate, require_plan_capacity,
    require_plan_feature, write_admin_principal,
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/mcp/servers", get(admin_servers).post(create_server))
        .route(
            "/admin/mcp/servers/{server_id}",
            get(admin_server).patch(update_server).delete(delete_server),
        )
        .route(
            "/admin/mcp/servers/{server_id}/refresh",
            post(refresh_server),
        )
        .route("/admin/mcp/servers/{server_id}/health", post(health_server))
        .route(
            "/admin/mcp/servers/{server_id}/tools",
            get(admin_server_tools),
        )
        .route("/admin/mcp/tools", get(admin_tools))
        .route(
            "/admin/mcp/tools/{server_id}/{tool_name}/annotations",
            patch(update_tool_annotations),
        )
        .route(
            "/admin/mcp/policies",
            get(mcp_policies).post(create_mcp_policy),
        )
        .route(
            "/admin/mcp/policies/{policy_id}",
            patch(update_mcp_policy).delete(delete_mcp_policy),
        )
        .route("/v1/mcp/servers", get(public_servers))
        .route("/v1/mcp/tools", get(public_tools))
        .route("/v1/mcp/tools/call", post(call_tool))
        .route("/mcp", get(mcp_get).post(mcp_post).delete(mcp_delete))
        .route("/admin/approvals", get(approvals))
        .route("/admin/approvals/{approval_id}", get(approval))
        .route("/admin/approvals/{approval_id}/approve", post(approve))
        .route("/admin/approvals/{approval_id}/reject", post(reject))
        .route("/v1/gateway/approvals/{approval_id}", get(public_approval))
        .route("/v1/gateway/approvals", get(public_approvals))
        .route(
            "/admin/security/policies",
            get(security_policies).post(create_security_policy),
        )
        .route(
            "/admin/security/policies/{policy_id}",
            patch(update_security_policy).delete(delete_security_policy),
        )
        .route("/admin/security/incidents", get(incidents))
        .route(
            "/admin/security/incidents/{incident_id}",
            get(incident).patch(update_incident),
        )
        .route("/admin/security/findings", get(findings))
        .route("/admin/security/events", get(security_events))
        .route(
            "/admin/security/patterns",
            get(security_patterns).post(create_security_pattern),
        )
        .route(
            "/admin/security/patterns/{pattern_id}",
            patch(update_security_pattern).delete(delete_security_pattern),
        )
        .route(
            "/admin/security/incidents/{incident_id}/timeline",
            get(incident_timeline),
        )
}

#[derive(Debug, Deserialize)]
struct CreateServer {
    name: String,
    description: Option<String>,
    project_id: Option<String>,
    transport_type: McpTransportType,
    endpoint: Option<String>,
    command: Option<String>,
    #[serde(default)]
    arguments: Vec<String>,
    #[serde(default)]
    environment: HashMap<String, String>,
    credential: Option<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

async fn create_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateServer>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    require_plan_capacity(&state, &principal.tenant_id, "mcp_servers").await?;
    validate_server_input(&input)?;
    let server_id = McpServerId::new();
    let secrets = state
        .secrets
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("secret storage unavailable"))?;
    let credential_ref = match input.credential {
        Some(value) => Some(
            secrets
                .store(
                    &principal.tenant_id,
                    &format!("mcp:{server_id}:credential"),
                    value.as_bytes(),
                )
                .await
                .map_err(|_| ApiError::internal())?,
        ),
        None => None,
    };
    let mut environment_secret_refs = Vec::new();
    for (name, value) in input.environment {
        if !valid_environment_name(&name) {
            return Err(ApiError::invalid("invalid environment variable name"));
        }
        let encoded = serde_json::to_vec(&json!({"name":name,"value":value}))
            .map_err(|_| ApiError::internal())?;
        environment_secret_refs.push(
            secrets
                .store(
                    &principal.tenant_id,
                    &format!("mcp:{server_id}:environment"),
                    &encoded,
                )
                .await
                .map_err(|_| ApiError::internal())?,
        );
    }
    let now = Utc::now();
    let record = McpServerRecord {
        server_id,
        tenant_id: principal.tenant_id.clone(),
        project_id: input.project_id,
        name: input.name,
        description: input.description,
        transport_type: input.transport_type,
        endpoint: input.endpoint,
        command: input.command,
        arguments: input.arguments,
        environment_secret_refs,
        credential_ref,
        enabled: input.enabled,
        metadata: Default::default(),
        created_at: now,
        updated_at: now,
    };
    registry(&state)?.register(record).await.map_err(map_mcp)?;
    audit(&state)?
        .emit(
            format!("mcp.server.registered:{server_id}"),
            "mcp.server.registered",
            &principal,
            None,
            json!({"server_id":server_id}),
        )
        .await
        .map_err(|_| ApiError::internal())?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"server_id":server_id,"enabled":input.enabled})),
    ))
}

async fn admin_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(
        json!({"data":registry(&state)?.admin_safe_servers(&principal.tenant_id).await.map_err(map_mcp)?}),
    ))
}
async fn admin_server(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let server = registry(&state)?
        .admin_server_for(&principal.tenant_id, McpServerId(server_id))
        .await
        .map_err(map_mcp)?;
    Ok(Json(
        json!({"server_id":server.server_id,"name":server.name,"description":server.description,"transport_type":server.transport_type,"enabled":server.enabled}),
    ))
}

#[derive(Deserialize)]
struct UpdateServer {
    name: Option<String>,
    description: Option<String>,
    enabled: Option<bool>,
    endpoint: Option<String>,
    command: Option<String>,
    arguments: Option<Vec<String>>,
    credential: Option<String>,
    environment: Option<HashMap<String, String>>,
}
async fn update_server(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateServer>,
) -> Result<Json<Value>, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    let mut server = registry(&state)?
        .admin_server_for(&principal.tenant_id, McpServerId(server_id))
        .await
        .map_err(map_mcp)?;
    let old_credential = server.credential_ref.clone();
    let old_environment = server.environment_secret_refs.clone();
    let rotates_credential = input.credential.is_some();
    let rotates_environment = input.environment.is_some();
    if let Some(name) = input.name {
        if name.trim().is_empty() || name.len() > 255 {
            return Err(ApiError::invalid("invalid server name"));
        }
        server.name = name;
    }
    if input.description.is_some() {
        server.description = input.description;
    }
    if let Some(enabled) = input.enabled {
        server.enabled = enabled;
    }
    if let Some(endpoint) = input.endpoint {
        server.endpoint = Some(endpoint)
    }
    if let Some(command) = input.command {
        server.command = Some(command)
    }
    if let Some(arguments) = input.arguments {
        server.arguments = arguments
    }
    let secrets = state
        .secrets
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("secret storage unavailable"))?;
    if let Some(credential) = input.credential {
        server.credential_ref = Some(
            secrets
                .store(
                    &principal.tenant_id,
                    &format!("mcp:{}:credential", server.server_id),
                    credential.as_bytes(),
                )
                .await
                .map_err(|_| ApiError::internal())?,
        )
    }
    if let Some(environment) = input.environment {
        let mut refs = Vec::new();
        for (name, value) in environment {
            if !valid_environment_name(&name) {
                return Err(ApiError::invalid("invalid environment variable name"));
            }
            let encoded = serde_json::to_vec(&json!({"name":name,"value":value}))
                .map_err(|_| ApiError::internal())?;
            refs.push(
                secrets
                    .store(
                        &principal.tenant_id,
                        &format!("mcp:{}:environment", server.server_id),
                        &encoded,
                    )
                    .await
                    .map_err(|_| ApiError::internal())?,
            )
        }
        server.environment_secret_refs = refs
    }
    server.updated_at = Utc::now();
    registry(&state)?
        .update(server.clone())
        .await
        .map_err(map_mcp)?;
    if rotates_credential {
        if let Some(secret_ref) = old_credential {
            let _ = secrets.delete(&principal.tenant_id, &secret_ref).await;
        }
    }
    if rotates_environment {
        for secret_ref in old_environment {
            let _ = secrets.delete(&principal.tenant_id, &secret_ref).await;
        }
    }
    audit(&state)?
        .emit(
            format!(
                "mcp.server.updated:{server_id}:{}",
                server.updated_at.timestamp_micros()
            ),
            "mcp.server.updated",
            &principal,
            None,
            json!({"server_id":server_id}),
        )
        .await
        .map_err(|_| ApiError::internal())?;
    Ok(Json(
        json!({"server_id":server.server_id,"name":server.name,"description":server.description,"transport_type":server.transport_type,"enabled":server.enabled}),
    ))
}
async fn delete_server(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    let server = registry(&state)?
        .admin_server_for(&principal.tenant_id, McpServerId(server_id))
        .await
        .map_err(map_mcp)?;
    if registry(&state)?
        .delete(&principal.tenant_id, McpServerId(server_id))
        .await
        .map_err(map_mcp)?
    {
        if let Some(secrets) = &state.secrets {
            if let Some(secret_ref) = server.credential_ref {
                let _ = secrets.delete(&principal.tenant_id, &secret_ref).await;
            }
            for secret_ref in server.environment_secret_refs {
                let _ = secrets.delete(&principal.tenant_id, &secret_ref).await;
            }
        }
        audit(&state)?
            .emit(
                format!("mcp.server.deleted:{server_id}"),
                "mcp.server.deleted",
                &principal,
                None,
                json!({"server_id":server_id}),
            )
            .await
            .map_err(|_| ApiError::internal())?;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("MCP server not found"))
    }
}
async fn refresh_server(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    let tools = registry(&state)?
        .refresh(&principal, McpServerId(server_id))
        .await
        .map_err(map_mcp)?;
    audit(&state)?
        .emit(
            format!(
                "mcp.tools.discovered:{server_id}:{}",
                Utc::now().timestamp()
            ),
            "mcp.tools.discovered",
            &principal,
            None,
            json!({"server_id":server_id,"tool_count":tools.len()}),
        )
        .await
        .map_err(|_| ApiError::internal())?;
    Ok(Json(json!({"data":tools})))
}
async fn health_server(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    let health = registry(&state)?
        .health(&principal, McpServerId(server_id))
        .await
        .map_err(map_mcp)?;
    audit(&state)?
        .emit(
            format!(
                "mcp.server.health:{server_id}:{}",
                health.checked_at.timestamp_micros()
            ),
            "mcp.server.health_changed",
            &principal,
            None,
            json!({"server_id":server_id,"status":health.status}),
        )
        .await
        .map_err(|_| ApiError::internal())?;
    Ok(Json(json!(health)))
}
async fn admin_server_tools(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(
        json!({"data":registry(&state)?.tools(&principal,Some(McpServerId(server_id))).await.map_err(map_mcp)?}),
    ))
}
async fn admin_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(
        json!({"data":registry(&state)?.tools(&principal,None).await.map_err(map_mcp)?}),
    ))
}
async fn update_tool_annotations(
    State(state): State<AppState>,
    Path((server_id, tool_name)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(annotations): Json<ToolAnnotations>,
) -> Result<Json<Value>, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    if !registry(&state)?
        .update_tool_annotations(
            &principal.tenant_id,
            McpServerId(server_id),
            &tool_name,
            annotations.clone(),
        )
        .await
        .map_err(map_mcp)?
    {
        return Err(ApiError::not_found("MCP tool not found"));
    }
    audit(&state)?
        .emit(
            format!("mcp.tool.annotations:{server_id}:{tool_name}"),
            "mcp.tool.annotations.updated",
            &principal,
            None,
            json!({"server_id":server_id,"tool_name":tool_name}),
        )
        .await
        .map_err(|_| ApiError::internal())?;
    Ok(Json(json!({
        "server_id":server_id,
        "tool_name":tool_name,
        "annotations":annotations
    })))
}
async fn public_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = authenticate(&state, &headers).await?;
    Ok(Json(
        json!({"data":registry(&state)?.safe_servers(&principal).await.map_err(map_mcp)?}),
    ))
}
async fn public_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = authenticate(&state, &headers).await?;
    let policy = state
        .mcp_policies
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("MCP policy unavailable"))?
        .resolved_policy(&principal)
        .await
        .map_err(map_mcp)?;
    let tools = registry(&state)?
        .tools(&principal, None)
        .await
        .map_err(map_mcp)?
        .into_iter()
        .filter(|tool| {
            policy
                .authorize(tool.server_id, &tool.tool_name, &json!({}))
                .is_ok()
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({"data":tools})))
}

#[derive(Deserialize)]
struct ToolCall {
    server_id: McpServerId,
    tool_name: String,
    #[serde(default)]
    arguments: Value,
    approval_id: Option<ApprovalId>,
}
async fn call_tool(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(input): Json<ToolCall>,
) -> Result<Response, ApiError> {
    let principal = authenticate(&state, &headers).await?;
    if input.approval_id.is_some() {
        require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let invocation = GatewayMcpInvocation {
        server_id: input.server_id,
        tool_name: input.tool_name,
        arguments: input.arguments,
        metadata: Default::default(),
    };
    match invocations(&state)?.invoke(request_id.0,principal,invocation,ApprovalReference{approval_id:input.approval_id,idempotency_key:key}).await{Ok(result)=>Ok(Json(result).into_response()),Err(McpError::ApprovalRequired(id))=>Ok((StatusCode::ACCEPTED,Json(json!({"error":{"message":"Human approval is required.","type":"gateway_security_error","param":null,"code":"approval_required"},"approval_id":id}))).into_response()),Err(error)=>Err(map_mcp(error))}
}

#[derive(Deserialize)]
struct PolicyInput {
    name: String,
    policy: McpPolicy,
    scope_kind: String,
    scope_id: String,
}
async fn create_mcp_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PolicyInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    if mcp_policy_requests_approval(&input.policy) {
        require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    }
    validate_scope(&input.scope_kind)?;
    let now = Utc::now();
    let record = McpPolicyRecord {
        policy_id: McpPolicyId::new(),
        tenant_id: principal.tenant_id,
        name: input.name,
        policy: input.policy,
        scope_kind: input.scope_kind,
        scope_id: input.scope_id,
        created_at: now,
        updated_at: now,
    };
    mcp_admin(&state)?
        .insert_mcp_policy(record.clone())
        .await
        .map_err(map_mcp)?;
    Ok((StatusCode::CREATED, Json(json!(record))))
}
async fn mcp_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(
        json!({"data":mcp_admin(&state)?.mcp_policies(&principal.tenant_id).await.map_err(map_mcp)?}),
    ))
}
async fn update_mcp_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<PolicyInput>,
) -> Result<StatusCode, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    if mcp_policy_requests_approval(&input.policy) {
        require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    }
    validate_scope(&input.scope_kind)?;
    let now = Utc::now();
    mcp_admin(&state)?
        .update_mcp_policy(McpPolicyRecord {
            policy_id: McpPolicyId(policy_id),
            tenant_id: principal.tenant_id,
            name: input.name,
            policy: input.policy,
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            created_at: now,
            updated_at: now,
        })
        .await
        .map_err(map_mcp)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn delete_mcp_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    if mcp_admin(&state)?
        .delete_mcp_policy(&principal.tenant_id, McpPolicyId(policy_id))
        .await
        .map_err(map_mcp)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("MCP policy not found"))
    }
}

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    limit: Option<u32>,
}
async fn approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    let status = query.status.as_deref().map(parse_approval).transpose()?;
    Ok(Json(
        json!({"data":approvals_service(&state)?.list(&principal.tenant_id,status,query.limit.unwrap_or(100)).await.map_err(|_|ApiError::internal())?}),
    ))
}
async fn approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    Ok(Json(json!(
        approvals_service(&state)?
            .get(&principal.tenant_id, ApprovalId(id))
            .await
            .map_err(|_| ApiError::not_found("approval not found"))?
    )))
}
#[derive(Deserialize)]
struct DecisionInput {
    reason: Option<String>,
}
async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    input: Option<Json<DecisionInput>>,
) -> Result<Json<Value>, ApiError> {
    decide_approval(
        state,
        id,
        headers,
        input.and_then(|value| value.0.reason),
        true,
    )
    .await
}
async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    input: Option<Json<DecisionInput>>,
) -> Result<Json<Value>, ApiError> {
    decide_approval(
        state,
        id,
        headers,
        input.and_then(|value| value.0.reason),
        false,
    )
    .await
}
async fn decide_approval(
    state: AppState,
    id: Uuid,
    headers: HeaderMap,
    reason: Option<String>,
    allow: bool,
) -> Result<Json<Value>, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    let value = approvals_service(&state)?
        .decide(
            &principal.tenant_id,
            ApprovalId(id),
            &principal,
            allow,
            reason,
        )
        .await
        .map_err(|_| ApiError::forbidden("approval cannot be resolved"))?;
    let event = if allow {
        "approval.approved"
    } else {
        "approval.rejected"
    };
    audit(&state)?
        .emit(
            format!("{event}:{id}"),
            event,
            &principal,
            None,
            json!({"approval_id":id}),
        )
        .await
        .map_err(|_| ApiError::internal())?;
    Ok(Json(json!(value)))
}
async fn public_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = authenticate(&state, &headers).await?;
    require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    let request = approvals_service(&state)?
        .get(&principal.tenant_id, ApprovalId(id))
        .await
        .map_err(|_| ApiError::not_found("approval not found"))?;
    if request.principal_id != principal.principal_id {
        return Err(ApiError::not_found("approval not found"));
    }
    Ok(Json(
        json!({"approval_id":request.approval_id,"status":request.status,"expires_at":request.expires_at}),
    ))
}
async fn public_approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let principal = authenticate(&state, &headers).await?;
    require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    let status = query.status.as_deref().map(parse_approval).transpose()?;
    let data = approvals_service(&state)?
        .list(
            &principal.tenant_id,
            status,
            query.limit.unwrap_or(50).min(100),
        )
        .await
        .map_err(|_| ApiError::internal())?
        .into_iter()
        .filter(|request| request.principal_id == principal.principal_id)
        .collect::<Vec<_>>();
    Ok(Json(json!({"data":data,"next_cursor":null})))
}

#[derive(Deserialize)]
struct SecurityPolicyInput {
    name: String,
    enabled: bool,
    policy: SecurityPolicy,
    scope_kind: String,
    scope_id: String,
}
async fn create_security_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SecurityPolicyInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    require_plan_feature(&state, &principal.tenant_id, "custom_security_policy").await?;
    if input.policy.inspect_llm_output || input.policy.create_incidents {
        require_plan_feature(&state, &principal.tenant_id, "output_inspection").await?;
    }
    if input.policy.inspect_mcp_results {
        require_plan_feature(&state, &principal.tenant_id, "mcp_result_inspection").await?;
    }
    if security_policy_requests_approval(&input.policy) {
        require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    }
    validate_scope(&input.scope_kind)?;
    let now = Utc::now();
    let record = SecurityPolicyRecord {
        policy_id: SecurityPolicyId::new(),
        tenant_id: principal.tenant_id,
        name: input.name,
        enabled: input.enabled,
        policy: input.policy,
        scope_kind: input.scope_kind,
        scope_id: input.scope_id,
        created_at: now,
        updated_at: now,
    };
    security_repo(&state)?
        .insert_security_policy(record.clone(), &record.scope_kind, &record.scope_id)
        .await
        .map_err(|_| ApiError::internal())?;
    Ok((StatusCode::CREATED, Json(json!(record))))
}
async fn security_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(
        json!({"data":security_repo(&state)?.security_policies(&principal.tenant_id).await.map_err(|_|ApiError::internal())?}),
    ))
}
async fn update_security_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<SecurityPolicyInput>,
) -> Result<StatusCode, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    require_plan_feature(&state, &principal.tenant_id, "custom_security_policy").await?;
    if input.policy.inspect_llm_output || input.policy.create_incidents {
        require_plan_feature(&state, &principal.tenant_id, "output_inspection").await?;
    }
    if input.policy.inspect_mcp_results {
        require_plan_feature(&state, &principal.tenant_id, "mcp_result_inspection").await?;
    }
    if security_policy_requests_approval(&input.policy) {
        require_plan_feature(&state, &principal.tenant_id, "human_approval").await?;
    }
    validate_scope(&input.scope_kind)?;
    let now = Utc::now();
    let record = SecurityPolicyRecord {
        policy_id: SecurityPolicyId(id),
        tenant_id: principal.tenant_id,
        name: input.name,
        enabled: input.enabled,
        policy: input.policy,
        scope_kind: input.scope_kind,
        scope_id: input.scope_id,
        created_at: now,
        updated_at: now,
    };
    security_repo(&state)?
        .update_security_policy(record.clone(), &record.scope_kind, &record.scope_id)
        .await
        .map_err(|_| ApiError::internal())?;
    Ok(StatusCode::NO_CONTENT)
}
async fn delete_security_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    if security_repo(&state)?
        .delete_security_policy(&principal.tenant_id, SecurityPolicyId(id))
        .await
        .map_err(|_| ApiError::internal())?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("security policy not found"))
    }
}
async fn incidents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let status = query.status.as_deref().map(parse_incident).transpose()?;
    Ok(Json(
        json!({"data":incidents_service(&state)?.list(&principal.tenant_id,status,query.limit.unwrap_or(100)).await.map_err(|_|ApiError::internal())?}),
    ))
}
async fn incident(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(json!(
        incidents_service(&state)?
            .get(&principal.tenant_id, IncidentId(id))
            .await
            .map_err(|_| ApiError::not_found("incident not found"))?
    )))
}
#[derive(Deserialize)]
struct IncidentInput {
    status: String,
    note: Option<String>,
}
async fn update_incident(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<IncidentInput>,
) -> Result<Json<Value>, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    let status = parse_incident(&input.status)?;
    Ok(Json(json!(
        incidents_service(&state)?
            .update(
                &principal.tenant_id,
                IncidentId(id),
                status,
                principal.principal_id,
                input.note
            )
            .await
            .map_err(|_| ApiError::internal())?
    )))
}
async fn findings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(
        json!({"data":security_repo(&state)?.findings(&principal.tenant_id,query.limit.unwrap_or(100)).await.map_err(|_|ApiError::internal())?}),
    ))
}
async fn security_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    Ok(Json(
        json!({"data":security_repo(&state)?.security_events(&principal.tenant_id,query.limit.unwrap_or(100)).await.map_err(|_|ApiError::internal())?}),
    ))
}

#[derive(Deserialize)]
struct SecurityPatternInput {
    name: String,
    category: SecurityCategory,
    pattern: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

async fn security_patterns(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let data = security_repo(&state)?
        .custom_patterns(&principal.tenant_id)
        .await
        .map_err(|_| ApiError::internal())?;
    Ok(Json(json!({"data":data,"next_cursor":null})))
}

async fn create_security_pattern(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SecurityPatternInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    require_plan_capacity(&state, &principal.tenant_id, "security_patterns").await?;
    validate_custom_pattern(&input.name, &input.pattern)
        .map_err(|_| ApiError::invalid("invalid custom security pattern"))?;
    let now = Utc::now();
    let record = SecurityCustomPattern {
        pattern_id: Uuid::now_v7(),
        tenant_id: principal.tenant_id,
        name: input.name,
        category: input.category,
        pattern: input.pattern,
        enabled: input.enabled,
        version: 1,
        created_at: now,
        updated_at: now,
    };
    security_repo(&state)?
        .insert_custom_pattern(record.clone())
        .await
        .map_err(|_| ApiError::conflict("custom pattern already exists"))?;
    Ok((StatusCode::CREATED, Json(json!(record))))
}

async fn update_security_pattern(
    State(state): State<AppState>,
    Path(pattern_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<SecurityPatternInput>,
) -> Result<Json<Value>, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    validate_custom_pattern(&input.name, &input.pattern)
        .map_err(|_| ApiError::invalid("invalid custom security pattern"))?;
    let version = if_match(&headers)?;
    let now = Utc::now();
    let record = SecurityCustomPattern {
        pattern_id,
        tenant_id: principal.tenant_id,
        name: input.name,
        category: input.category,
        pattern: input.pattern,
        enabled: input.enabled,
        version: version + 1,
        created_at: now,
        updated_at: now,
    };
    if !security_repo(&state)?
        .update_custom_pattern(record.clone(), version)
        .await
        .map_err(|_| ApiError::internal())?
    {
        return Err(ApiError::conflict("custom pattern version conflict"));
    }
    Ok(Json(json!(record)))
}

async fn delete_security_pattern(
    State(state): State<AppState>,
    Path(pattern_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    if security_repo(&state)?
        .delete_custom_pattern(&principal.tenant_id, pattern_id, if_match(&headers)?)
        .await
        .map_err(|_| ApiError::internal())?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::conflict("custom pattern version conflict"))
    }
}

async fn incident_timeline(
    State(state): State<AppState>,
    Path(incident_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = admin_principal(&state, &headers).await?;
    let data = incidents_service(&state)?
        .timeline(&principal.tenant_id, IncidentId(incident_id))
        .await
        .map_err(|_| ApiError::not_found("incident not found"))?;
    Ok(Json(json!({"data":data,"next_cursor":null})))
}

fn if_match(headers: &HeaderMap) -> Result<u64, ApiError> {
    headers
        .get(axum::http::header::IF_MATCH)
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

#[derive(Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}
async fn mcp_post(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(request): Json<RpcRequest>,
) -> Result<Json<Value>, ApiError> {
    validate_origin(&headers)?;
    let principal = authenticate(&state, &headers).await?;
    if request.jsonrpc != "2.0" {
        return Err(ApiError::invalid("invalid JSON-RPC version"));
    }
    let result = match request.method.as_str() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"tuenel-gateway","version":"0.3"}})
        }
        "ping" => json!({}),
        "notifications/initialized" => return Ok(Json(json!({}))),
        "tools/list" => {
            let policy = state
                .mcp_policies
                .as_ref()
                .ok_or_else(|| ApiError::service_unavailable("MCP policy unavailable"))?
                .resolved_policy(&principal)
                .await
                .map_err(map_mcp)?;
            let tools = registry(&state)?
                .tools(&principal, None)
                .await
                .map_err(map_mcp)?
                .into_iter()
                .filter(|tool| {
                    policy
                        .authorize(tool.server_id, &tool.tool_name, &json!({}))
                        .is_ok()
                });
            json!({"tools":tools.map(|tool|json!({"name":format!("{}__{}",tool.server_id,tool.tool_name),"description":tool.description,"inputSchema":tool.input_schema,"annotations":tool.annotations})).collect::<Vec<_>>()})
        }
        "tools/call" => {
            let name = request
                .params
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::invalid("missing tool name"))?;
            let (server, tool) = name
                .split_once("__")
                .ok_or_else(|| ApiError::invalid("invalid tool name"))?;
            let server_id =
                Uuid::parse_str(server).map_err(|_| ApiError::invalid("invalid server ID"))?;
            let invocation = GatewayMcpInvocation {
                server_id: McpServerId(server_id),
                tool_name: tool.into(),
                arguments: request
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
                metadata: Default::default(),
            };
            match invocations(&state)?
                .invoke(
                    request_id.0,
                    principal,
                    invocation,
                    ApprovalReference::default(),
                )
                .await
            {
                Ok(value) => serde_json::to_value(value).map_err(|_| ApiError::internal())?,
                Err(McpError::ApprovalRequired(id)) => {
                    json!({"content":[{"type":"text","text":"Human approval is required."}],"structuredContent":{"code":"approval_required","approvalId":id},"isError":true})
                }
                Err(error) => {
                    json!({"content":[{"type":"text","text":error.to_string()}],"isError":true})
                }
            }
        }
        _ => {
            return Ok(Json(
                json!({"jsonrpc":"2.0","id":request.id,"error":{"code":-32601,"message":"Method not found"}}),
            ));
        }
    };
    Ok(Json(
        json!({"jsonrpc":"2.0","id":request.id,"result":result}),
    ))
}
async fn mcp_get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    validate_origin(&headers)?;
    authenticate(&state, &headers).await?;
    Ok(StatusCode::METHOD_NOT_ALLOWED)
}
async fn mcp_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    validate_origin(&headers)?;
    authenticate(&state, &headers).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn registry(state: &AppState) -> Result<&gateway_mcp::McpRegistry, ApiError> {
    state
        .mcp_registry
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("MCP is disabled"))
}
fn invocations(state: &AppState) -> Result<&gateway_mcp::McpInvocationService, ApiError> {
    state
        .mcp_invocations
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("MCP is disabled"))
}
fn mcp_admin(
    state: &AppState,
) -> Result<&std::sync::Arc<dyn gateway_mcp::McpPolicyAdministration>, ApiError> {
    state
        .mcp_policy_admin
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("MCP policy unavailable"))
}
fn approvals_service(state: &AppState) -> Result<&gateway_approval::ApprovalService, ApiError> {
    state
        .approvals
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("approval is disabled"))
}
fn incidents_service(state: &AppState) -> Result<&gateway_incidents::IncidentService, ApiError> {
    state
        .incidents
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("incidents unavailable"))
}
fn security_repo(
    state: &AppState,
) -> Result<&std::sync::Arc<dyn gateway_security::SecurityRepository>, ApiError> {
    state
        .security_repository
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("security persistence unavailable"))
}
fn audit(state: &AppState) -> Result<&gateway_events::AuditService, ApiError> {
    state
        .audit
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("audit persistence unavailable"))
}
fn map_mcp(error: McpError) -> ApiError {
    match error {
        McpError::ServerNotFound => ApiError::new(
            StatusCode::NOT_FOUND,
            "gateway_mcp_error",
            "mcp_tool_unavailable",
            "MCP server is unavailable.",
        ),
        McpError::ServerNotAllowed => ApiError::new(
            StatusCode::FORBIDDEN,
            "gateway_mcp_error",
            "mcp_server_not_allowed",
            "MCP server is not allowed.",
        ),
        McpError::ToolNotAllowed => ApiError::new(
            StatusCode::FORBIDDEN,
            "gateway_mcp_error",
            "mcp_tool_not_allowed",
            "MCP tool is not allowed.",
        ),
        McpError::ToolUnavailable => ApiError::new(
            StatusCode::NOT_FOUND,
            "gateway_mcp_error",
            "mcp_tool_unavailable",
            "MCP tool is unavailable.",
        ),
        McpError::QuotaExceeded => ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "gateway_mcp_error",
            "mcp_quota_exceeded",
            "MCP quota exceeded.",
        ),
        McpError::Timeout => ApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "gateway_mcp_error",
            "mcp_invocation_failed",
            "MCP invocation timed out.",
        ),
        McpError::ApprovalRequired(_) => ApiError::new(
            StatusCode::ACCEPTED,
            "gateway_security_error",
            "approval_required",
            "Human approval is required.",
        ),
        McpError::ApprovalRejected => ApiError::new(
            StatusCode::FORBIDDEN,
            "gateway_security_error",
            "approval_rejected",
            "Approval was rejected.",
        ),
        McpError::ApprovalExpired => ApiError::new(
            StatusCode::FORBIDDEN,
            "gateway_security_error",
            "approval_expired",
            "Approval expired.",
        ),
        McpError::Invalid | McpError::TooLarge => ApiError::invalid("invalid MCP request"),
        McpError::Transport | McpError::Unavailable => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway_mcp_error",
            "mcp_invocation_failed",
            "MCP invocation failed.",
        ),
    }
}
fn validate_server_input(input: &CreateServer) -> Result<(), ApiError> {
    if input.name.trim().is_empty() || input.name.len() > 255 {
        return Err(ApiError::invalid("invalid server name"));
    }
    match input.transport_type {
        McpTransportType::Stdio if input.command.is_none() || input.endpoint.is_some() => {
            Err(ApiError::invalid("stdio requires command only"))
        }
        McpTransportType::StreamableHttp
            if input.endpoint.as_deref().is_none_or(|value| {
                !value.starts_with("http://") && !value.starts_with("https://")
            }) || input.command.is_some() =>
        {
            Err(ApiError::invalid(
                "streamable HTTP requires an HTTP endpoint only",
            ))
        }
        _ => Ok(()),
    }
}
fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        && !value.as_bytes()[0].is_ascii_digit()
}
fn validate_origin(headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) else {
        return Ok(());
    };
    let host = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::forbidden("invalid MCP origin"))?;
    let authority = origin
        .parse::<http::Uri>()
        .ok()
        .and_then(|uri| uri.authority().map(ToString::to_string))
        .ok_or_else(|| ApiError::forbidden("invalid MCP origin"))?;
    if authority.eq_ignore_ascii_case(host) {
        Ok(())
    } else {
        Err(ApiError::forbidden("invalid MCP origin"))
    }
}
fn validate_scope(value: &str) -> Result<(), ApiError> {
    if matches!(
        value,
        "global" | "tenant" | "project" | "principal" | "virtual_key"
    ) {
        Ok(())
    } else {
        Err(ApiError::invalid("invalid policy scope"))
    }
}
fn security_policy_requests_approval(policy: &SecurityPolicy) -> bool {
    policy.actions.values().any(|actions| {
        actions
            .values()
            .any(|action| matches!(action, SecurityAction::RequireApproval))
    })
}
fn mcp_policy_requests_approval(policy: &McpPolicy) -> bool {
    matches!(
        policy.default_mutating_action,
        Some(SecurityAction::RequireApproval)
    ) || policy
        .tool_actions
        .values()
        .any(|action| matches!(action, SecurityAction::RequireApproval))
}
fn parse_approval(value: &str) -> Result<ApprovalStatus, ApiError> {
    match value {
        "pending" => Ok(ApprovalStatus::Pending),
        "approved" => Ok(ApprovalStatus::Approved),
        "rejected" => Ok(ApprovalStatus::Rejected),
        "expired" => Ok(ApprovalStatus::Expired),
        "cancelled" => Ok(ApprovalStatus::Cancelled),
        _ => Err(ApiError::invalid("invalid approval status")),
    }
}
fn parse_incident(value: &str) -> Result<IncidentStatus, ApiError> {
    match value {
        "open" => Ok(IncidentStatus::Open),
        "acknowledged" => Ok(IncidentStatus::Acknowledged),
        "resolved" => Ok(IncidentStatus::Resolved),
        "ignored" => Ok(IncidentStatus::Ignored),
        _ => Err(ApiError::invalid("invalid incident status")),
    }
}
fn default_true() -> bool {
    true
}
