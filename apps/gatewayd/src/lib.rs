use std::{sync::Arc, time::Duration};

use chrono::Utc;
use gateway_approval::{ApprovalRepository, ApprovalService};
use gateway_auth::{
    AuthAttemptLimiter, AuthEmailKind, AuthEmailQueue, AuthService, IdentityRepository,
    InvitationDelivery, JwtAuthenticator, RegistrationMode, VerificationEmailSender, WebAuthError,
    WebAuthService,
};
use gateway_billing::{BillingRepository, BillingWorker};
use gateway_config::Settings;
use gateway_core::{GatewayRuntime, GatewayService};
use gateway_entitlements::{
    CommunityEntitlements, Edition, EntitlementProvider, EntitlementStateRepository,
};
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
    GatewayStream, ModelProvider, ProviderContext, ProviderError, ProviderHealthMonitor,
    ProviderHealthRepository, ProviderRegistry,
};
use gateway_quota::QuotaService;
use gateway_routing::{RoutePlan, RouteTarget, StaticRouter};
use gateway_secrets::{SecretCipher, SecretRepository, SecretService};
use gateway_security::{SecurityEnforcer, SecurityInspector, SecurityPipeline, SecurityRepository};
use gateway_server::AppState;
use gateway_store::GatewayStore;
use gateway_types::{
    GatewayRequest, GatewayResponse, McpTransportType, SecretRef, TokenUsage, UsageEvent,
    UsageStatus,
};
use provider_anthropic::AnthropicProvider;
use provider_gemini::GeminiProvider;
use provider_openai_compatible::OpenAiCompatibleProvider;
use secrecy::{ExposeSecret, SecretString};
use sqlx::Row;
use store_postgres::PostgresStore;
use tokio::signal;
use uuid::Uuid;

#[derive(Clone)]
struct ResendEmailSender {
    client: reqwest::Client,
    api_key: SecretString,
    from: String,
    app_url: url::Url,
}

impl ResendEmailSender {
    async fn deliver(
        &self,
        event_id: Uuid,
        email: &str,
        subject: &str,
        path: &str,
        token: &str,
    ) -> Result<(), WebAuthError> {
        let mut target = self.app_url.clone();
        target.set_path(path);
        target.set_fragment(Some(&format!("token={token}")));
        let response = self
            .client
            .post("https://api.resend.com/emails")
            .bearer_auth(self.api_key.expose_secret())
            .header("Idempotency-Key", event_id.to_string())
            .json(&serde_json::json!({
                "from": self.from,
                "to": [email],
                "subject": subject,
                "html": format!("<p>{subject}</p><p><a href=\"{target}\">Continue</a></p>"),
            }))
            .send()
            .await
            .map_err(|_| WebAuthError::Unavailable)?;
        response
            .error_for_status()
            .map(|_| ())
            .map_err(|_| WebAuthError::Unavailable)
    }
}

#[async_trait::async_trait]
impl VerificationEmailSender for ResendEmailSender {
    async fn send_verification(
        &self,
        event_id: Uuid,
        email: &str,
        token: &str,
    ) -> Result<(), WebAuthError> {
        self.deliver(
            event_id,
            email,
            "Verify your Tuenel account",
            "/en/verify",
            token,
        )
        .await
    }

    async fn send_invitation(
        &self,
        event_id: Uuid,
        email: &str,
        token: &str,
    ) -> Result<(), WebAuthError> {
        self.deliver(
            event_id,
            email,
            "You are invited to Tuenel",
            "/en/invite",
            token,
        )
        .await
    }
}

#[derive(Clone)]
struct PostgresAuthEmailQueue {
    store: Arc<PostgresStore>,
    cipher: SecretCipher,
}

struct ClaimedAuthEmail {
    event_id: Uuid,
    kind: AuthEmailKind,
    recipient: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    attempt_count: i32,
}

impl PostgresAuthEmailQueue {
    fn aad(event_id: Uuid, kind: AuthEmailKind, recipient: &str) -> String {
        format!("auth-email:{event_id}:{}:{recipient}", kind.as_str())
    }

    async fn claim(&self) -> Result<Option<ClaimedAuthEmail>, WebAuthError> {
        let row = sqlx::query(
            "UPDATE auth_email_outbox SET attempt_count=attempt_count+1,
                    next_attempt_at=now()+interval '5 minutes'
             WHERE event_id=(
                SELECT event_id FROM auth_email_outbox
                WHERE delivered_at IS NULL AND next_attempt_at<=now() AND attempt_count<10
                ORDER BY next_attempt_at,created_at
                FOR UPDATE SKIP LOCKED LIMIT 1
             )
             RETURNING event_id,delivery_kind,recipient,nonce,ciphertext,attempt_count",
        )
        .fetch_optional(self.store.pool())
        .await
        .map_err(|_| WebAuthError::Unavailable)?;
        row.map(|row| {
            let kind = match row
                .try_get::<String, _>("delivery_kind")
                .map_err(|_| WebAuthError::Unavailable)?
                .as_str()
            {
                "verification" => AuthEmailKind::Verification,
                "invitation" => AuthEmailKind::Invitation,
                _ => return Err(WebAuthError::Unavailable),
            };
            Ok(ClaimedAuthEmail {
                event_id: row
                    .try_get("event_id")
                    .map_err(|_| WebAuthError::Unavailable)?,
                kind,
                recipient: row
                    .try_get("recipient")
                    .map_err(|_| WebAuthError::Unavailable)?,
                nonce: row
                    .try_get("nonce")
                    .map_err(|_| WebAuthError::Unavailable)?,
                ciphertext: row
                    .try_get("ciphertext")
                    .map_err(|_| WebAuthError::Unavailable)?,
                attempt_count: row
                    .try_get("attempt_count")
                    .map_err(|_| WebAuthError::Unavailable)?,
            })
        })
        .transpose()
    }

    async fn delivered(&self, event_id: Uuid) -> Result<(), WebAuthError> {
        sqlx::query(
            "UPDATE auth_email_outbox
             SET delivered_at=now(),nonce=$2,ciphertext=$2,last_error_code=NULL
             WHERE event_id=$1",
        )
        .bind(event_id)
        .bind(Vec::<u8>::new())
        .execute(self.store.pool())
        .await
        .map(|_| ())
        .map_err(|_| WebAuthError::Unavailable)
    }

    async fn failed(&self, event: &ClaimedAuthEmail) -> Result<(), WebAuthError> {
        let delay = 2_i64.pow(u32::try_from(event.attempt_count.min(10)).unwrap_or(10));
        sqlx::query(
            "UPDATE auth_email_outbox
             SET next_attempt_at=now()+make_interval(secs=>$2::double precision),
                    last_error_code='provider_unavailable'
             WHERE event_id=$1",
        )
        .bind(event.event_id)
        .bind(delay)
        .execute(self.store.pool())
        .await
        .map(|_| ())
        .map_err(|_| WebAuthError::Unavailable)
    }
}

#[async_trait::async_trait]
impl AuthEmailQueue for PostgresAuthEmailQueue {
    async fn enqueue(
        &self,
        kind: AuthEmailKind,
        email: &str,
        token: &str,
    ) -> Result<(), WebAuthError> {
        let event_id = Uuid::now_v7();
        let aad = Self::aad(event_id, kind, email);
        let encrypted = self
            .cipher
            .seal(aad.as_bytes(), token.as_bytes())
            .map_err(|_| WebAuthError::Unavailable)?;
        sqlx::query(
            "INSERT INTO auth_email_outbox
             (event_id,delivery_kind,recipient,nonce,ciphertext)
             VALUES($1,$2,$3,$4,$5)",
        )
        .bind(event_id)
        .bind(kind.as_str())
        .bind(email)
        .bind(encrypted.nonce)
        .bind(encrypted.ciphertext)
        .execute(self.store.pool())
        .await
        .map(|_| ())
        .map_err(|_| WebAuthError::Unavailable)
    }
}

fn spawn_auth_email_worker(
    queue: Arc<PostgresAuthEmailQueue>,
    sender: Arc<dyn VerificationEmailSender>,
) {
    tokio::spawn(async move {
        loop {
            match queue.claim().await {
                Ok(Some(event)) => {
                    let aad =
                        PostgresAuthEmailQueue::aad(event.event_id, event.kind, &event.recipient);
                    let delivery = queue
                        .cipher
                        .open(aad.as_bytes(), &event.nonce, &event.ciphertext)
                        .ok()
                        .and_then(|value| String::from_utf8(value).ok());
                    let result = match delivery {
                        Some(token) if event.kind == AuthEmailKind::Verification => {
                            sender
                                .send_verification(event.event_id, &event.recipient, &token)
                                .await
                        }
                        Some(token) => {
                            sender
                                .send_invitation(event.event_id, &event.recipient, &token)
                                .await
                        }
                        None => Err(WebAuthError::Unavailable),
                    };
                    if result.is_ok() {
                        let _ = queue.delivered(event.event_id).await;
                        tracing::info!(
                            event_id = %event.event_id,
                            delivery_kind = event.kind.as_str(),
                            "authentication email delivered"
                        );
                    } else {
                        let _ = queue.failed(&event).await;
                        tracing::warn!(
                            event_id = %event.event_id,
                            delivery_kind = event.kind.as_str(),
                            attempt = event.attempt_count,
                            "authentication email delivery failed; retry scheduled"
                        );
                    }
                }
                Ok(None) => tokio::time::sleep(Duration::from_secs(2)).await,
                Err(_) => {
                    tracing::warn!("authentication email outbox unavailable");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });
}

/// Runtime customization point used by separately distributed editions.
pub struct RuntimeOptions {
    pub edition: Edition,
    pub entitlements: Arc<dyn EntitlementProvider>,
    pub extend_router:
        Option<Arc<dyn Fn(axum::Router, AppState) -> axum::Router + Send + Sync + 'static>>,
}

impl RuntimeOptions {
    /// Fully offline Community defaults.
    pub fn community() -> Self {
        Self {
            edition: Edition::Community,
            entitlements: Arc::new(CommunityEntitlements),
            extend_router: None,
        }
    }
}

/// Start the gateway with provider-neutral runtime extensions.
pub async fn run(options: RuntimeOptions) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        println!(
            "{}",
            serde_json::to_string_pretty(&gateway_server::openapi_document())?
        );
        return Ok(());
    }

    gateway_observability::init()?;
    let settings = Settings::from_env()?;
    if let Some(warning) = settings.development_secret_warning() {
        tracing::warn!("{warning}");
    }
    let postgres = Arc::new(PostgresStore::connect(settings.database_url.expose_secret()).await?);
    tracing::info!("PostgreSQL connected and migrations applied");
    let installation_id = EntitlementStateRepository::installation_id(postgres.as_ref()).await?;
    if !IdentityRepository::installation_initialized(postgres.as_ref()).await?
        && settings.bootstrap_token_hash.is_none()
    {
        return Err(
            "AUTH_BOOTSTRAP_TOKEN_HASH is required for an uninitialized installation".into(),
        );
    }
    let store: Arc<dyn GatewayStore> = postgres.clone();
    let keys = VirtualKeyService::new(store.clone());
    let jwt = match (
        settings.oidc_issuer.clone(),
        settings.oidc_audience.clone(),
        settings.oidc_jwks_url.clone(),
    ) {
        (Some(issuer), Some(audience), Some(jwks_url)) => Some(
            JwtAuthenticator::new(issuer, audience, jwks_url.to_string(), store.clone()).await?,
        ),
        _ => {
            tracing::info!(
                "OIDC is not configured; local sessions and virtual keys remain enabled"
            );
            None
        }
    };
    let verification_sender = match (
        settings.resend_api_key.clone(),
        settings.resend_from.clone(),
        settings.app_url.clone(),
    ) {
        (Some(api_key), Some(from), Some(app_url)) => Some(Arc::new(ResendEmailSender {
            client: reqwest::Client::new(),
            api_key,
            from,
            app_url,
        })
            as Arc<dyn VerificationEmailSender>),
        _ => None,
    };
    let email_queue = if let Some(sender) = verification_sender.clone() {
        let queue = Arc::new(PostgresAuthEmailQueue {
            store: postgres.clone(),
            cipher: SecretCipher::new(settings.credentials_master_key.expose_secret())?,
        });
        spawn_auth_email_worker(queue.clone(), sender);
        Some(queue as Arc<dyn AuthEmailQueue>)
    } else {
        None
    };
    let web_auth = WebAuthService::new(
        postgres.clone() as Arc<dyn IdentityRepository>,
        Duration::from_secs(12 * 60 * 60),
    )
    .with_registration(
        settings.deployment_mode.clone(),
        match settings.registration_mode.as_str() {
            "public" => RegistrationMode::Public,
            "closed" => RegistrationMode::Closed,
            _ => RegistrationMode::InviteOnly,
        },
        settings.bootstrap_token_hash.clone(),
        match settings.invitation_delivery.as_str() {
            "email" => InvitationDelivery::Email,
            _ => InvitationDelivery::Manual,
        },
    );
    let web_auth = email_queue.map_or(web_auth.clone(), |queue| {
        web_auth.clone().with_email_queue(queue)
    });
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
    let web_auth = web_auth.with_attempt_limiter(
        redis.clone() as Arc<dyn AuthAttemptLimiter>,
        settings
            .credentials_master_key
            .expose_secret()
            .as_bytes()
            .to_vec(),
    );
    let authenticator = Arc::new(AuthService::new(jwt, keys.clone()).with_web(web_auth.clone()));
    let quota =
        QuotaService::new(store.clone(), settings.reservation_ttl).with_counter(redis.clone());
    let mut providers: Vec<Arc<dyn ModelProvider>> = Vec::new();
    let mut targets = Vec::new();
    if let (Some(base_url), Some(model)) = (
        settings.upstream_base_url.clone(),
        settings.upstream_model.clone(),
    ) {
        postgres
            .upsert_provider("openai-compatible", "openai_compatible", base_url.as_str())
            .await?;
        providers.push(Arc::new(OpenAiCompatibleProvider::new(
            "openai-compatible".into(),
            base_url,
            settings.upstream_api_key.clone(),
            settings.request_timeout,
        )?));
        targets.push(RouteTarget {
            tenant_id: None,
            project_id: None,
            provider: "openai-compatible".into(),
            requested_model: settings.model_alias.clone(),
            upstream_model: model,
            priority: 1,
            enabled: true,
        });
    }
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
            tenant_id: None,
            project_id: None,
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
            tenant_id: None,
            project_id: None,
            provider: "gemini".into(),
            requested_model: settings.model_alias.clone(),
            upstream_model: model,
            priority: 3,
            enabled: true,
        });
    }
    for provider in &providers {
        let id = provider.id();
        let (provider_type, base_url) = match id {
            "anthropic" => ("anthropic", settings.anthropic_base_url.as_str()),
            "gemini" => ("gemini", settings.gemini_base_url.as_str()),
            _ => (
                "openai_compatible",
                settings
                    .upstream_base_url
                    .as_ref()
                    .expect("OpenAI-compatible provider has a base URL")
                    .as_str(),
            ),
        };
        postgres
            .bootstrap_runtime_resource(
                gateway_admin::ResourceKind::Providers,
                id,
                serde_json::json!({
                    "provider_type":provider_type,
                    "base_url":base_url,
                    "environment_credential":true
                }),
            )
            .await?;
    }
    for target in &targets {
        postgres
            .bootstrap_runtime_resource(
                gateway_admin::ResourceKind::ModelRoutes,
                &format!("{}:{}", target.requested_model, target.provider),
                serde_json::json!({
                    "provider":target.provider,
                    "requested_model":target.requested_model,
                    "upstream_model":target.upstream_model,
                    "priority":target.priority,
                    "enabled":target.enabled
                }),
            )
            .await?;
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
    let service = if targets.is_empty() {
        tracing::info!("no model provider configured; complete setup in the web console");
        setup_gateway(
            &settings,
            policy.clone(),
            quota.clone(),
            metering.clone(),
            security_enforcer.clone(),
        )?
    } else {
        GatewayService::configured(
            StaticRouter::new(
                targets[0].provider.clone(),
                targets[0].requested_model.clone(),
                targets[0].upstream_model.clone(),
            ),
            RoutePlan::new(targets)?,
            provider_registry,
            policy.clone(),
            quota.clone(),
            metering.clone(),
            settings.request_timeout,
        )?
        .with_security(security_enforcer.clone())
    };
    let gateway = GatewayRuntime::new(service);
    tokio::spawn(reconcile_expired(store.clone(), metering.clone()));

    let secret_repository: Arc<dyn SecretRepository> = postgres.clone();
    let secrets = SecretService::new(
        secret_repository.clone(),
        settings.credentials_master_key.expose_secret(),
    )?;
    tokio::spawn(reconcile_runtime(
        postgres.clone(),
        gateway.clone(),
        settings.clone(),
        secrets.clone(),
        policy.clone(),
        quota.clone(),
        metering.clone(),
        security_enforcer.clone(),
    ));
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

    let state = AppState {
        authenticator,
        web_auth: Some(web_auth),
        gateway,
        keys,
        store,
        provider_health: Some(postgres.clone() as Arc<dyn ProviderHealthRepository>),
        admin_role: settings.oidc_admin_role.clone(),
        max_output_tokens: settings.max_output_tokens,
        default_output_tokens: settings.default_output_tokens,
        default_virtual_key_daily_tokens: settings.default_virtual_key_daily_tokens,
        max_body_bytes: settings.max_body_bytes,
        mcp_registry,
        mcp_invocations,
        mcp_policies: Some(postgres.clone() as Arc<dyn McpPolicyRepository>),
        mcp_policy_admin: Some(postgres.clone() as Arc<dyn McpPolicyAdministration>),
        secrets: Some(secrets.clone()),
        approvals: settings.approval_enabled.then_some(approvals),
        incidents: Some(incidents),
        security_repository: Some(security_repository),
        audit: Some(audit),
        admin: Some(
            gateway_admin::AdminService::new(
                postgres.clone() as Arc<dyn gateway_admin::AdminRepository>,
                settings.oidc_admin_role.clone(),
            )
            .with_secrets(secrets),
        ),
        deployment_mode: settings.deployment_mode.clone(),
        edition: options.edition,
        installation_id,
        entitlements: options.entitlements,
    };
    let app = gateway_server::router(state.clone());
    let app = match options.extend_router {
        Some(extension) => extension(app, state),
        None => app,
    };
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

#[allow(clippy::too_many_arguments)]
async fn reconcile_runtime(
    postgres: Arc<PostgresStore>,
    runtime: GatewayRuntime,
    settings: Settings,
    secrets: SecretService,
    policy: Arc<dyn PolicyResolver>,
    quota: QuotaService,
    metering: MeteringService,
    security: SecurityEnforcer,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        match build_runtime(
            &postgres,
            &settings,
            &secrets,
            policy.clone(),
            quota.clone(),
            metering.clone(),
            security.clone(),
        )
        .await
        {
            Ok(candidate) => runtime.replace(candidate),
            Err(error) => {
                tracing::warn!(reason = %error, "runtime configuration reconciliation failed");
                runtime.reconciliation_failed(error);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn build_runtime(
    postgres: &PostgresStore,
    settings: &Settings,
    secrets: &SecretService,
    policy: Arc<dyn PolicyResolver>,
    quota: QuotaService,
    metering: MeteringService,
    security: SecurityEnforcer,
) -> Result<GatewayService, String> {
    let resources = postgres
        .runtime_resources()
        .await
        .map_err(|_| "configuration repository unavailable".to_owned())?;
    let mut providers: Vec<Arc<dyn ModelProvider>> = Vec::new();
    let mut targets = Vec::new();
    for resource in resources {
        match resource.kind.as_str() {
            "providers" => {
                let provider_type = text(&resource.body, "provider_type")?;
                let base_url: url::Url = text(&resource.body, "base_url")?
                    .parse()
                    .map_err(|_| "invalid provider base URL".to_owned())?;
                let credential =
                    runtime_credential(&resource.id, &resource.body, settings, secrets).await?;
                let provider: Arc<dyn ModelProvider> = match provider_type {
                    "anthropic" => Arc::new(
                        AnthropicProvider::new(
                            resource.id,
                            base_url,
                            credential
                                .ok_or_else(|| "anthropic credential is missing".to_owned())?,
                            settings.request_timeout,
                        )
                        .map_err(|error| error.to_string())?,
                    ),
                    "gemini" => Arc::new(
                        GeminiProvider::new(
                            resource.id,
                            base_url,
                            credential.ok_or_else(|| "gemini credential is missing".to_owned())?,
                            settings.request_timeout,
                        )
                        .map_err(|error| error.to_string())?,
                    ),
                    "openai_compatible" => Arc::new(
                        OpenAiCompatibleProvider::new(
                            resource.id,
                            base_url,
                            credential,
                            settings.request_timeout,
                        )
                        .map_err(|error| error.to_string())?,
                    ),
                    _ => return Err("unsupported provider type".into()),
                };
                providers.push(provider);
            }
            "model_routes" => {
                let priority = resource
                    .body
                    .get("priority")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| value.try_into().ok())
                    .ok_or_else(|| "invalid route priority".to_owned())?;
                targets.push(RouteTarget {
                    tenant_id: resource.tenant_id.clone(),
                    project_id: resource
                        .body
                        .get("project_id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    provider: text(&resource.body, "provider")?.to_owned(),
                    requested_model: text(&resource.body, "requested_model")?.to_owned(),
                    upstream_model: text(&resource.body, "upstream_model")?.to_owned(),
                    priority,
                    enabled: resource
                        .body
                        .get("enabled")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true),
                });
            }
            _ => {}
        }
    }
    targets.sort_by_key(|target| target.priority);
    if targets.is_empty() {
        return setup_gateway(settings, policy, quota, metering, security)
            .map_err(|error| error.to_string());
    }
    let first = targets
        .iter()
        .find(|target| target.enabled)
        .cloned()
        .ok_or_else(|| "no enabled model route".to_owned())?;
    let registry = ProviderRegistry::new(providers);
    if targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| registry.get(&target.provider).is_none())
    {
        return Err("route references an unavailable provider".into());
    }
    GatewayService::configured(
        StaticRouter::new(first.provider, first.requested_model, first.upstream_model),
        RoutePlan::new(targets).map_err(|error| error.to_string())?,
        registry,
        policy,
        quota,
        metering,
        settings.request_timeout,
    )
    .map(|gateway| gateway.with_security(security))
    .map_err(|error| error.to_string())
}

async fn runtime_credential(
    provider_id: &str,
    body: &serde_json::Value,
    settings: &Settings,
    secrets: &SecretService,
) -> Result<Option<SecretString>, String> {
    if let Some(secret_ref) = body.get("secret_ref").and_then(serde_json::Value::as_str) {
        let tenant_id = text(body, "secret_tenant_id")?;
        let value = secrets
            .expose(tenant_id, &SecretRef(secret_ref.to_owned()))
            .await
            .map_err(|_| "provider credential is unavailable".to_owned())?;
        return String::from_utf8(value.expose().to_vec())
            .map(SecretString::from)
            .map(Some)
            .map_err(|_| "provider credential is invalid".to_owned());
    }
    Ok(match provider_id {
        "anthropic" => settings.anthropic_api_key.clone(),
        "gemini" => settings.gemini_api_key.clone(),
        "openai-compatible" => settings.upstream_api_key.clone(),
        _ => None,
    })
}

fn setup_gateway(
    settings: &Settings,
    policy: Arc<dyn PolicyResolver>,
    quota: QuotaService,
    metering: MeteringService,
    security: SecurityEnforcer,
) -> Result<GatewayService, gateway_core::GatewayError> {
    let provider: Arc<dyn ModelProvider> = Arc::new(SetupProvider);
    let target = RouteTarget {
        tenant_id: None,
        project_id: None,
        provider: provider.id().to_owned(),
        requested_model: settings.model_alias.clone(),
        upstream_model: "unconfigured".into(),
        priority: 1,
        enabled: true,
    };
    GatewayService::configured(
        StaticRouter::new(
            target.provider.clone(),
            target.requested_model.clone(),
            target.upstream_model.clone(),
        ),
        RoutePlan::new(vec![target])?,
        ProviderRegistry::new([provider]),
        policy,
        quota,
        metering,
        settings.request_timeout,
    )
    .map(|gateway| gateway.with_security(security))
}

struct SetupProvider;

#[async_trait::async_trait]
impl ModelProvider for SetupProvider {
    fn id(&self) -> &str {
        "__setup__"
    }

    async fn execute(
        &self,
        _context: ProviderContext,
        _request: GatewayRequest,
    ) -> Result<GatewayResponse, ProviderError> {
        Err(ProviderError::Unavailable)
    }

    async fn stream(
        &self,
        _context: ProviderContext,
        _request: GatewayRequest,
    ) -> Result<GatewayStream, ProviderError> {
        Err(ProviderError::Unavailable)
    }
}

fn text<'a>(body: &'a serde_json::Value, field: &str) -> Result<&'a str, String> {
    body.get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing {field}"))
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
                latency_ms: None,
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
