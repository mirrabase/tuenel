use async_trait::async_trait;
use gateway_providers::{
    GatewayStream, ModelProvider, ProviderCapabilities, ProviderContext, ProviderError,
};
use gateway_types::{
    GatewayEmbeddingRequest, GatewayEmbeddingResponse, GatewayInferenceRequest, GatewayRequest,
    GatewayResponse, TokenUsage,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

pub struct GeminiProvider {
    id: String,
    base_url: Url,
    api_key: SecretString,
    client: reqwest::Client,
}
impl GeminiProvider {
    pub fn new(
        id: String,
        base_url: Url,
        api_key: SecretString,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            id,
            base_url,
            api_key,
            client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|_| ProviderError::Transport)?,
        })
    }
}
#[async_trait]
impl ModelProvider for GeminiProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            responses: true,
            embeddings: true,
            tool_calling: true,
            ..Default::default()
        }
    }
    async fn infer(
        &self,
        context: ProviderContext,
        request: GatewayInferenceRequest,
    ) -> Result<GatewayResponse, ProviderError> {
        let contents = request
            .messages
            .into_iter()
            .map(|message| GeminiContent {
                role: "user",
                parts: vec![GeminiPart {
                    text: message.content,
                }],
            })
            .collect();
        let endpoint = self
            .base_url
            .join(&format!(
                "v1beta/models/{}:generateContent",
                context.upstream_model
            ))
            .map_err(|_| ProviderError::Protocol)?;
        let response = self
            .client
            .post(endpoint)
            .query(&[("key", self.api_key.expose_secret())])
            .json(&GeminiRequest { contents })
            .send()
            .await
            .map_err(map_error)?;
        let status = response.status();
        if status.as_u16() == 429 {
            return Err(ProviderError::RateLimited);
        }
        if !status.is_success() {
            return Err(ProviderError::Upstream(status.as_u16()));
        }
        let response: GeminiResponse =
            response.json().await.map_err(|_| ProviderError::Protocol)?;
        let content = response
            .candidates
            .into_iter()
            .flat_map(|item| item.content.parts)
            .map(|part| part.text)
            .collect();
        Ok(GatewayResponse {
            id: format!("gemini-{}", context.request_id),
            model: request.requested_model,
            content,
            finish_reason: None,
            usage: response.usage.map(Into::into).unwrap_or_default(),
        })
    }
    async fn execute(
        &self,
        context: ProviderContext,
        request: GatewayRequest,
    ) -> Result<GatewayResponse, ProviderError> {
        self.infer(
            context,
            GatewayInferenceRequest {
                requested_model: request.model,
                instructions: Vec::new(),
                messages: request.messages,
                tools: Vec::new(),
                tool_choice: None,
                response_format: None,
                generation: request.generation,
                stream: false,
                metadata: Default::default(),
            },
        )
        .await
    }
    async fn stream(
        &self,
        _: ProviderContext,
        _: GatewayRequest,
    ) -> Result<GatewayStream, ProviderError> {
        Err(ProviderError::Unsupported)
    }
    async fn stream_infer(
        &self,
        _: ProviderContext,
        _: GatewayInferenceRequest,
    ) -> Result<GatewayStream, ProviderError> {
        Err(ProviderError::Unsupported)
    }
    async fn embed(
        &self,
        _: ProviderContext,
        _: GatewayEmbeddingRequest,
    ) -> Result<GatewayEmbeddingResponse, ProviderError> {
        Err(ProviderError::Unsupported)
    }
}
#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}
#[derive(Serialize)]
struct GeminiContent {
    role: &'static str,
    parts: Vec<GeminiPart>,
}
#[derive(Serialize)]
struct GeminiPart {
    text: String,
}
#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage: Option<GeminiUsage>,
}
#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiOutput,
}
#[derive(Deserialize)]
struct GeminiOutput {
    parts: Vec<GeminiPartOutput>,
}
#[derive(Deserialize)]
struct GeminiPartOutput {
    text: String,
}
#[derive(Deserialize)]
struct GeminiUsage {
    #[serde(rename = "promptTokenCount")]
    input: u64,
    #[serde(rename = "candidatesTokenCount")]
    output: u64,
}
impl From<GeminiUsage> for TokenUsage {
    fn from(value: GeminiUsage) -> Self {
        Self {
            prompt_tokens: value.input,
            completion_tokens: value.output,
            estimated: false,
        }
    }
}
fn map_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Transport
    }
}
