//! Stable, transport-neutral gateway domain types.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How a principal authenticated with the gateway.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationMethod {
    /// A JWT issued by the configured OIDC issuer.
    Jwt,
    /// A gateway-owned browser session presented by the web BFF.
    WebSession,
    /// A gateway-issued Virtual Key.
    VirtualKey,
}

/// Identity used by every operation after authentication.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Principal {
    /// Stable identity for authorization and metering.
    pub principal_id: String,
    /// Tenant that owns the request.
    pub tenant_id: String,
    /// Optional project binding.
    pub project_id: Option<String>,
    /// Optional end-user identity.
    pub user_id: Option<String>,
    /// Roles asserted by the authentication method.
    pub roles: Vec<String>,
    /// Scopes granted to the principal.
    pub scopes: Vec<String>,
    /// Authentication method used for this request.
    pub authentication_method: AuthenticationMethod,
    /// Virtual Key identifier when applicable.
    pub virtual_key_id: Option<Uuid>,
}

/// Canonical chat message role.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System instruction.
    System,
    /// User message.
    User,
    /// Assistant message.
    Assistant,
}

/// Canonical text-only chat message for v0.1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayMessage {
    /// Message author role.
    pub role: MessageRole,
    /// Text content.
    pub content: String,
}

/// Provider-neutral generation settings supported in v0.1.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationParameters {
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// Nucleus sampling threshold.
    pub top_p: Option<f32>,
    /// Maximum generated tokens.
    pub max_output_tokens: u32,
    /// Stop sequences.
    pub stop: Vec<String>,
}

/// Canonical provider-neutral inference request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayRequest {
    /// Public model alias requested by the client.
    pub model: String,
    /// Ordered conversation messages.
    pub messages: Vec<GatewayMessage>,
    /// Whether the response must stream.
    pub stream: bool,
    /// Whether an SSE usage chunk was requested.
    pub stream_include_usage: bool,
    /// Supported generation settings.
    pub generation: GenerationParameters,
}

/// Canonical operation-independent inference request used by v0.2 adapters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayInferenceRequest {
    /// Public model alias.
    pub requested_model: String,
    /// System and developer instructions.
    #[serde(default)]
    pub instructions: Vec<GatewayInstruction>,
    /// Conversation messages.
    #[serde(default)]
    pub messages: Vec<GatewayMessage>,
    /// Tool definitions.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    /// Requested tool selection.
    pub tool_choice: Option<ToolChoice>,
    /// Structured output preference.
    pub response_format: Option<ResponseFormat>,
    /// Generation settings.
    pub generation: GenerationParameters,
    /// Whether to stream the response.
    pub stream: bool,
    /// Request metadata that may be forwarded by adapters.
    #[serde(default)]
    pub metadata: RequestMetadata,
}

impl GatewayInferenceRequest {
    /// Conservative prompt bound used before provider execution.
    pub fn prompt_token_upper_bound(&self) -> u64 {
        self.instructions
            .iter()
            .map(|item| item.content.len() as u64 + 8)
            .sum::<u64>()
            + self
                .messages
                .iter()
                .map(|item| item.content.len() as u64 + 8)
                .sum::<u64>()
            + self
                .tools
                .iter()
                .map(|item| serde_json::to_vec(item).map_or(0, |value| value.len() as u64 + 8))
                .sum::<u64>()
    }
}

/// An instruction kept separate from ordinary conversation messages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayInstruction {
    /// Instruction text.
    pub content: String,
    /// Whether the instruction came from a developer or system field.
    pub role: InstructionRole,
}

/// Canonical instruction role.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstructionRole {
    /// System instruction.
    System,
    /// Developer instruction.
    Developer,
}

/// Provider-neutral tool definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool type, normally `function`.
    pub kind: String,
    /// Function name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// JSON Schema for arguments.
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// Provider-neutral tool selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Automatic selection.
    Auto,
    /// No tool selection.
    None,
    /// A required named tool.
    Function { name: String },
}

/// Provider-neutral structured output preference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// Format kind, such as `text` or `json_schema`.
    pub kind: String,
    /// Optional JSON schema payload.
    pub schema: Option<serde_json::Value>,
}

/// Canonical embeddings request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayEmbeddingRequest {
    /// Public model alias.
    pub requested_model: String,
    /// One or more input strings.
    pub inputs: Vec<String>,
    /// Optional provider-supported dimensions.
    pub dimensions: Option<u32>,
    /// Request metadata.
    #[serde(default)]
    pub metadata: RequestMetadata,
}

/// Canonical embeddings response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayEmbeddingResponse {
    /// Embedding vectors.
    pub embeddings: Vec<Vec<f32>>,
    /// Public model alias.
    pub model: String,
    /// Provider model name.
    pub upstream_model: String,
    /// Provider usage.
    pub usage: ProviderUsage,
    /// Safe provider metadata.
    #[serde(default)]
    pub provider_metadata: RequestMetadata,
}

/// Usage reported by an upstream provider.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderUsage {
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Cached input tokens.
    pub cached_input_tokens: u64,
}

impl GatewayRequest {
    /// Returns a conservative upper bound for prompt tokens.
    pub fn prompt_token_upper_bound(&self) -> u64 {
        self.messages
            .iter()
            .map(|message| message.content.len() as u64 + 8)
            .sum()
    }
}

/// Canonical token usage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input token count.
    pub prompt_tokens: u64,
    /// Generated token count.
    pub completion_tokens: u64,
    /// Whether counts were estimated rather than provider-reported.
    pub estimated: bool,
}

impl TokenUsage {
    /// Total input and output tokens.
    pub const fn total_tokens(self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }
}

/// Canonical completion result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayResponse {
    /// Provider request identifier.
    pub id: String,
    /// Public model alias.
    pub model: String,
    /// Assistant response text.
    pub content: String,
    /// Provider finish reason.
    pub finish_reason: Option<String>,
    /// Token usage.
    pub usage: TokenUsage,
}

/// A normalized streaming item.
#[derive(Clone, Debug, PartialEq)]
pub enum GatewayStreamEvent {
    /// Provider supplied the response identifier.
    Started { id: String },
    /// Incremental assistant text.
    Delta { content: String },
    /// Choice completed.
    Finished { reason: Option<String> },
    /// Final provider-reported usage.
    Usage(TokenUsage),
}

/// Static route selected for a public model alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRoute {
    /// Provider identifier.
    pub provider: String,
    /// Public alias exposed to clients.
    pub requested_model: String,
    /// Model name sent upstream.
    pub upstream_model: String,
}

/// State of a recorded provider request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    /// Provider completed successfully.
    Succeeded,
    /// Provider returned an error.
    ProviderFailed,
    /// Client disconnected during execution.
    Interrupted,
}

/// Whether a usage event had an exact model price at execution time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingStatus {
    /// A provider/model price was active for the event.
    Priced,
    /// No active provider/model price existed.
    Unpriced,
    /// Compatibility price from the process-wide legacy configuration.
    LegacyEstimate,
}

/// Immutable usage ledger entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsageEvent {
    /// Event identifier.
    pub event_id: Uuid,
    /// Gateway request identifier and idempotency key.
    pub request_id: Uuid,
    /// Owning tenant.
    pub tenant_id: String,
    /// Optional project attribution.
    pub project_id: Option<String>,
    /// Metered principal.
    pub principal_id: String,
    /// Optional end-user identifier.
    pub user_id: Option<String>,
    /// Provider identifier.
    pub provider: String,
    /// Public model alias.
    pub requested_model: String,
    /// Provider model.
    pub upstream_model: String,
    /// Token counts.
    pub usage: TokenUsage,
    /// Estimated USD cost.
    pub estimated_cost: Decimal,
    /// Source and reliability of the cost value.
    pub pricing_status: PricingStatus,
    /// Request outcome.
    pub status: UsageStatus,
    /// End-to-end attempt latency when observed.
    pub latency_ms: Option<u64>,
    /// Event creation time.
    pub occurred_at: DateTime<Utc>,
}

/// Quota owner for a request.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum QuotaOwner {
    /// Tenant-level quota used by JWT traffic.
    Tenant(String),
    /// Virtual-Key-specific quota.
    VirtualKey(Uuid),
}

impl QuotaOwner {
    /// Stable database discriminator.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Tenant(_) => "tenant",
            Self::VirtualKey(_) => "virtual_key",
        }
    }

    /// Stable owner identifier.
    pub fn id(&self) -> String {
        match self {
            Self::Tenant(id) => id.clone(),
            Self::VirtualKey(id) => id.to_string(),
        }
    }
}

/// Durable quota reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaReservation {
    /// Reservation identifier.
    pub reservation_id: Uuid,
    /// Associated request.
    pub request_id: Uuid,
    /// Quota owner.
    pub owner: QuotaOwner,
    /// Owning tenant.
    pub tenant_id: String,
    /// Optional project quota scope.
    pub project_id: Option<String>,
    /// Metered principal.
    pub principal_id: String,
    /// Optional end-user identifier.
    pub user_id: Option<String>,
    /// Provider identifier.
    pub provider: String,
    /// Public model alias.
    pub requested_model: String,
    /// Provider model name.
    pub upstream_model: String,
    /// Reserved prompt-token upper bound.
    pub prompt_tokens: u64,
    /// Reserved completion-token allowance.
    pub completion_tokens: u64,
    /// Reservation expiration time.
    pub expires_at: DateTime<Utc>,
}

impl QuotaReservation {
    /// Total reserved tokens.
    pub const fn reserved_tokens(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }
}

/// Persisted Virtual Key metadata. The plaintext key is never stored here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualKeyRecord {
    /// Key identifier.
    pub id: Uuid,
    /// Human-readable non-secret label.
    pub display_name: Option<String>,
    /// Non-secret lookup prefix.
    pub lookup_prefix: String,
    /// Argon2id encoded secret hash.
    pub secret_hash: String,
    /// Owning tenant.
    pub tenant_id: String,
    /// Optional project binding.
    pub project_id: Option<String>,
    /// Optional user binding.
    pub user_id: Option<String>,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Optional expiration.
    pub expires_at: Option<DateTime<Utc>>,
    /// Revocation time.
    pub revoked_at: Option<DateTime<Utc>>,
    /// Daily token limit.
    pub daily_token_limit: u64,
    /// Public model aliases allowed for this key; empty means policy-controlled.
    pub allowed_models: Vec<String>,
    /// Optional daily request ceiling.
    pub daily_request_limit: Option<u64>,
    /// Optional monthly estimated-cost ceiling.
    pub monthly_budget: Option<Decimal>,
}

/// Metadata accepted when issuing a Virtual Key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewVirtualKey {
    /// Owning tenant.
    pub tenant_id: String,
    /// Human-readable non-secret label.
    pub display_name: Option<String>,
    /// Optional project binding.
    pub project_id: Option<String>,
    /// Optional user binding.
    pub user_id: Option<String>,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Optional expiration.
    pub expires_at: Option<DateTime<Utc>>,
    /// Daily token limit.
    pub daily_token_limit: u64,
    /// Public model aliases allowed for this key.
    pub allowed_models: Vec<String>,
    /// Optional daily request ceiling.
    pub daily_request_limit: Option<u64>,
    /// Optional monthly estimated-cost ceiling.
    pub monthly_budget: Option<Decimal>,
}

/// Plaintext key returned exactly once after issuance.
#[derive(Clone, Eq, PartialEq)]
pub struct IssuedVirtualKey {
    /// Persisted metadata.
    pub record: VirtualKeyRecord,
    /// Plaintext bearer credential.
    pub plaintext: PlaintextVirtualKey,
}

impl std::fmt::Debug for IssuedVirtualKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssuedVirtualKey")
            .field("record", &self.record)
            .field("plaintext", &"[REDACTED]")
            .finish()
    }
}

/// One-time plaintext Virtual Key with redacted debug formatting.
#[derive(Clone, Eq, PartialEq)]
pub struct PlaintextVirtualKey(String);

impl PlaintextVirtualKey {
    /// Wrap a newly generated plaintext key.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Expose the key only at the response boundary.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for PlaintextVirtualKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

/// Additional request metadata passed through the core pipeline.
pub type RequestMetadata = HashMap<String, String>;

/// JSON metadata attached to MCP and security records.
pub type Metadata = HashMap<String, serde_json::Value>;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

uuid_id!(McpServerId);
uuid_id!(ApprovalId);
uuid_id!(FindingId);
uuid_id!(IncidentId);
uuid_id!(SecurityPolicyId);
uuid_id!(McpPolicyId);

/// Opaque reference to encrypted secret material.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretRef(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportType {
    Stdio,
    StreamableHttp,
}

/// Internal transport configuration. This type is never serialized in API responses.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransportConfig {
    Stdio {
        command: String,
        arguments: Vec<String>,
        environment_secret_refs: Vec<SecretRef>,
    },
    StreamableHttp {
        endpoint: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayMcpServer {
    pub server_id: McpServerId,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub transport: McpTransportConfig,
    pub credential_ref: Option<SecretRef>,
    pub enabled: bool,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolAnnotations {
    pub title: Option<String>,
    pub read_only_hint: Option<bool>,
    pub destructive_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
    pub administrator_risk: Option<ToolRiskLevel>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayMcpTool {
    pub server_id: McpServerId,
    pub tool_name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub annotations: ToolAnnotations,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayMcpInvocation {
    pub server_id: McpServerId,
    pub tool_name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentPart {
    Text {
        text: String,
    },
    Json {
        value: serde_json::Value,
    },
    Resource {
        uri: String,
        text: Option<String>,
        mime_type: Option<String>,
    },
    Binary {
        mime_type: String,
        data_base64: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayMcpResult {
    pub content: Vec<McpContentPart>,
    pub is_error: bool,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    ReadOnly,
    Mutating,
    Destructive,
    Privileged,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageOperation {
    ChatCompletion,
    Response,
    Embedding,
    McpToolInvocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct McpUsageDetails {
    pub server_id: McpServerId,
    pub tool_name: String,
    pub invocation_count: u64,
    pub duration_ms: u64,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub risk_level: ToolRiskLevel,
    pub approval_required: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityCategory {
    PromptInjection,
    JailbreakAttempt,
    SecretExposure,
    CredentialExposure,
    SensitivePersonalData,
    FinancialData,
    SourceCodeSecret,
    DataExfiltrationAttempt,
    PolicyViolation,
    SuspiciousToolArgument,
    SuspiciousToolResult,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityAction {
    Allow,
    Warn,
    Redact,
    RequireApproval,
    Block,
}

impl SecurityAction {
    pub const fn priority(self) -> u8 {
        match self {
            Self::Allow => 0,
            Self::Warn => 1,
            Self::Redact => 2,
            Self::RequireApproval => 3,
            Self::Block => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SanitizedEvidence {
    pub redacted: String,
    pub sha256: String,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub finding_id: FindingId,
    pub inspector_id: String,
    pub category: SecurityCategory,
    pub severity: SecuritySeverity,
    pub confidence: f32,
    pub evidence: Vec<SanitizedEvidence>,
    pub recommended_action: SecurityAction,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "snake_case")]
pub enum InspectionContent {
    PromptText(String),
    StructuredInput(serde_json::Value),
    ToolArguments(serde_json::Value),
    ToolResult(serde_json::Value),
    ModelOutput(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InspectionContext {
    pub request_id: Uuid,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub principal_id: String,
    pub stage: String,
    pub tool_risk: Option<ToolRiskLevel>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalResourceType {
    McpTool,
    InferenceRequest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub approval_id: ApprovalId,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub principal_id: String,
    pub request_id: Uuid,
    pub resource_type: ApprovalResourceType,
    pub resource_id: String,
    pub action: String,
    pub sanitized_arguments: serde_json::Value,
    pub risk_level: ToolRiskLevel,
    pub status: ApprovalStatus,
    pub request_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Resolved,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecurityIncident {
    pub incident_id: IncidentId,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub principal_id: Option<String>,
    pub request_id: Uuid,
    pub category: SecurityCategory,
    pub severity: SecuritySeverity,
    pub status: IncidentStatus,
    pub risk_score: u8,
    pub sanitized_summary: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}
