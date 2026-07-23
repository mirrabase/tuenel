use std::{sync::Arc, time::Duration};

use chrono::Utc;
use gateway_approval::{ApprovalRepository, ApprovalService};
use gateway_auth::{AuthService, IdentityRepository, JwtAuthenticator, WebAuthService};
use gateway_billing::{BillingRepository, BillingWorker};
use gateway_config::Settings;
use gateway_core::GatewayService;
use gateway_events::{AuditRepository, AuditService};
use gateway_incidents::{IncidentRepository, IncidentService};
use gateway_keys::VirtualKeyService;
use gateway_mcp::{
    McpConnectionContext, McpError, McpPolicyAdministration, McpPolicyRepository, McpRepository,
    McpTransport, McpTransportResolver, McpUsageRepository, SchemaLimits, SecretValue, ToolCache,
};
use gateway_metering::{MeteringService, Pricing};
use gateway_policy::PolicyResolver;
use gateway_pricing::PricingCatalog;
use gateway_providers::{
    ModelProvider, ProviderHealthMonitor, ProviderHealthRepository, ProviderRegistry,
};
use gateway_quota::QuotaService;
use gateway_routing::{RoutePlan, RouteTarget, StaticRouter};
use gateway_secrets::{SecretRepository, SecretService};
use gateway_security::{SecurityEnforcer, SecurityInspector, SecurityPipeline, SecurityRepository};
use gateway_server::AppState;
use gateway_store::GatewayStore;
use gateway_types::{McpTransportType, TokenUsage, UsageEvent, UsageStatus};
use provider_anthropic::AnthropicProvider;
use provider_gemini::GeminiProvider;
use provider_openai_compatible::OpenAiCompatibleProvider;
use secrecy::ExposeSecret;
use store_postgres::PostgresStore;
use tokio::signal;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        println!(
            "{}",
            serde_json::to_string_pretty(&gateway_server::openapi_document())?
        );
        return Ok(());
    }

    gateway_observability::init()?;
    let settings = Settings::from_env()?;
    let postgres = Arc::new(PostgresStore::connect(settings.database_url.expose_secret()).await?);
    let store: Arc<dyn GatewayStore> = postgres.clone();
    let keys = VirtualKeyService::new(store.clone());
    let jwt = JwtAuthenticator::new(
        settings.oidc_issuer.clone(),
        settings.oidc_audience.clone(),
        settings.oidc_jwks_url.to_string(),
        store.clone(),
    )
    .await?;
    let web_auth = WebAuthService::new(
        postgres.clone() as Arc<dyn IdentityRepository>,
        Duration::from_secs(12 * 60 * 60),
    );
    let authenticator = Arc::new(AuthService::new(jwt, keys.clone()).with_web(web_auth.clone()));
    let openai: Arc<dyn ModelProvider> = Arc::new(OpenAiCompatibleProvider::new(
        "openai-compatible".into(),
        settings.upstream_base_url.clone(),
        settings.upstream_api_key.clone(),
        settings.request_timeout,
    )?);
    postgres
        .upsert_provider(
            "openai-compatible",
            "openai_compatible",
            settings.upstream_base_url.as_str(),
        )
        .await?;
    let router = StaticRouter::new(
        "openai-compatible".into(),
        settings.model_alias.clone(),
        settings.upstream_model.clone(),
    );
    let pricing = Pricing {
        input_per_million: settings.input_cost_per_million,
        output_per_million: settings.output_cost_per_million,
    };
    let metering = MeteringService::new(store.clone(), pricing)
        .with_catalog(postgres.clone() as Arc<dyn PricingCatalog>);
    let redis = Arc::new(store_redis::RedisQuotaStore::new(
        settings.redis_url.expose_secret(),
        16,
    )?);
    redis.ping().await?;
    let quota =
        QuotaService::new(store.clone(), settings.reservation_ttl).with_counter(redis.clone());
    let mut providers = vec![openai];
    let mut targets = vec![RouteTarget {
        provider: "openai-compatible".into(),
        requested_model: settings.model_alias.clone(),
        upstream_model: settings.upstream_model.clone(),
        priority: 1,
        enabled: true,
    }];
    if let (Some(key), Some(model)) = (
        settings.anthropic_api_key.clone(),
        settings.anthropic_model.clone(),
    ) {
        postgres
            .upsert_provider(
                "anthropic",
                "anthropic",
                settings.anthropic_base_url.as_str(),
            )
            .await?;
        providers.push(Arc::new(AnthropicProvider::new(
            "anthropic".into(),
            settings.anthropic_base_url.clone(),
            key,
            settings.request_timeout,
        )?));
        targets.push(RouteTarget {
            provider: "anthropic".into(),
            requested_model: settings.model_alias.clone(),
            upstream_model: model,
            priority: 2,
            enabled: true,
        });
    }
    if let (Some(key), Some(model)) = (
        settings.gemini_api_key.clone(),
        settings.gemini_model.clone(),
    ) {
        postgres
            .upsert_provider("gemini", "gemini", settings.gemini_base_url.as_str())
            .await?;
        providers.push(Arc::new(GeminiProvider::new(
            "gemini".into(),
            settings.gemini_base_url.clone(),
            key,
            settings.request_timeout,
        )?));
        targets.push(RouteTarget {
            provider: "gemini".into(),
            requested_model: settings.model_alias.clone(),
            upstream_model: model,
            priority: 3,
            enabled: true,
        });
    }
    let provider_registry = ProviderRegistry::new(providers);
    tokio::spawn(
        ProviderHealthMonitor::new(
            provider_registry.clone(),
            postgres.clone() as Arc<dyn ProviderHealthRepository>,
            Duration::from_secs(30),
            Duration::from_secs(5),
        )
        .run(),
    );
    let policy: Arc<dyn PolicyResolver> = postgres.clone();
    let incidents = IncidentService::new(postgres.clone() as Arc<dyn IncidentRepository>);
    let security_repository: Arc<dyn SecurityRepository> = postgres.clone();
    let inspectors: Vec<Arc<dyn SecurityInspector>> = vec![
        Arc::new(security_secrets::SecretInspector::new(&[])?),
        Arc::new(security_sensitive_data::SensitiveDataInspector::new(&[])?),
        Arc::new(security_prompt_injection::PromptInjectionInspector::new()?),
        Arc::new(gateway_security::CustomPatternInspector::new(
            security_repository.clone(),
        )),
    ];
    let security_pipeline = SecurityPipeline::new(inspectors);
    let security_enforcer = SecurityEnforcer::new(
        security_pipeline,
        security_repository.clone(),
        incidents.clone(),
    )
    .with_enabled(settings.security_enabled);
    let gateway = Arc::new(
        GatewayService::configured(
            router,
            RoutePlan::new(targets)?,
            provider_registry,
            policy,
            quota,
            metering.clone(),
            settings.request_timeout,
        )?
        .with_security(security_enforcer.clone()),
    );
    tokio::spawn(reconcile_expired(store.clone(), metering));

    let secret_repository: Arc<dyn SecretRepository> = postgres.clone();
    let secrets = SecretService::new(
        secret_repository.clone(),
        settings.credentials_master_key.expose_secret(),
    )?;
    let billing = BillingWorker::new(
        postgres.clone() as Arc<dyn BillingRepository>,
        secrets.clone(),
        settings.request_timeout,
    )?;
    tokio::spawn(billing.run());
    let approvals = ApprovalService::new(
        postgres.clone() as Arc<dyn ApprovalRepository>,
        settings.approval_expiration,
    );
    let audit = AuditService::new(postgres.clone() as Arc<dyn AuditRepository>);
    let (mcp_registry, mcp_invocations) = if settings.mcp_enabled {
        let resolver = Arc::new(TransportResolver {
            secrets: secrets.clone(),
            secret_repository,
            allow_private: settings.mcp_allow_private_http_endpoints,
            allowed_commands: settings.mcp_allowed_stdio_commands.clone(),
            timeout: settings.mcp_tool_timeout,
            maximum_response_bytes: settings.mcp_maximum_response_bytes,
        });
        let repository: Arc<dyn McpRepository> = postgres.clone();
        let registry = gateway_mcp::McpRegistry::new(
            repository,
            resolver,
            ToolCache::new(settings.mcp_discovery_cache),
            SchemaLimits {
                maximum_bytes: settings.mcp_maximum_schema_bytes,
                ..SchemaLimits::default()
            },
        );
        let policies: Arc<dyn McpPolicyRepository> = postgres.clone();
        let usage: Arc<dyn McpUsageRepository> = postgres.clone();
        let invocations = gateway_mcp::McpInvocationService::new(
            registry.clone(),
            policies,
            redis,
            usage,
            approvals.clone(),
            settings.approval_enabled,
            audit.clone(),
            security_enforcer.clone(),
        );
        (Some(registry), Some(invocations))
    } else {
        (None, None)
    };

    let app = gateway_server::router(AppState {
        authenticator,
        web_auth: Some(web_auth),
        gateway,
        keys,
        store,
        admin_role: settings.oidc_admin_role,
        max_output_tokens: settings.max_output_tokens,
        default_output_tokens: settings.default_output_tokens,
        default_virtual_key_daily_tokens: settings.default_virtual_key_daily_tokens,
        max_body_bytes: settings.max_body_bytes,
        mcp_registry,
        mcp_invocations,
        mcp_policies: Some(postgres.clone() as Arc<dyn McpPolicyRepository>),
        mcp_policy_admin: Some(postgres.clone() as Arc<dyn McpPolicyAdministration>),
        secrets: Some(secrets),
        approvals: settings.approval_enabled.then_some(approvals),
        incidents: Some(incidents),
        security_repository: Some(security_repository),
        audit: Some(audit),
    });
    let listener = tokio::net::TcpListener::bind(settings.bind_addr).await?;
    tracing::info!(address = %settings.bind_addr, "gateway listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

#[derive(Clone)]
struct TransportResolver {
    secrets: SecretService,
    secret_repository: Arc<dyn SecretRepository>,
    allow_private: bool,
    allowed_commands: Vec<String>,
    timeout: Duration,
    maximum_response_bytes: usize,
}

#[async_trait::async_trait]
impl McpTransportResolver for TransportResolver {
    async fn validate(&self, server: &gateway_mcp::McpServerRecord) -> Result<(), McpError> {
        match server.transport_type {
            McpTransportType::Stdio => mcp_transport_stdio::validate_command(
                server.command.as_deref().ok_or(McpError::Invalid)?,
                &self.allowed_commands,
            ),
            McpTransportType::StreamableHttp => {
                let endpoint = server
                    .endpoint
                    .as_deref()
                    .ok_or(McpError::Invalid)?
                    .parse()
                    .map_err(|_| McpError::Invalid)?;
                mcp_transport_http::validate_endpoint(&endpoint, self.allow_private).await
            }
        }
    }
    async fn resolve(
        &self,
        server: &gateway_mcp::McpServerRecord,
    ) -> Result<(Arc<dyn McpTransport>, McpConnectionContext), McpError> {
        self.validate(server).await?;
        let credential = match &server.credential_ref {
            Some(secret_ref) => {
                let value = self
                    .secrets
                    .expose(&server.tenant_id, secret_ref)
                    .await
                    .map_err(|_| McpError::Unavailable)?;
                Some(SecretValue::new(
                    String::from_utf8(value.expose().to_vec()).map_err(|_| McpError::Invalid)?,
                ))
            }
            None => None,
        };
        let mut environment = Vec::new();
        for secret_ref in &server.environment_secret_refs {
            let record = self
                .secret_repository
                .secret(&server.tenant_id, secret_ref)
                .await
                .map_err(|_| McpError::Unavailable)?
                .ok_or(McpError::Unavailable)?;
            if !record.purpose.ends_with(":environment") {
                return Err(McpError::Invalid);
            }
            let value = self
                .secrets
                .expose(&server.tenant_id, secret_ref)
                .await
                .map_err(|_| McpError::Unavailable)?;
            let decoded: serde_json::Value =
                serde_json::from_slice(value.expose()).map_err(|_| McpError::Invalid)?;
            let name = decoded
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or(McpError::Invalid)?
                .to_owned();
            let value = decoded
                .get("value")
                .and_then(serde_json::Value::as_str)
                .ok_or(McpError::Invalid)?
                .to_owned();
            environment.push((name, SecretValue::new(value)));
        }
        let transport: Arc<dyn McpTransport> = match server.transport_type {
            McpTransportType::Stdio => Arc::new(mcp_transport_stdio::StdioMcpTransport::new(
                self.allowed_commands.clone(),
                self.timeout,
                self.maximum_response_bytes,
            )),
            McpTransportType::StreamableHttp => {
                Arc::new(mcp_transport_http::HttpMcpTransport::new(
                    self.timeout,
                    self.allow_private,
                    self.maximum_response_bytes,
                )?)
            }
        };
        Ok((
            transport,
            McpConnectionContext {
                server: server.clone(),
                credential,
                environment,
            },
        ))
    }
}

async fn reconcile_expired(store: Arc<dyn GatewayStore>, metering: MeteringService) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        let Ok(reservations) = store.expired_reservations(Utc::now()).await else {
            tracing::warn!("failed to load expired quota reservations");
            continue;
        };
        for reservation in reservations {
            // ponytail: charge the conservative reservation after a crash; add a durable
            // provider receipt protocol if exact crash-time usage becomes available.
            let usage = TokenUsage {
                prompt_tokens: reservation.prompt_tokens,
                completion_tokens: reservation.completion_tokens,
                estimated: true,
            };
            let event = UsageEvent {
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
                estimated_cost: metering
                    .cost_for(
                        &reservation.provider,
                        &reservation.upstream_model,
                        usage.prompt_tokens,
                        usage.completion_tokens,
                    )
                    .await,
                status: UsageStatus::Interrupted,
                occurred_at: Utc::now(),
            };
            if metering.finalize(&reservation, event).await.is_err() {
                tracing::warn!(request_id = %reservation.request_id, "failed to reconcile expired reservation");
            }
        }
    }
}

async fn shutdown_signal() {
    if signal::ctrl_c().await.is_err() {
        tracing::error!("failed to install shutdown signal handler");
    }
}
