//! Provider-neutral inference interfaces.

mod health;
pub use health::{ProviderHealthMonitor, ProviderHealthRepository, ProviderHealthTracker};

use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::Stream;
use gateway_types::{
    GatewayEmbeddingRequest, GatewayEmbeddingResponse, GatewayInferenceRequest, GatewayRequest,
    GatewayResponse, GatewayStreamEvent,
};
use thiserror::Error;

/// Context selected by routing and supplied to a provider.
#[derive(Clone, Debug)]
pub struct ProviderContext {
    /// Gateway request identifier.
    pub request_id: uuid::Uuid,
    /// Upstream model name.
    pub upstream_model: String,
}

/// Provider-neutral asynchronous stream.
pub type GatewayStream =
    Pin<Box<dyn Stream<Item = Result<GatewayStreamEvent, ProviderError>> + Send>>;

/// Immutable provider registry used by route resolution and administration.
#[derive(Clone, Default)]
pub struct ProviderRegistry(Arc<HashMap<String, Arc<dyn ModelProvider>>>);

impl ProviderRegistry {
    pub fn new(providers: impl IntoIterator<Item = Arc<dyn ModelProvider>>) -> Self {
        Self(Arc::new(
            providers
                .into_iter()
                .map(|provider| (provider.id().to_owned(), provider))
                .collect(),
        ))
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn ModelProvider>> {
        self.0.get(id).cloned()
    }
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub async fn check_health(
        &self,
        id: &str,
        timeout: Duration,
    ) -> Result<ProviderHealth, ProviderError> {
        let provider = self.get(id).ok_or(ProviderError::Unavailable)?;
        Ok(
            match tokio::time::timeout(timeout, provider.health_check()).await {
                Ok(Ok(health)) => health,
                _ => ProviderHealth {
                    status: ProviderHealthStatus::Unhealthy,
                    consecutive_failures: 1,
                    latest_success_at: None,
                    latest_failure_at: Some(chrono::Utc::now()),
                },
            },
        )
    }

    pub async fn list_models(
        &self,
        id: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, ProviderError> {
        let provider = self.get(id).ok_or(ProviderError::Unavailable)?;
        tokio::time::timeout(timeout, provider.list_models())
            .await
            .map_err(|_| ProviderError::Timeout)?
    }
}

/// Operations and features supported by a provider adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderCapabilities {
    pub chat_completions: bool,
    pub responses: bool,
    pub embeddings: bool,
    pub streaming: bool,
    pub tool_calling: bool,
    pub parallel_tool_calls: bool,
    pub structured_output: bool,
    pub vision: bool,
    pub usage_in_stream: bool,
}

/// Provider health snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderHealth {
    pub status: ProviderHealthStatus,
    pub consecutive_failures: u32,
    pub latest_success_at: Option<chrono::DateTime<chrono::Utc>>,
    pub latest_failure_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Coarse provider health state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Upstream provider contract.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Stable provider identifier.
    fn id(&self) -> &str;

    /// Capabilities advertised by this adapter.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            chat_completions: true,
            streaming: true,
            ..ProviderCapabilities::default()
        }
    }

    /// Execute a canonical v0.2 inference request.
    async fn infer(
        &self,
        _context: ProviderContext,
        _request: GatewayInferenceRequest,
    ) -> Result<GatewayResponse, ProviderError> {
        Err(ProviderError::Unsupported)
    }

    /// Start a canonical v0.2 inference stream.
    async fn stream_infer(
        &self,
        _context: ProviderContext,
        _request: GatewayInferenceRequest,
    ) -> Result<GatewayStream, ProviderError> {
        Err(ProviderError::Unsupported)
    }

    /// Execute a canonical embeddings request.
    async fn embed(
        &self,
        _context: ProviderContext,
        _request: GatewayEmbeddingRequest,
    ) -> Result<GatewayEmbeddingResponse, ProviderError> {
        Err(ProviderError::Unsupported)
    }

    /// Return the latest provider health result.
    async fn health_check(&self) -> Result<ProviderHealth, ProviderError> {
        Ok(ProviderHealth {
            status: ProviderHealthStatus::Unknown,
            consecutive_failures: 0,
            latest_success_at: None,
            latest_failure_at: None,
        })
    }

    /// List model identifiers currently available from the upstream provider.
    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        Err(ProviderError::Unsupported)
    }

    /// Execute a non-streaming chat completion.
    async fn execute(
        &self,
        context: ProviderContext,
        request: GatewayRequest,
    ) -> Result<GatewayResponse, ProviderError>;

    /// Start a streaming chat completion.
    async fn stream(
        &self,
        context: ProviderContext,
        request: GatewayRequest,
    ) -> Result<GatewayStream, ProviderError>;
}

/// Sanitized provider failure.
#[derive(Clone, Debug, Error)]
pub enum ProviderError {
    /// The configured provider is unavailable in the registry.
    #[error("provider is unavailable")]
    Unavailable,

    /// The selected adapter does not support the requested operation.
    #[error("provider capability is unsupported")]
    Unsupported,
    /// Upstream request timed out.
    #[error("upstream request timed out")]
    Timeout,
    /// Upstream rejected rate limits.
    #[error("upstream rate limit exceeded")]
    RateLimited,
    /// Upstream returned a non-success response.
    #[error("upstream request failed with status {0}")]
    Upstream(u16),
    /// Upstream returned a sanitized structured error.
    #[error("upstream request failed with status {status} ({code})")]
    UpstreamRejected {
        /// Upstream HTTP status.
        status: u16,
        /// Provider error code with credentials and free-form details removed.
        code: String,
    },
    /// Upstream response did not follow the supported protocol.
    #[error("upstream returned an invalid response")]
    Protocol,
    /// Upstream transport failed.
    #[error("upstream transport failed")]
    Transport,
}
