//! Gateway application pipeline, independent of HTTP and concrete adapters.

use std::{
    pin::Pin,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use gateway_metering::MeteringService;
use gateway_policy::{AllowAllPolicyResolver, PolicyError, PolicyResolver};
use gateway_providers::{ModelProvider, ProviderContext, ProviderError, ProviderRegistry};
use gateway_quota::{QuotaError, QuotaService};
use gateway_routing::{RoutePlan, RoutingError, StaticRouter, retryable};
use gateway_security::{SecurityEnforcer, SecurityError};
use gateway_types::{
    GatewayEmbeddingRequest, GatewayEmbeddingResponse, GatewayInferenceRequest, GatewayRequest,
    GatewayResponse, GatewayStreamEvent, GenerationParameters, MessageRole, ModelRoute, Principal,
    QuotaReservation, TokenUsage, UsageEvent, UsageStatus,
};
use thiserror::Error;
use tokio::{sync::mpsc, time::timeout};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

/// Atomically replaceable immutable inference configuration.
#[derive(Clone)]
pub struct GatewayRuntime {
    current: Arc<RwLock<Arc<GatewayService>>>,
    status: Arc<RwLock<RuntimeStatus>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RuntimeStatus {
    pub state: &'static str,
    pub pending: bool,
    pub error: Option<String>,
    pub reconciled_at: DateTime<Utc>,
}

impl GatewayRuntime {
    pub fn new(service: GatewayService) -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(service))),
            status: Arc::new(RwLock::new(RuntimeStatus {
                state: "active",
                pending: false,
                error: None,
                reconciled_at: Utc::now(),
            })),
        }
    }

    pub fn replace(&self, service: GatewayService) {
        *self
            .current
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Arc::new(service);
        *self
            .status
            .write()
            .unwrap_or_else(|error| error.into_inner()) = RuntimeStatus {
            state: "active",
            pending: false,
            error: None,
            reconciled_at: Utc::now(),
        };
    }

    pub fn reconciliation_failed(&self, message: impl Into<String>) {
        *self
            .status
            .write()
            .unwrap_or_else(|error| error.into_inner()) = RuntimeStatus {
            state: "degraded",
            pending: true,
            error: Some(message.into()),
            reconciled_at: Utc::now(),
        };
    }

    pub fn status(&self) -> RuntimeStatus {
        self.status
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn model_route(&self) -> ModelRoute {
        self.service().model_route().clone()
    }

    pub async fn execute(
        &self,
        request_id: Uuid,
        principal: Principal,
        request: GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        self.service().execute(request_id, principal, request).await
    }

    pub async fn stream(
        &self,
        request_id: Uuid,
        principal: Principal,
        request: GatewayRequest,
    ) -> Result<GatewayResultStream, GatewayError> {
        self.service().stream(request_id, principal, request).await
    }

    pub async fn execute_inference(
        &self,
        request_id: Uuid,
        principal: Principal,
        request: GatewayInferenceRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        self.service()
            .execute_inference(request_id, principal, request)
            .await
    }

    pub async fn stream_inference(
        &self,
        request_id: Uuid,
        principal: Principal,
        request: GatewayInferenceRequest,
    ) -> Result<GatewayResultStream, GatewayError> {
        self.service()
            .stream_inference(request_id, principal, request)
            .await
    }

    pub async fn embed(
        &self,
        request_id: Uuid,
        principal: Principal,
        request: GatewayEmbeddingRequest,
    ) -> Result<GatewayEmbeddingResponse, GatewayError> {
        self.service().embed(request_id, principal, request).await
    }

    fn service(&self) -> Arc<GatewayService> {
        self.current
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

/// Stream returned by the application service.
pub type GatewayResultStream =
    Pin<Box<dyn Stream<Item = Result<GatewayStreamEvent, GatewayError>> + Send>>;

/// Main v0.1 inference use case.
#[derive(Clone)]
pub struct GatewayService {
    router: StaticRouter,
    providers: ProviderRegistry,
    route_plan: Option<RoutePlan>,
    policy: Arc<dyn PolicyResolver>,
    security: Option<SecurityEnforcer>,
    quota: QuotaService,
    metering: MeteringService,
    request_timeout: Duration,
}

impl GatewayService {
    /// Assemble the provider-neutral pipeline.
    pub fn new(
        router: StaticRouter,
        provider: Arc<dyn ModelProvider>,
        quota: QuotaService,
        metering: MeteringService,
        request_timeout: Duration,
    ) -> Self {
        Self {
            router,
            providers: ProviderRegistry::new(std::iter::once(provider.clone())),
            route_plan: None,
            policy: Arc::new(AllowAllPolicyResolver),
            security: None,
            quota,
            metering,
            request_timeout,
        }
    }

    /// Assemble a service with a provider registry for multi-provider routes.
    pub fn with_registry(
        router: StaticRouter,
        providers: ProviderRegistry,
        quota: QuotaService,
        metering: MeteringService,
        request_timeout: Duration,
    ) -> Result<Self, GatewayError> {
        let provider_id = router.route().provider.clone();
        providers
            .get(&provider_id)
            .ok_or(GatewayError::Provider(ProviderError::Unavailable))?;
        Ok(Self {
            router,
            providers,
            route_plan: None,
            policy: Arc::new(AllowAllPolicyResolver),
            security: None,
            quota,
            metering,
            request_timeout,
        })
    }

    /// Assemble the complete policy-aware, multi-provider v0.2 pipeline.
    pub fn configured(
        router: StaticRouter,
        route_plan: RoutePlan,
        providers: ProviderRegistry,
        policy: Arc<dyn PolicyResolver>,
        quota: QuotaService,
        metering: MeteringService,
        request_timeout: Duration,
    ) -> Result<Self, GatewayError> {
        providers
            .get(&router.route().provider)
            .ok_or(GatewayError::Provider(ProviderError::Unavailable))?;
        Ok(Self {
            router,
            providers,
            route_plan: Some(route_plan),
            policy,
            security: None,
            quota,
            metering,
            request_timeout,
        })
    }

    pub fn with_security(mut self, security: SecurityEnforcer) -> Self {
        self.security = Some(security);
        self
    }

    /// Return the public model route.
    pub fn model_route(&self) -> &ModelRoute {
        self.router.route()
    }

    /// Execute and meter a non-streaming completion.
    pub async fn execute(
        &self,
        request_id: Uuid,
        principal: Principal,
        mut request: GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        self.authorize(
            &principal,
            &request.model,
            "chat_completion",
            request.generation.max_output_tokens,
        )
        .await?;
        self.inspect_messages(request_id, &principal, &mut request.messages)
            .await?;
        let routes = self.routes(&request.model)?;
        let mut last_error = ProviderError::Unavailable;
        for (index, route) in routes.iter().enumerate() {
            let started = Instant::now();
            let reservation = self
                .reserve(request_id, &principal, &request, route)
                .await?;
            let provider = self.provider_for(route)?;
            let context = ProviderContext {
                request_id,
                upstream_model: route.upstream_model.clone(),
            };
            let result = timeout(
                self.request_timeout,
                provider.execute(context, request.clone()),
            )
            .await
            .map_err(|_| ProviderError::Timeout)
            .and_then(|result| result);
            match result {
                Ok(mut response) => {
                    let inspection = self
                        .inspect_response(request_id, &principal, &mut response)
                        .await;
                    self.record(&reservation, response.usage, UsageStatus::Succeeded)
                        .await?;
                    inspection?;
                    log_completion(&reservation, UsageStatus::Succeeded, started);
                    return Ok(response);
                }
                Err(error) => {
                    self.record(
                        &reservation,
                        TokenUsage {
                            prompt_tokens: reservation.prompt_tokens,
                            completion_tokens: 0,
                            estimated: true,
                        },
                        UsageStatus::ProviderFailed,
                    )
                    .await?;
                    log_completion(&reservation, UsageStatus::ProviderFailed, started);
                    let can_retry = index + 1 < routes.len() && retryable(&error);
                    last_error = error;
                    if !can_retry {
                        break;
                    }
                }
            }
        }
        Err(GatewayError::Provider(last_error))
    }

    /// Execute a canonical Responses API request through the same v0.1 pipeline.
    pub async fn execute_inference(
        &self,
        request_id: Uuid,
        principal: Principal,
        mut request: GatewayInferenceRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        self.authorize(
            &principal,
            &request.requested_model,
            "response",
            request.generation.max_output_tokens,
        )
        .await?;
        self.inspect_inference(request_id, &principal, &mut request)
            .await?;
        let routes = self.routes(&request.requested_model)?;
        let legacy = canonical_request(&request, false);
        let mut last_error = ProviderError::Unavailable;
        for (index, route) in routes.iter().enumerate() {
            let reservation = self.reserve(request_id, &principal, &legacy, route).await?;
            let context = ProviderContext {
                request_id,
                upstream_model: route.upstream_model.clone(),
            };
            let provider = self.provider_for(route)?;
            let result = timeout(
                self.request_timeout,
                provider.infer(context, request.clone()),
            )
            .await
            .map_err(|_| ProviderError::Timeout)
            .and_then(|value| value);
            match result {
                Ok(mut response) => {
                    let inspection = self
                        .inspect_response(request_id, &principal, &mut response)
                        .await;
                    self.record(&reservation, response.usage, UsageStatus::Succeeded)
                        .await?;
                    inspection?;
                    return Ok(response);
                }
                Err(error) => {
                    self.record(
                        &reservation,
                        TokenUsage {
                            prompt_tokens: reservation.prompt_tokens,
                            ..Default::default()
                        },
                        UsageStatus::ProviderFailed,
                    )
                    .await?;
                    let can_retry = index + 1 < routes.len() && retryable(&error);
                    last_error = error;
                    if !can_retry {
                        break;
                    }
                }
            }
        }
        Err(GatewayError::Provider(last_error))
    }

    /// Start a canonical Responses stream without buffering downstream output.
    pub async fn stream_inference(
        &self,
        request_id: Uuid,
        principal: Principal,
        mut request: GatewayInferenceRequest,
    ) -> Result<GatewayResultStream, GatewayError> {
        self.authorize(
            &principal,
            &request.requested_model,
            "response",
            request.generation.max_output_tokens,
        )
        .await?;
        self.inspect_inference(request_id, &principal, &mut request)
            .await?;
        let output_security = self.output_security(request_id, &principal).await?;
        let routes = self.routes(&request.requested_model)?;
        let legacy = canonical_request(&request, true);
        let mut last_error = ProviderError::Unavailable;
        for (index, route) in routes.iter().enumerate() {
            let reservation = self.reserve(request_id, &principal, &legacy, route).await?;
            let context = ProviderContext {
                request_id,
                upstream_model: route.upstream_model.clone(),
            };
            let provider = self.provider_for(route)?;
            let attempt = timeout(
                self.request_timeout,
                provider.stream_infer(context, request.clone()),
            )
            .await
            .map_err(|_| ProviderError::Timeout)
            .and_then(|value| value);
            match attempt {
                Ok(upstream) => {
                    let (sender, receiver) = mpsc::channel(16);
                    let metering = self.metering.clone();
                    let quota = self.quota.clone();
                    let security = output_security.clone();
                    tokio::spawn(async move {
                        consume_stream(
                            upstream,
                            sender,
                            reservation,
                            metering,
                            quota,
                            true,
                            Instant::now(),
                            security,
                        )
                        .await;
                    });
                    return Ok(Box::pin(ReceiverStream::new(receiver)));
                }
                Err(error) => {
                    self.record(
                        &reservation,
                        TokenUsage {
                            prompt_tokens: reservation.prompt_tokens,
                            ..Default::default()
                        },
                        UsageStatus::ProviderFailed,
                    )
                    .await?;
                    let can_retry = index + 1 < routes.len() && retryable(&error);
                    last_error = error;
                    if !can_retry {
                        break;
                    }
                }
            }
        }
        Err(GatewayError::Provider(last_error))
    }

    /// Execute a canonical embeddings request with durable usage accounting.
    pub async fn embed(
        &self,
        request_id: Uuid,
        principal: Principal,
        mut request: GatewayEmbeddingRequest,
    ) -> Result<GatewayEmbeddingResponse, GatewayError> {
        self.authorize(&principal, &request.requested_model, "embedding", 0)
            .await?;
        self.inspect_embeddings(request_id, &principal, &mut request)
            .await?;
        let routes = self.routes(&request.requested_model)?;
        let legacy = GatewayRequest {
            model: request.requested_model.clone(),
            messages: request
                .inputs
                .iter()
                .map(|content| gateway_types::GatewayMessage {
                    role: MessageRole::User,
                    content: content.clone(),
                })
                .collect(),
            stream: false,
            stream_include_usage: false,
            generation: GenerationParameters::default(),
        };
        let mut last_error = ProviderError::Unavailable;
        for (index, route) in routes.iter().enumerate() {
            let reservation = self.reserve(request_id, &principal, &legacy, route).await?;
            let context = ProviderContext {
                request_id,
                upstream_model: route.upstream_model.clone(),
            };
            let provider = self.provider_for(route)?;
            let result = timeout(
                self.request_timeout,
                provider.embed(context, request.clone()),
            )
            .await
            .map_err(|_| ProviderError::Timeout)
            .and_then(|value| value);
            match result {
                Ok(response) => {
                    let usage = TokenUsage {
                        prompt_tokens: response.usage.input_tokens,
                        completion_tokens: 0,
                        estimated: false,
                    };
                    self.record(&reservation, usage, UsageStatus::Succeeded)
                        .await?;
                    return Ok(response);
                }
                Err(error) => {
                    self.record(
                        &reservation,
                        TokenUsage {
                            prompt_tokens: reservation.prompt_tokens,
                            ..Default::default()
                        },
                        UsageStatus::ProviderFailed,
                    )
                    .await?;
                    let can_retry = index + 1 < routes.len() && retryable(&error);
                    last_error = error;
                    if !can_retry {
                        break;
                    }
                }
            }
        }
        Err(GatewayError::Provider(last_error))
    }

    /// Start a bounded, cancellation-aware streaming completion.
    pub async fn stream(
        &self,
        request_id: Uuid,
        principal: Principal,
        mut request: GatewayRequest,
    ) -> Result<GatewayResultStream, GatewayError> {
        self.authorize(
            &principal,
            &request.model,
            "chat_completion",
            request.generation.max_output_tokens,
        )
        .await?;
        self.inspect_messages(request_id, &principal, &mut request.messages)
            .await?;
        let output_security = self.output_security(request_id, &principal).await?;
        let routes = self.routes(&request.model)?;
        let include_usage = request.stream_include_usage;
        let mut last_error = ProviderError::Unavailable;
        for (index, route) in routes.iter().enumerate() {
            let started = Instant::now();
            let reservation = self
                .reserve(request_id, &principal, &request, route)
                .await?;
            let context = ProviderContext {
                request_id,
                upstream_model: route.upstream_model.clone(),
            };
            let provider = self.provider_for(route)?;
            match timeout(
                self.request_timeout,
                provider.stream(context, request.clone()),
            )
            .await
            .map_err(|_| ProviderError::Timeout)
            .and_then(|value| value)
            {
                Ok(upstream) => {
                    let (sender, receiver) = mpsc::channel(16);
                    let metering = self.metering.clone();
                    let quota = self.quota.clone();
                    let security = output_security.clone();
                    tokio::spawn(async move {
                        consume_stream(
                            upstream,
                            sender,
                            reservation,
                            metering,
                            quota,
                            include_usage,
                            started,
                            security,
                        )
                        .await;
                    });
                    return Ok(Box::pin(ReceiverStream::new(receiver)));
                }
                Err(error) => {
                    self.record(
                        &reservation,
                        TokenUsage {
                            prompt_tokens: reservation.prompt_tokens,
                            completion_tokens: 0,
                            estimated: true,
                        },
                        UsageStatus::ProviderFailed,
                    )
                    .await?;
                    log_completion(&reservation, UsageStatus::ProviderFailed, started);
                    let can_retry = index + 1 < routes.len() && retryable(&error);
                    last_error = error;
                    if !can_retry {
                        break;
                    }
                }
            }
        }
        Err(GatewayError::Provider(last_error))
    }

    async fn reserve(
        &self,
        request_id: Uuid,
        principal: &Principal,
        request: &GatewayRequest,
        route: &ModelRoute,
    ) -> Result<QuotaReservation, GatewayError> {
        self.quota
            .reserve(
                request_id,
                principal,
                route,
                request.prompt_token_upper_bound(),
                u64::from(request.generation.max_output_tokens),
            )
            .await
            .map_err(Into::into)
    }

    async fn record(
        &self,
        reservation: &QuotaReservation,
        usage: TokenUsage,
        status: UsageStatus,
    ) -> Result<(), GatewayError> {
        let result = record_usage(&self.metering, reservation, usage, status)
            .await
            .map_err(|_| GatewayError::Metering);
        self.quota.release_counter(reservation.reservation_id).await;
        result
    }

    async fn authorize(
        &self,
        principal: &Principal,
        model: &str,
        operation: &str,
        output_tokens: u32,
    ) -> Result<(), GatewayError> {
        self.policy
            .resolve(principal)
            .await?
            .authorize(model, operation, output_tokens)?;
        Ok(())
    }

    async fn inspect_messages(
        &self,
        request_id: Uuid,
        principal: &Principal,
        messages: &mut [gateway_types::GatewayMessage],
    ) -> Result<(), GatewayError> {
        let Some(security) = &self.security else {
            return Ok(());
        };
        for (index, message) in messages.iter_mut().enumerate() {
            let context = gateway_types::InspectionContext {
                request_id,
                tenant_id: principal.tenant_id.clone(),
                project_id: principal.project_id.clone(),
                principal_id: principal.principal_id.clone(),
                stage: format!("llm_input:{index}"),
                tool_risk: None,
            };
            let (content, _) = security
                .inspect(
                    context,
                    gateway_types::InspectionContent::PromptText(message.content.clone()),
                )
                .await?;
            if let gateway_types::InspectionContent::PromptText(value) = content {
                message.content = value;
            }
        }
        Ok(())
    }

    async fn inspect_inference(
        &self,
        request_id: Uuid,
        principal: &Principal,
        request: &mut GatewayInferenceRequest,
    ) -> Result<(), GatewayError> {
        let Some(security) = &self.security else {
            return Ok(());
        };
        for (index, instruction) in request.instructions.iter_mut().enumerate() {
            let context = gateway_types::InspectionContext {
                request_id,
                tenant_id: principal.tenant_id.clone(),
                project_id: principal.project_id.clone(),
                principal_id: principal.principal_id.clone(),
                stage: format!("llm_instruction:{index}"),
                tool_risk: None,
            };
            let (content, _) = security
                .inspect(
                    context,
                    gateway_types::InspectionContent::PromptText(instruction.content.clone()),
                )
                .await?;
            if let gateway_types::InspectionContent::PromptText(value) = content {
                instruction.content = value;
            }
        }
        for (index, tool) in request.tools.iter_mut().enumerate() {
            let context = gateway_types::InspectionContext {
                request_id,
                tenant_id: principal.tenant_id.clone(),
                project_id: principal.project_id.clone(),
                principal_id: principal.principal_id.clone(),
                stage: format!("llm_tool_definition:{index}"),
                tool_risk: None,
            };
            let value = serde_json::to_value(&*tool)
                .map_err(|_| GatewayError::Security(SecurityError::InspectionFailed))?;
            let (content, _) = security
                .inspect(
                    context,
                    gateway_types::InspectionContent::StructuredInput(value),
                )
                .await?;
            if let gateway_types::InspectionContent::StructuredInput(value) = content {
                *tool = serde_json::from_value(value)
                    .map_err(|_| GatewayError::Security(SecurityError::InspectionFailed))?
            }
        }
        if let Some(format) = request.response_format.as_mut() {
            let context = gateway_types::InspectionContext {
                request_id,
                tenant_id: principal.tenant_id.clone(),
                project_id: principal.project_id.clone(),
                principal_id: principal.principal_id.clone(),
                stage: "llm_response_format".into(),
                tool_risk: None,
            };
            let value = serde_json::to_value(&*format)
                .map_err(|_| GatewayError::Security(SecurityError::InspectionFailed))?;
            let (content, _) = security
                .inspect(
                    context,
                    gateway_types::InspectionContent::StructuredInput(value),
                )
                .await?;
            if let gateway_types::InspectionContent::StructuredInput(value) = content {
                *format = serde_json::from_value(value)
                    .map_err(|_| GatewayError::Security(SecurityError::InspectionFailed))?
            }
        }
        self.inspect_messages(request_id, principal, &mut request.messages)
            .await
    }

    async fn inspect_embeddings(
        &self,
        request_id: Uuid,
        principal: &Principal,
        request: &mut GatewayEmbeddingRequest,
    ) -> Result<(), GatewayError> {
        let Some(security) = &self.security else {
            return Ok(());
        };
        for (index, input) in request.inputs.iter_mut().enumerate() {
            let context = gateway_types::InspectionContext {
                request_id,
                tenant_id: principal.tenant_id.clone(),
                project_id: principal.project_id.clone(),
                principal_id: principal.principal_id.clone(),
                stage: format!("embedding_input:{index}"),
                tool_risk: None,
            };
            let (content, _) = security
                .inspect(
                    context,
                    gateway_types::InspectionContent::PromptText(input.clone()),
                )
                .await?;
            if let gateway_types::InspectionContent::PromptText(value) = content {
                *input = value;
            }
        }
        Ok(())
    }

    async fn inspect_response(
        &self,
        request_id: Uuid,
        principal: &Principal,
        response: &mut GatewayResponse,
    ) -> Result<(), GatewayError> {
        let Some(security) = &self.security else {
            return Ok(());
        };
        let context = gateway_types::InspectionContext {
            request_id,
            tenant_id: principal.tenant_id.clone(),
            project_id: principal.project_id.clone(),
            principal_id: principal.principal_id.clone(),
            stage: "llm_output".into(),
            tool_risk: None,
        };
        let (content, _) = security
            .inspect(
                context,
                gateway_types::InspectionContent::ModelOutput(response.content.clone()),
            )
            .await?;
        if let gateway_types::InspectionContent::ModelOutput(value) = content {
            response.content = value;
        }
        Ok(())
    }

    async fn output_security(
        &self,
        request_id: Uuid,
        principal: &Principal,
    ) -> Result<Option<(SecurityEnforcer, Principal, Uuid)>, GatewayError> {
        let Some(security) = &self.security else {
            return Ok(None);
        };
        let context = gateway_types::InspectionContext {
            request_id,
            tenant_id: principal.tenant_id.clone(),
            project_id: principal.project_id.clone(),
            principal_id: principal.principal_id.clone(),
            stage: "llm_output".into(),
            tool_risk: None,
        };
        Ok(security
            .enabled_for(&context)
            .await?
            .then(|| (security.clone(), principal.clone(), request_id)))
    }

    fn routes(&self, model: &str) -> Result<Vec<ModelRoute>, GatewayError> {
        Ok(match &self.route_plan {
            Some(plan) => plan.route(model)?,
            None => vec![self.router.resolve(model)?],
        })
    }

    fn provider_for(&self, route: &ModelRoute) -> Result<Arc<dyn ModelProvider>, GatewayError> {
        self.providers
            .get(&route.provider)
            .ok_or(GatewayError::Provider(ProviderError::Unavailable))
    }
}

#[allow(clippy::too_many_arguments)]
async fn consume_stream(
    mut upstream: gateway_providers::GatewayStream,
    sender: mpsc::Sender<Result<GatewayStreamEvent, GatewayError>>,
    reservation: QuotaReservation,
    metering: MeteringService,
    quota: QuotaService,
    include_usage: bool,
    started: Instant,
    security: Option<(SecurityEnforcer, Principal, Uuid)>,
) {
    if let Some((security, principal, request_id)) = security {
        consume_inspected_stream(
            upstream,
            sender,
            reservation,
            metering,
            quota,
            include_usage,
            started,
            security,
            principal,
            request_id,
        )
        .await;
        return;
    }
    let mut output_bytes = 0_u64;
    let mut usage = None;
    let mut status = UsageStatus::Succeeded;
    while let Some(item) = upstream.next().await {
        match item {
            Ok(event) => {
                let should_send = match &event {
                    GatewayStreamEvent::Delta { content } => {
                        output_bytes = output_bytes.saturating_add(content.len() as u64);
                        true
                    }
                    GatewayStreamEvent::Usage(provider_usage) => {
                        usage = Some(*provider_usage);
                        include_usage
                    }
                    _ => true,
                };
                if should_send && sender.send(Ok(event)).await.is_err() {
                    status = UsageStatus::Interrupted;
                    break;
                }
            }
            Err(error) => {
                status = UsageStatus::ProviderFailed;
                let _ = sender.send(Err(GatewayError::Provider(error))).await;
                break;
            }
        }
    }
    let usage = usage.unwrap_or(TokenUsage {
        prompt_tokens: reservation.prompt_tokens,
        completion_tokens: output_bytes,
        estimated: true,
    });
    let recorded = record_usage(&metering, &reservation, usage, status)
        .await
        .is_err();
    quota.release_counter(reservation.reservation_id).await;
    if recorded {
        let _ = sender.send(Err(GatewayError::Metering)).await;
    } else {
        log_completion(&reservation, status, started);
    }
}

#[allow(clippy::too_many_arguments)]
async fn consume_inspected_stream(
    mut upstream: gateway_providers::GatewayStream,
    sender: mpsc::Sender<Result<GatewayStreamEvent, GatewayError>>,
    reservation: QuotaReservation,
    metering: MeteringService,
    quota: QuotaService,
    include_usage: bool,
    started: Instant,
    security: SecurityEnforcer,
    principal: Principal,
    request_id: Uuid,
) {
    let mut events = Vec::new();
    let mut output = String::new();
    let mut usage = None;
    let mut status = UsageStatus::Succeeded;
    while let Some(item) = upstream.next().await {
        match item {
            Ok(GatewayStreamEvent::Delta { content }) => {
                output.push_str(&content);
                if output.len() > 16 * 1024 * 1024 {
                    status = UsageStatus::Interrupted;
                    break;
                }
            }
            Ok(GatewayStreamEvent::Usage(value)) => {
                usage = Some(value);
                if include_usage {
                    events.push(GatewayStreamEvent::Usage(value));
                }
            }
            Ok(event) => events.push(event),
            Err(error) => {
                status = UsageStatus::ProviderFailed;
                let _ = sender.send(Err(GatewayError::Provider(error))).await;
                break;
            }
        }
    }
    if status == UsageStatus::Succeeded {
        let context = gateway_types::InspectionContext {
            request_id,
            tenant_id: principal.tenant_id,
            project_id: principal.project_id,
            principal_id: principal.principal_id,
            stage: "llm_output".into(),
            tool_risk: None,
        };
        match security
            .inspect(
                context,
                gateway_types::InspectionContent::ModelOutput(output.clone()),
            )
            .await
        {
            Ok((gateway_types::InspectionContent::ModelOutput(content), _)) => {
                let mut emitted = false;
                for event in events {
                    if !emitted && matches!(event, GatewayStreamEvent::Finished { .. }) {
                        let _ = sender
                            .send(Ok(GatewayStreamEvent::Delta {
                                content: content.clone(),
                            }))
                            .await;
                        emitted = true;
                    }
                    if sender.send(Ok(event)).await.is_err() {
                        status = UsageStatus::Interrupted;
                        break;
                    }
                }
                if !emitted && !content.is_empty() {
                    let _ = sender.send(Ok(GatewayStreamEvent::Delta { content })).await;
                }
            }
            Ok(_) => {}
            Err(error) => {
                let _ = sender.send(Err(GatewayError::Security(error))).await;
            }
        }
    }
    let usage = usage.unwrap_or(TokenUsage {
        prompt_tokens: reservation.prompt_tokens,
        completion_tokens: output.len() as u64,
        estimated: true,
    });
    let failed = record_usage(&metering, &reservation, usage, status)
        .await
        .is_err();
    quota.release_counter(reservation.reservation_id).await;
    if failed {
        let _ = sender.send(Err(GatewayError::Metering)).await;
    } else {
        log_completion(&reservation, status, started);
    }
}

fn log_completion(reservation: &QuotaReservation, status: UsageStatus, started: Instant) {
    tracing::info!(
        request_id = %reservation.request_id,
        provider = %reservation.provider,
        requested_model = %reservation.requested_model,
        upstream_model = %reservation.upstream_model,
        status = ?status,
        latency_ms = started.elapsed().as_millis(),
        "inference completed"
    );
}

fn canonical_request(request: &GatewayInferenceRequest, stream: bool) -> GatewayRequest {
    let mut messages = request
        .instructions
        .iter()
        .map(|instruction| gateway_types::GatewayMessage {
            role: match instruction.role {
                gateway_types::InstructionRole::System
                | gateway_types::InstructionRole::Developer => MessageRole::System,
            },
            content: instruction.content.clone(),
        })
        .collect::<Vec<_>>();
    messages.extend(request.messages.clone());
    GatewayRequest {
        model: request.requested_model.clone(),
        messages,
        stream,
        stream_include_usage: stream,
        generation: request.generation.clone(),
    }
}

async fn record_usage(
    metering: &MeteringService,
    reservation: &QuotaReservation,
    usage: TokenUsage,
    status: UsageStatus,
) -> Result<(), gateway_metering::MeteringError> {
    let estimated_cost = metering
        .cost_for(
            &reservation.provider,
            &reservation.upstream_model,
            usage.prompt_tokens,
            usage.completion_tokens,
        )
        .await;
    metering
        .finalize(
            reservation,
            UsageEvent {
                event_id: Uuid::now_v7(),
                request_id: reservation.request_id,
                tenant_id: reservation.tenant_id.clone(),
                project_id: reservation.project_id.clone(),
                principal_id: reservation.principal_id.clone(),
                user_id: reservation.user_id.clone(),
                provider: reservation.provider.clone(),
                requested_model: reservation.requested_model.clone(),
                upstream_model: reservation.upstream_model.clone(),
                usage,
                estimated_cost,
                status,
                occurred_at: Utc::now(),
            },
        )
        .await
}

/// Application failure mapped to OpenAI errors by the transport.
#[derive(Clone, Debug, Error)]
pub enum GatewayError {
    /// Unknown model alias.
    #[error(transparent)]
    Routing(#[from] RoutingError),
    /// Hierarchical policy denied the operation.
    #[error(transparent)]
    Policy(#[from] PolicyError),
    /// Security inspection denied or could not safely inspect the request.
    #[error(transparent)]
    Security(#[from] SecurityError),
    /// Quota failure.
    #[error(transparent)]
    Quota(#[from] QuotaError),
    /// Sanitized upstream failure.
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// Usage could not be durably recorded.
    #[error("usage metering unavailable")]
    Metering,
}
