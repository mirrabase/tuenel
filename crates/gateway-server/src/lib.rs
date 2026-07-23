//! Axum transport for the OpenAI-compatible v0.1 API.

mod admin;
mod v03;

use std::{convert::Infallible, sync::Arc, time::Instant};

use async_stream::stream;
use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{delete, get, post},
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use gateway_auth::{AuthError, Authenticator, LoginResult, Signup, WebAuthError, WebAuthService};
use gateway_core::{GatewayError, GatewayRuntime};
use gateway_keys::VirtualKeyService;
use gateway_quota::QuotaError;
use gateway_store::GatewayStore;
use gateway_types::{
    AuthenticationMethod, GatewayEmbeddingRequest, GatewayInferenceRequest, GatewayInstruction,
    GatewayMessage, GatewayRequest, GatewayStreamEvent, GenerationParameters, InstructionRole,
    MessageRole, NewVirtualKey, Principal, RequestMetadata, TokenUsage,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_http::limit::RequestBodyLimitLayer;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";

/// Shared HTTP dependencies.
#[derive(Clone)]
pub struct AppState {
    /// Authentication chain.
    pub authenticator: Arc<dyn Authenticator>,
    /// Gateway-owned browser identity and session service.
    pub web_auth: Option<WebAuthService>,
    /// Inference application service.
    pub gateway: GatewayRuntime,
    /// Virtual Key lifecycle service.
    pub keys: VirtualKeyService,
    /// Store used for readiness checks.
    pub store: Arc<dyn GatewayStore>,
    /// JWT role required by admin routes.
    pub admin_role: String,
    /// Maximum accepted output tokens.
    pub max_output_tokens: u32,
    /// Default output tokens when omitted.
    pub default_output_tokens: u32,
    /// Default daily limit for new Virtual Keys.
    pub default_virtual_key_daily_tokens: u64,
    /// Maximum JSON body size.
    pub max_body_bytes: usize,
    /// Optional v0.3 MCP registry.
    pub mcp_registry: Option<gateway_mcp::McpRegistry>,
    /// Optional v0.3 MCP invocation pipeline.
    pub mcp_invocations: Option<gateway_mcp::McpInvocationService>,
    /// MCP policy persistence and resolution.
    pub mcp_policies: Option<Arc<dyn gateway_mcp::McpPolicyRepository>>,
    /// MCP policy administration persistence.
    pub mcp_policy_admin: Option<Arc<dyn gateway_mcp::McpPolicyAdministration>>,
    /// Encrypted secret service.
    pub secrets: Option<gateway_secrets::SecretService>,
    /// Approval service.
    pub approvals: Option<gateway_approval::ApprovalService>,
    /// Incident service.
    pub incidents: Option<gateway_incidents::IncidentService>,
    /// Security policy, finding, and event persistence.
    pub security_repository: Option<Arc<dyn gateway_security::SecurityRepository>>,
    /// Append-only audit event service.
    pub audit: Option<gateway_events::AuditService>,
    /// PostgreSQL-backed control-plane administration.
    pub admin: Option<gateway_admin::AdminService>,
}

/// Build the complete HTTP router.
pub fn router(state: AppState) -> Router {
    let body_limit = state.max_body_bytes;
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/openapi.json", get(openapi))
        .route("/metrics", get(metrics))
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/session", get(session).delete(logout))
        .route(
            "/auth/tenants/{tenant_id}/invitations",
            post(create_invitation),
        )
        .route("/auth/invitations/accept", post(accept_invitation))
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .route("/v1/embeddings", post(embeddings))
        .route("/admin/virtual-keys", post(create_virtual_key))
        .route("/admin/virtual-keys/{id}", delete(revoke_virtual_key))
        .merge(admin::routes())
        .merge(v03::routes())
        .layer(RequestBodyLimitLayer::new(body_limit))
        .layer(middleware::from_fn(request_context))
        .with_state(state)
}

/// Return the generated OpenAPI document.
pub fn openapi_document() -> utoipa::openapi::OpenApi {
    let document = ApiDoc::openapi();
    let mut value = serde_json::to_value(&document).expect("OpenAPI document serializes");
    let paths = value
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .expect("OpenAPI paths object");
    for (path, methods) in v03_openapi_paths() {
        let operations = methods.iter().map(|method| ((*method).to_owned(), json!({"responses":{"200":{"description":"Success"},"401":{"description":"Authentication required"},"403":{"description":"Not authorized"}}}))).collect();
        paths.insert(path.to_owned(), Value::Object(operations));
    }
    serde_json::from_value(value).expect("generated OpenAPI is valid")
}

fn v03_openapi_paths() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        ("/admin/mcp/servers", &["get", "post"]),
        (
            "/admin/mcp/servers/{server_id}",
            &["get", "patch", "delete"],
        ),
        ("/admin/mcp/servers/{server_id}/refresh", &["post"]),
        ("/admin/mcp/servers/{server_id}/health", &["post"]),
        ("/admin/mcp/servers/{server_id}/tools", &["get"]),
        ("/admin/mcp/tools", &["get"]),
        (
            "/admin/mcp/tools/{server_id}/{tool_name}/annotations",
            &["patch"],
        ),
        ("/admin/mcp/policies", &["get", "post"]),
        ("/admin/mcp/policies/{policy_id}", &["patch", "delete"]),
        ("/v1/mcp/servers", &["get"]),
        ("/v1/mcp/tools", &["get"]),
        ("/v1/mcp/tools/call", &["post"]),
        ("/mcp", &["get", "post", "delete"]),
        ("/admin/approvals", &["get"]),
        ("/admin/approvals/{approval_id}", &["get"]),
        ("/admin/approvals/{approval_id}/approve", &["post"]),
        ("/admin/approvals/{approval_id}/reject", &["post"]),
        ("/v1/gateway/approvals/{approval_id}", &["get"]),
        ("/v1/gateway/approvals", &["get"]),
        ("/admin/security/policies", &["get", "post"]),
        ("/admin/security/policies/{policy_id}", &["patch", "delete"]),
        ("/admin/security/incidents", &["get"]),
        ("/admin/security/incidents/{incident_id}", &["get", "patch"]),
        ("/admin/security/findings", &["get"]),
        ("/admin/security/events", &["get"]),
        ("/admin/security/patterns", &["get", "post"]),
        (
            "/admin/security/patterns/{pattern_id}",
            &["patch", "delete"],
        ),
        ("/admin/security/incidents/{incident_id}/timeline", &["get"]),
        ("/admin/tenants", &["get"]),
        ("/admin/projects", &["get", "post"]),
        ("/admin/projects/{id}", &["patch", "delete"]),
        ("/auth/tenants/{tenant_id}/members", &["get"]),
        ("/admin/virtual-keys", &["get", "post"]),
        ("/admin/providers", &["get", "post"]),
        ("/admin/providers/{id}", &["patch", "delete"]),
        ("/admin/providers/{id}/check", &["post"]),
        ("/admin/model-routes", &["get", "post"]),
        ("/admin/model-routes/{id}", &["patch", "delete"]),
        ("/admin/model-prices", &["get", "post"]),
        ("/admin/model-prices/{id}", &["patch", "delete"]),
        ("/admin/policies", &["get", "post"]),
        ("/admin/policies/{id}", &["patch", "delete"]),
        ("/admin/quota-limits", &["get", "post"]),
        ("/admin/quota-limits/{id}", &["patch", "delete"]),
        ("/admin/usage/summary", &["get"]),
        ("/admin/usage/series", &["get"]),
        ("/admin/usage/events", &["get"]),
        ("/admin/usage/reservations", &["get"]),
        ("/admin/usage/mcp-invocations", &["get"]),
        ("/admin/audit-events", &["get"]),
        ("/admin/summary", &["get"]),
        ("/admin/system", &["get"]),
        ("/admin/billing/webhooks", &["get"]),
        ("/admin/billing/outbox", &["get"]),
        ("/admin/billing/outbox/{event_id}/retry", &["post"]),
    ]
}

#[derive(OpenApi)]
#[openapi(
    paths(health, ready, models, chat_completions, responses, embeddings, create_virtual_key, revoke_virtual_key),
    components(schemas(
        ChatCompletionRequest, ChatMessage, StreamOptions, ChatCompletionResponse,
        CompletionChoice, ResponseMessage, UsageResponse, ModelList, ModelObject,
        ResponsesRequest, ResponsesInput, ResponsesResponse, ResponsesOutput, ResponsesOutputText,
        EmbeddingsRequest, EmbeddingsInput, EmbeddingsResponse, EmbeddingData,
        CreateVirtualKeyRequest, CreateVirtualKeyResponse, ErrorEnvelope, ErrorObject
    )),
    tags((name = "gateway", description = "Tuenel Gateway v0.3"))
)]
struct ApiDoc;

#[utoipa::path(get, path = "/health", responses((status = 200, body = StatusResponse)))]
async fn health() -> Json<StatusResponse> {
    Json(StatusResponse { status: "ok" })
}

#[utoipa::path(get, path = "/ready", responses((status = 200, body = StatusResponse), (status = 503, body = ErrorEnvelope)))]
async fn ready(State(state): State<AppState>) -> Result<Json<StatusResponse>, ApiError> {
    state
        .store
        .ping()
        .await
        .map_err(|_| ApiError::service_unavailable("gateway is not ready"))?;
    Ok(Json(StatusResponse { status: "ready" }))
}

async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi_document())
}

async fn metrics() -> Result<([(axum::http::HeaderName, &'static str); 1], String), ApiError> {
    gateway_observability::prometheus_text()
        .map(|body| {
            (
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/plain; version=0.0.4",
                )],
                body,
            )
        })
        .map_err(|_| ApiError::internal())
}

#[derive(Deserialize)]
struct SignupRequest {
    email: String,
    password: String,
    tenant_name: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct InvitationRequest {
    email: String,
    role: gateway_auth::TenantRole,
}

#[derive(Deserialize)]
struct AcceptInvitationRequest {
    token: String,
}

async fn signup(
    State(state): State<AppState>,
    Json(input): Json<SignupRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let result = web_auth(&state)?
        .signup(Signup {
            email: input.email,
            password: input.password,
            tenant_name: input.tenant_name,
        })
        .await
        .map_err(map_web_auth)?;
    Ok((StatusCode::CREATED, Json(login_json(result))))
}

async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginRequest>,
) -> Result<Json<Value>, ApiError> {
    web_auth(&state)?
        .login(&input.email, &input.password)
        .await
        .map(login_json)
        .map(Json)
        .map_err(map_web_auth)
}

async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let session = web_auth(&state)?
        .session(bearer_credential(&headers)?)
        .await
        .map_err(map_web_auth)?;
    Ok(Json(json!({
        "user_id": session.user_id,
        "email": session.email,
        "gateway_admin": session.gateway_admin,
        "expires_at": session.expires_at,
        "memberships": session.memberships,
    })))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    web_auth(&state)?
        .logout(bearer_credential(&headers)?)
        .await
        .map_err(map_web_auth)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_invitation(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<InvitationRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let credential = bearer_credential(&headers)?;
    if !credential.ends_with(&format!(".{tenant_id}")) {
        return Err(ApiError::forbidden("tenant context mismatch"));
    }
    let invitation = web_auth(&state)?
        .invite(credential, &input.email, input.role)
        .await
        .map_err(map_web_auth)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": invitation.id,
            "token": invitation.token.expose(),
            "expires_at": invitation.expires_at,
        })),
    ))
}

async fn accept_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AcceptInvitationRequest>,
) -> Result<Json<Value>, ApiError> {
    web_auth(&state)?
        .accept_invitation(bearer_credential(&headers)?, &input.token)
        .await
        .map(|membership| Json(json!({ "membership": membership })))
        .map_err(map_web_auth)
}

fn login_json(result: LoginResult) -> Value {
    json!({
        "user_id": result.user_id,
        "credential": result.credential.expose(),
        "expires_at": result.expires_at,
        "memberships": result.memberships,
    })
}

fn web_auth(state: &AppState) -> Result<&WebAuthService, ApiError> {
    state
        .web_auth
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("web authentication unavailable"))
}

fn map_web_auth(error: WebAuthError) -> ApiError {
    match error {
        WebAuthError::Invalid => ApiError::invalid("invalid authentication input"),
        WebAuthError::InvalidCredentials => ApiError::unauthorized(),
        WebAuthError::Conflict => ApiError::conflict("identity already exists"),
        WebAuthError::Forbidden => ApiError::forbidden("operation is not permitted"),
        WebAuthError::Hashing | WebAuthError::Unavailable => {
            ApiError::service_unavailable("authentication unavailable")
        }
    }
}

#[utoipa::path(get, path = "/v1/models", responses((status = 200, body = ModelList), (status = 401, body = ErrorEnvelope)))]
async fn models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ModelList>, ApiError> {
    authenticate(&state, &headers).await?;
    let route = state.gateway.model_route();
    Ok(Json(ModelList {
        object: "list",
        data: vec![ModelObject {
            id: route.requested_model,
            object: "model",
            created: 0,
            owned_by: "tuenel",
        }],
    }))
}

#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    request_body = ChatCompletionRequest,
    responses((status = 200, body = ChatCompletionResponse), (status = 400, body = ErrorEnvelope), (status = 401, body = ErrorEnvelope), (status = 429, body = ErrorEnvelope))
)]
async fn chat_completions(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(input): Json<ChatCompletionRequest>,
) -> Result<Response, ApiError> {
    let principal = inference_principal(&state, &headers).await?;
    let request = input.into_gateway(state.default_output_tokens, state.max_output_tokens)?;
    if request.stream {
        let model = request.model.clone();
        let stream = state
            .gateway
            .stream(request_id.0, principal, request)
            .await
            .map_err(ApiError::from)?;
        Ok(streaming_response(request_id.0, model, stream))
    } else {
        let response = state
            .gateway
            .execute(request_id.0, principal, request)
            .await
            .map_err(ApiError::from)?;
        Ok(Json(ChatCompletionResponse::from_gateway(response)).into_response())
    }
}

#[utoipa::path(post,path="/v1/responses",request_body=ResponsesRequest,responses((status=200,body=ResponsesResponse),(status=401,body=ErrorEnvelope),(status=403,body=ErrorEnvelope)))]
async fn responses(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(input): Json<ResponsesRequest>,
) -> Result<Response, ApiError> {
    let principal = inference_principal(&state, &headers).await?;
    let stream = input.stream;
    let request = input.into_gateway(state.default_output_tokens, state.max_output_tokens)?;
    if stream {
        let source = state
            .gateway
            .stream_inference(request_id.0, principal, request)
            .await
            .map_err(ApiError::from)?;
        return Ok(responses_streaming_response(request_id.0, source));
    }
    let response = state
        .gateway
        .execute_inference(request_id.0, principal, request)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ResponsesResponse::from_gateway(response)).into_response())
}

fn responses_streaming_response(
    request_id: Uuid,
    mut source: gateway_core::GatewayResultStream,
) -> Response {
    let output = stream! {
        while let Some(item) = source.next().await {
            let data = match item {
                Ok(GatewayStreamEvent::Started { id }) => json!({"type":"response.created","response":{"id":id,"object":"response"}}).to_string(),
                Ok(GatewayStreamEvent::Delta { content }) => json!({"type":"response.output_text.delta","item_id":format!("item-{request_id}"),"delta":content}).to_string(),
                Ok(GatewayStreamEvent::Finished { .. }) => json!({"type":"response.completed","response":{"id":format!("resp-{request_id}")}}).to_string(),
                Ok(GatewayStreamEvent::Usage(usage)) => json!({"type":"response.usage","usage":usage}).to_string(),
                Err(error) => serde_json::to_string(&ApiError::from(error).body).unwrap_or_else(|_| "{\"error\":{\"code\":\"internal_error\"}}".into()),
            };
            yield Ok::<Event, Infallible>(Event::default().data(data));
        }
    };
    Sse::new(output).into_response()
}

#[utoipa::path(post,path="/v1/embeddings",request_body=EmbeddingsRequest,responses((status=200,body=EmbeddingsResponse),(status=401,body=ErrorEnvelope),(status=403,body=ErrorEnvelope)))]
async fn embeddings(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    headers: HeaderMap,
    Json(input): Json<EmbeddingsRequest>,
) -> Result<Json<EmbeddingsResponse>, ApiError> {
    let principal = inference_principal(&state, &headers).await?;
    let request = input.into_gateway(state.max_body_bytes)?;
    let response = state
        .gateway
        .embed(request_id.0, principal, request)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(EmbeddingsResponse::from_gateway(response)))
}

#[utoipa::path(
    post,
    path = "/admin/virtual-keys",
    request_body = CreateVirtualKeyRequest,
    responses((status = 201, body = CreateVirtualKeyResponse), (status = 401, body = ErrorEnvelope), (status = 403, body = ErrorEnvelope))
)]
async fn create_virtual_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateVirtualKeyRequest>,
) -> Result<(StatusCode, Json<CreateVirtualKeyResponse>), ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    let daily_token_limit = input
        .daily_token_limit
        .unwrap_or(state.default_virtual_key_daily_tokens);
    if daily_token_limit == 0 {
        return Err(ApiError::invalid(
            "daily_token_limit must be greater than zero",
        ));
    }
    if input.expires_at.is_some_and(|time| time <= Utc::now()) {
        return Err(ApiError::invalid("expires_at must be in the future"));
    }
    let issued = state
        .keys
        .issue(NewVirtualKey {
            tenant_id: principal.tenant_id,
            project_id: input.project_id,
            user_id: input.user_id,
            scopes: input.scopes,
            expires_at: input.expires_at,
            daily_token_limit,
        })
        .await
        .map_err(|_| ApiError::internal())?;
    Ok((
        StatusCode::CREATED,
        Json(CreateVirtualKeyResponse {
            id: issued.record.id,
            key: issued.plaintext.expose().to_owned(),
            key_prefix: issued.record.lookup_prefix,
            expires_at: issued.record.expires_at,
            daily_token_limit: issued.record.daily_token_limit,
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/admin/virtual-keys/{id}",
    params(("id" = Uuid, Path)),
    responses((status = 204), (status = 401, body = ErrorEnvelope), (status = 404, body = ErrorEnvelope))
)]
async fn revoke_virtual_key(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let principal = write_admin_principal(&state, &headers).await?;
    if state
        .keys
        .revoke(&principal.tenant_id, id)
        .await
        .map_err(|_| ApiError::internal())?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("virtual key not found"))
    }
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let credential = bearer_credential(headers)?;
    state
        .authenticator
        .authenticate(credential)
        .await
        .map_err(|error| match error {
            AuthError::UnknownTenant => ApiError::forbidden("tenant is not provisioned"),
            AuthError::Unavailable => ApiError::service_unavailable("authentication unavailable"),
            AuthError::Invalid => ApiError::unauthorized(),
        })
}

fn bearer_credential(headers: &HeaderMap) -> Result<&str, ApiError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(ApiError::unauthorized)?;
    header
        .strip_prefix("Bearer ")
        .filter(|credential| !credential.is_empty())
        .ok_or_else(ApiError::unauthorized)
}

async fn admin_principal(state: &AppState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let principal = authenticate(state, headers).await?;
    let allowed = principal.roles.iter().any(|role| {
        role == &state.admin_role || matches!(role.as_str(), "owner" | "admin" | "engineer")
    });
    if principal.authentication_method == AuthenticationMethod::VirtualKey || !allowed {
        return Err(ApiError::forbidden("administrator role required"));
    }
    Ok(principal)
}

async fn write_admin_principal(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Principal, ApiError> {
    let principal = admin_principal(state, headers).await?;
    principal
        .roles
        .iter()
        .any(|role| role == &state.admin_role || matches!(role.as_str(), "owner" | "admin"))
        .then_some(principal)
        .ok_or_else(|| ApiError::forbidden("administrator write role required"))
}

async fn inference_principal(state: &AppState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let principal = authenticate(state, headers).await?;
    if principal.authentication_method == AuthenticationMethod::WebSession
        && principal.roles.iter().any(|role| role == "viewer")
    {
        return Err(ApiError::forbidden("viewer role is read-only"));
    }
    Ok(principal)
}

fn streaming_response(
    request_id: Uuid,
    model: String,
    mut source: gateway_core::GatewayResultStream,
) -> Response {
    let output = stream! {
        let mut id = format!("chatcmpl-{request_id}");
        while let Some(item) = source.next().await {
            let data = match item {
                Ok(GatewayStreamEvent::Started { id: provider_id }) => {
                    id = provider_id;
                    stream_chunk(&id, &model, json!({"role":"assistant","content":""}), None, None)
                }
                Ok(GatewayStreamEvent::Delta { content }) => {
                    stream_chunk(&id, &model, json!({"content":content}), None, None)
                }
                Ok(GatewayStreamEvent::Finished { reason }) => {
                    stream_chunk(&id, &model, json!({}), reason, None)
                }
                Ok(GatewayStreamEvent::Usage(usage)) => {
                    stream_chunk(&id, &model, json!({}), None, Some(UsageResponse::from(usage)))
                }
                Err(error) => serde_json::to_string(&ApiError::from(error).body)
                    .unwrap_or_else(|_| "{\"error\":{\"message\":\"internal error\",\"type\":\"server_error\",\"code\":\"internal_error\",\"param\":null}}".into()),
            };
            yield Ok::<Event, Infallible>(Event::default().data(data));
        }
        yield Ok(Event::default().data("[DONE]"));
    };
    Sse::new(output).into_response()
}

fn stream_chunk(
    id: &str,
    model: &str,
    delta: Value,
    finish_reason: Option<String>,
    usage: Option<UsageResponse>,
) -> String {
    let choices = if usage.is_some() {
        Vec::<Value>::new()
    } else {
        vec![json!({"index":0,"delta":delta,"finish_reason":finish_reason})]
    };
    json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": Utc::now().timestamp(),
        "model": model,
        "choices": choices,
        "usage": usage,
    })
    .to_string()
}

async fn request_context(mut request: Request<Body>, next: Next) -> Response {
    let request_id = Uuid::now_v7();
    request.extensions_mut().insert(RequestId(request_id));
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let started = Instant::now();
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    tracing::info!(
        request_id = %request_id,
        method = %method,
        path,
        status = response.status().as_u16(),
        latency_ms = started.elapsed().as_millis() as u64,
        "request completed"
    );
    response
}

#[derive(Clone, Copy, Debug)]
struct RequestId(Uuid);

/// OpenAI Responses API request. Provider-specific fields are normalized before core execution.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: ResponsesInput,
    pub instructions: Option<String>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum ResponsesInput {
    Text(String),
    Messages(Vec<ChatMessage>),
}

impl ResponsesRequest {
    fn into_gateway(
        self,
        default_tokens: u32,
        max_tokens: u32,
    ) -> Result<GatewayInferenceRequest, ApiError> {
        let messages = match self.input {
            ResponsesInput::Text(content) if !content.is_empty() => vec![GatewayMessage {
                role: MessageRole::User,
                content,
            }],
            ResponsesInput::Messages(messages) if !messages.is_empty() => {
                messages.into_iter().map(Into::into).collect()
            }
            _ => return Err(ApiError::invalid("input must contain non-empty text")),
        };
        let output_tokens = self.max_output_tokens.unwrap_or(default_tokens);
        if output_tokens == 0 || output_tokens > max_tokens {
            return Err(ApiError::invalid("max_output_tokens is out of range"));
        }
        Ok(GatewayInferenceRequest {
            requested_model: self.model,
            instructions: self
                .instructions
                .into_iter()
                .map(|content| GatewayInstruction {
                    content,
                    role: InstructionRole::System,
                })
                .collect(),
            messages,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            generation: GenerationParameters {
                temperature: self.temperature,
                top_p: self.top_p,
                max_output_tokens: output_tokens,
                stop: Vec::new(),
            },
            stream: self.stream,
            metadata: RequestMetadata::new(),
        })
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResponsesResponse {
    pub id: String,
    pub object: &'static str,
    pub model: String,
    pub output: Vec<ResponsesOutput>,
    pub usage: UsageResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResponsesOutput {
    pub r#type: &'static str,
    pub role: &'static str,
    pub content: Vec<ResponsesOutputText>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResponsesOutputText {
    pub r#type: &'static str,
    pub text: String,
}

impl ResponsesResponse {
    fn from_gateway(response: gateway_types::GatewayResponse) -> Self {
        Self {
            id: response.id,
            object: "response",
            model: response.model,
            output: vec![ResponsesOutput {
                r#type: "message",
                role: "assistant",
                content: vec![ResponsesOutputText {
                    r#type: "output_text",
                    text: response.content,
                }],
            }],
            usage: response.usage.into(),
        }
    }
}

/// OpenAI embeddings request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub input: EmbeddingsInput,
    pub dimensions: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum EmbeddingsInput {
    One(String),
    Many(Vec<String>),
}

impl EmbeddingsRequest {
    fn into_gateway(self, max_body_bytes: usize) -> Result<GatewayEmbeddingRequest, ApiError> {
        let inputs = match self.input {
            EmbeddingsInput::One(value) => vec![value],
            EmbeddingsInput::Many(values) => values,
        };
        if inputs.is_empty() || inputs.iter().any(String::is_empty) {
            return Err(ApiError::invalid("input must contain non-empty text"));
        }
        if inputs.len() > 2048 || inputs.iter().map(String::len).sum::<usize>() > max_body_bytes {
            return Err(ApiError::invalid("embedding input exceeds gateway limits"));
        }
        if self
            .dimensions
            .is_some_and(|value| value == 0 || value > 32768)
        {
            return Err(ApiError::invalid("dimensions is out of range"));
        }
        Ok(GatewayEmbeddingRequest {
            requested_model: self.model,
            inputs,
            dimensions: self.dimensions,
            metadata: RequestMetadata::new(),
        })
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmbeddingsResponse {
    pub object: &'static str,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: UsageResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmbeddingData {
    pub object: &'static str,
    pub index: usize,
    pub embedding: Vec<f32>,
}

impl EmbeddingsResponse {
    fn from_gateway(response: gateway_types::GatewayEmbeddingResponse) -> Self {
        Self {
            object: "list",
            data: response
                .embeddings
                .into_iter()
                .enumerate()
                .map(|(index, embedding)| EmbeddingData {
                    object: "embedding",
                    index,
                    embedding,
                })
                .collect(),
            model: response.model,
            usage: TokenUsage {
                prompt_tokens: response.usage.input_tokens,
                completion_tokens: 0,
                estimated: false,
            }
            .into(),
        }
    }
}

/// OpenAI-compatible chat-completions request supported by v0.1.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatCompletionRequest {
    /// Public model alias.
    pub model: String,
    /// Ordered text-only messages.
    pub messages: Vec<ChatMessage>,
    /// Enable SSE streaming.
    #[serde(default)]
    pub stream: bool,
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// Nucleus sampling threshold.
    pub top_p: Option<f32>,
    /// Legacy output-token limit.
    pub max_tokens: Option<u32>,
    /// Current output-token limit.
    pub max_completion_tokens: Option<u32>,
    /// Stop sequence or sequences.
    pub stop: Option<StopInput>,
    /// Streaming response options.
    pub stream_options: Option<StreamOptions>,
    /// Unsupported in v0.1 and rejected when present.
    #[schema(value_type = Option<Object>)]
    pub tools: Option<Value>,
    /// Unsupported in v0.1 and rejected when present.
    #[schema(value_type = Option<Object>)]
    pub tool_choice: Option<Value>,
    /// Unsupported in v0.1 and rejected when present.
    #[schema(value_type = Option<Object>)]
    pub response_format: Option<Value>,
    /// v0.1 supports exactly one choice.
    pub n: Option<u32>,
}

impl ChatCompletionRequest {
    fn into_gateway(
        self,
        default_tokens: u32,
        max_tokens: u32,
    ) -> Result<GatewayRequest, ApiError> {
        if self.messages.is_empty()
            || self
                .messages
                .iter()
                .any(|message| message.content.is_empty())
        {
            return Err(ApiError::invalid("messages must contain non-empty text"));
        }
        if self.tools.is_some() || self.tool_choice.is_some() || self.response_format.is_some() {
            return Err(ApiError::invalid(
                "tools and structured output are not supported in v0.1",
            ));
        }
        if self.n.unwrap_or(1) != 1 {
            return Err(ApiError::invalid("n must be 1"));
        }
        if self.max_tokens.is_some() && self.max_completion_tokens.is_some() {
            return Err(ApiError::invalid("set only one output-token limit"));
        }
        let output_tokens = self
            .max_completion_tokens
            .or(self.max_tokens)
            .unwrap_or(default_tokens);
        if output_tokens == 0 || output_tokens > max_tokens {
            return Err(ApiError::invalid("output-token limit is out of range"));
        }
        if self
            .temperature
            .is_some_and(|value| !(0.0..=2.0).contains(&value))
        {
            return Err(ApiError::invalid("temperature must be between 0 and 2"));
        }
        if self
            .top_p
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(ApiError::invalid("top_p must be between 0 and 1"));
        }
        Ok(GatewayRequest {
            model: self.model,
            messages: self.messages.into_iter().map(Into::into).collect(),
            stream: self.stream,
            stream_include_usage: self
                .stream_options
                .is_some_and(|options| options.include_usage),
            generation: GenerationParameters {
                temperature: self.temperature,
                top_p: self.top_p,
                max_output_tokens: output_tokens,
                stop: self.stop.map(StopInput::into_vec).unwrap_or_default(),
            },
        })
    }
}

/// OpenAI text message DTO.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatMessage {
    /// Message role.
    pub role: MessageRoleDto,
    /// Text content.
    pub content: String,
}

impl From<ChatMessage> for GatewayMessage {
    fn from(message: ChatMessage) -> Self {
        Self {
            role: match message.role {
                MessageRoleDto::System => MessageRole::System,
                MessageRoleDto::User => MessageRole::User,
                MessageRoleDto::Assistant => MessageRole::Assistant,
            },
            content: message.content,
        }
    }
}

/// Supported OpenAI message roles.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MessageRoleDto {
    /// System instruction.
    System,
    /// User input.
    User,
    /// Assistant output.
    Assistant,
}

/// One stop sequence or a list of sequences.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum StopInput {
    /// One sequence.
    One(String),
    /// Multiple sequences.
    Many(Vec<String>),
}

impl StopInput {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

/// OpenAI stream options supported by v0.1.
#[derive(Debug, Deserialize, ToSchema)]
pub struct StreamOptions {
    /// Emit a final usage chunk.
    #[serde(default)]
    pub include_usage: bool,
}

/// Non-streaming OpenAI chat completion response.
#[derive(Debug, Serialize, ToSchema)]
pub struct ChatCompletionResponse {
    /// Completion identifier.
    pub id: String,
    /// Object discriminator.
    pub object: &'static str,
    /// Unix creation timestamp.
    pub created: i64,
    /// Public model alias.
    pub model: String,
    /// Completion choices.
    pub choices: Vec<CompletionChoice>,
    /// Token usage.
    pub usage: UsageResponse,
}

impl ChatCompletionResponse {
    fn from_gateway(response: gateway_types::GatewayResponse) -> Self {
        Self {
            id: response.id,
            object: "chat.completion",
            created: Utc::now().timestamp(),
            model: response.model,
            choices: vec![CompletionChoice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant",
                    content: response.content,
                },
                finish_reason: response.finish_reason,
            }],
            usage: response.usage.into(),
        }
    }
}

/// Completion choice DTO.
#[derive(Debug, Serialize, ToSchema)]
pub struct CompletionChoice {
    /// Choice index.
    pub index: u32,
    /// Assistant message.
    pub message: ResponseMessage,
    /// Provider finish reason.
    pub finish_reason: Option<String>,
}

/// Assistant response message DTO.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResponseMessage {
    /// Always `assistant`.
    pub role: &'static str,
    /// Generated text.
    pub content: String,
}

/// OpenAI token usage DTO.
#[derive(Debug, Serialize, ToSchema)]
pub struct UsageResponse {
    /// Input tokens.
    pub prompt_tokens: u64,
    /// Output tokens.
    pub completion_tokens: u64,
    /// Total tokens.
    pub total_tokens: u64,
}

impl From<TokenUsage> for UsageResponse {
    fn from(usage: TokenUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens(),
        }
    }
}

/// OpenAI model list.
#[derive(Debug, Serialize, ToSchema)]
pub struct ModelList {
    /// Always `list`.
    pub object: &'static str,
    /// Available public models.
    pub data: Vec<ModelObject>,
}

/// OpenAI model descriptor.
#[derive(Debug, Serialize, ToSchema)]
pub struct ModelObject {
    /// Public alias.
    pub id: String,
    /// Always `model`.
    pub object: &'static str,
    /// Creation timestamp; zero for configured aliases.
    pub created: i64,
    /// Gateway owner label.
    pub owned_by: &'static str,
}

/// Tenant-scoped Virtual Key issuance request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateVirtualKeyRequest {
    /// Optional project binding.
    pub project_id: Option<String>,
    /// Optional user binding.
    pub user_id: Option<String>,
    /// Granted scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Optional expiration time.
    pub expires_at: Option<DateTime<Utc>>,
    /// Daily token limit, or the configured default.
    pub daily_token_limit: Option<u64>,
}

/// One-time Virtual Key issuance response.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateVirtualKeyResponse {
    /// Key identifier.
    pub id: Uuid,
    /// Plaintext bearer key, returned only once.
    pub key: String,
    /// Non-secret lookup prefix.
    pub key_prefix: String,
    /// Optional expiration.
    pub expires_at: Option<DateTime<Utc>>,
    /// Daily token limit.
    pub daily_token_limit: u64,
}

#[derive(Debug, Serialize, ToSchema)]
struct StatusResponse {
    status: &'static str,
}

/// OpenAI error envelope.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ErrorEnvelope {
    /// Error details.
    pub error: ErrorObject,
}

/// OpenAI error details.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ErrorObject {
    /// Safe client-facing message.
    pub message: String,
    /// OpenAI-compatible category.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Related request parameter.
    pub param: Option<String>,
    /// Stable machine-readable code.
    pub code: String,
}

#[derive(Clone, Debug)]
struct ApiError {
    status: StatusCode,
    body: ErrorEnvelope,
}

impl ApiError {
    fn new(status: StatusCode, error_type: &str, code: &str, message: &str) -> Self {
        Self {
            status,
            body: ErrorEnvelope {
                error: ErrorObject {
                    message: message.into(),
                    error_type: error_type.into(),
                    param: None,
                    code: code.into(),
                },
            },
        }
    }

    fn invalid(message: &str) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "invalid_request",
            message,
        )
    }

    fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "invalid_api_key",
            "invalid authentication credential",
        )
    }

    fn forbidden(message: &str) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "permission_error",
            "permission_denied",
            message,
        )
    }

    fn conflict(message: &str) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "invalid_request_error",
            "conflict",
            message,
        )
    }

    fn not_found(message: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "invalid_request_error",
            "not_found",
            message,
        )
    }

    fn service_unavailable(message: &str) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "server_error",
            "service_unavailable",
            message,
        )
    }

    fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "internal_error",
            "internal gateway error",
        )
    }
}

impl From<GatewayError> for ApiError {
    fn from(error: GatewayError) -> Self {
        match error {
            GatewayError::Routing(_) => Self::not_found("model is not available"),
            GatewayError::Policy(_) => Self::new(
                StatusCode::FORBIDDEN,
                "permission_error",
                "policy_denied",
                "request is not permitted by policy",
            ),
            GatewayError::Security(gateway_security::SecurityError::Blocked) => Self::new(
                StatusCode::FORBIDDEN,
                "gateway_security_error",
                "security_policy_blocked",
                "Request blocked by organization security policy.",
            ),
            GatewayError::Security(gateway_security::SecurityError::PromptInjectionDetected) => {
                Self::new(
                    StatusCode::FORBIDDEN,
                    "gateway_security_error",
                    "prompt_injection_detected",
                    "Request blocked by organization security policy.",
                )
            }
            GatewayError::Security(gateway_security::SecurityError::SecretExposureDetected) => {
                Self::new(
                    StatusCode::FORBIDDEN,
                    "gateway_security_error",
                    "secret_exposure_detected",
                    "Request blocked by organization security policy.",
                )
            }
            GatewayError::Security(gateway_security::SecurityError::SensitiveDataDetected) => {
                Self::new(
                    StatusCode::FORBIDDEN,
                    "gateway_security_error",
                    "sensitive_data_detected",
                    "Request blocked by organization security policy.",
                )
            }
            GatewayError::Security(gateway_security::SecurityError::ApprovalRequired) => Self::new(
                StatusCode::ACCEPTED,
                "gateway_security_error",
                "approval_required",
                "Human approval is required.",
            ),
            GatewayError::Security(_) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "gateway_security_error",
                "security_inspection_failed",
                "Security inspection failed.",
            ),
            GatewayError::Quota(QuotaError::Exceeded) => Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "insufficient_quota",
                "insufficient_quota",
                "daily token quota exceeded",
            ),
            GatewayError::Quota(QuotaError::Unavailable) | GatewayError::Metering => {
                Self::service_unavailable("gateway accounting unavailable")
            }
            GatewayError::Provider(gateway_providers::ProviderError::Timeout) => Self::new(
                StatusCode::GATEWAY_TIMEOUT,
                "server_error",
                "upstream_timeout",
                "upstream request timed out",
            ),
            GatewayError::Provider(gateway_providers::ProviderError::RateLimited) => Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_error",
                "upstream_rate_limit",
                "upstream rate limit exceeded",
            ),
            GatewayError::Provider(_) => Self::new(
                StatusCode::BAD_GATEWAY,
                "server_error",
                "upstream_error",
                "upstream provider request failed",
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ChatCompletionRequest, StopInput, openapi_document};

    #[test]
    fn committed_openapi_matches_generated_document() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/openapi.json");
        let committed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let generated = serde_json::to_value(openapi_document()).unwrap();
        assert_eq!(committed, generated);
    }

    #[test]
    fn rejects_v01_non_goals() {
        let request: ChatCompletionRequest = serde_json::from_value(serde_json::json!({
            "model":"gateway-default",
            "messages":[{"role":"user","content":"hello"}],
            "tools":[{"type":"function"}]
        }))
        .unwrap();
        assert!(request.into_gateway(128, 4096).is_err());
        assert_eq!(StopInput::One("stop".into()).into_vec(), vec!["stop"]);
    }
}
