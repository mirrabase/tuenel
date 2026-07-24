use async_trait::async_trait;
use gateway_providers::{
    GatewayStream, ModelProvider, ProviderCapabilities, ProviderContext, ProviderError,
};
use gateway_types::{
    GatewayEmbeddingRequest, GatewayEmbeddingResponse, GatewayInferenceRequest, GatewayMessage,
    GatewayRequest, GatewayResponse, MessageRole, TokenUsage,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

pub struct AnthropicProvider {
    id: String,
    endpoint: Url,
    models_endpoint: Url,
    api_key: SecretString,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(
        id: String,
        mut base_url: Url,
        api_key: SecretString,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let endpoint = base_url
            .join("v1/messages")
            .map_err(|_| ProviderError::Protocol)?;
        let models_endpoint = base_url
            .join("v1/models")
            .map_err(|_| ProviderError::Protocol)?;
        Ok(Self {
            id,
            endpoint,
            models_endpoint,
            api_key,
            client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|_| ProviderError::Transport)?,
        })
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            responses: true,
            streaming: false,
            tool_calling: true,
            ..Default::default()
        }
    }
    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let response = self
            .client
            .get(self.models_endpoint.clone())
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .query(&[("limit", 1000)])
            .send()
            .await
            .map_err(map_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Upstream(status.as_u16()));
        }
        let response: AnthropicModelList =
            response.json().await.map_err(|_| ProviderError::Protocol)?;
        Ok(response.data.into_iter().map(|model| model.id).collect())
    }
    async fn infer(
        &self,
        context: ProviderContext,
        request: GatewayInferenceRequest,
    ) -> Result<GatewayResponse, ProviderError> {
        let system = request
            .instructions
            .into_iter()
            .map(|item| item.content)
            .collect::<Vec<_>>()
            .join("\n");
        let messages = request
            .messages
            .iter()
            .map(AnthropicMessage::from)
            .collect();
        let body = AnthropicRequest {
            model: context.upstream_model,
            max_tokens: request.generation.max_output_tokens,
            system,
            messages,
        };
        let response = self
            .client
            .post(self.endpoint.clone())
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .json(&body)
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
        let response: AnthropicResponse =
            response.json().await.map_err(|_| ProviderError::Protocol)?;
        let content = response
            .content
            .into_iter()
            .filter_map(|item| item.text)
            .collect();
        Ok(GatewayResponse {
            id: response.id,
            model: request.requested_model,
            content,
            finish_reason: response.stop_reason,
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
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}
#[derive(Serialize)]
struct AnthropicMessage {
    role: &'static str,
    content: String,
}
impl From<&GatewayMessage> for AnthropicMessage {
    fn from(value: &GatewayMessage) -> Self {
        Self {
            role: if matches!(value.role, MessageRole::Assistant) {
                "assistant"
            } else {
                "user"
            },
            content: value.content.clone(),
        }
    }
}
#[derive(Deserialize)]
struct AnthropicResponse {
    id: String,
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}
#[derive(Deserialize)]
struct AnthropicModelList {
    data: Vec<AnthropicModel>,
}
#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
}
#[derive(Deserialize)]
struct AnthropicContent {
    text: Option<String>,
}
#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}
impl From<AnthropicUsage> for TokenUsage {
    fn from(value: AnthropicUsage) -> Self {
        Self {
            prompt_tokens: value.input_tokens,
            completion_tokens: value.output_tokens,
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

#[cfg(test)]
mod tests {
    use super::AnthropicModelList;

    #[test]
    fn parses_model_ids() {
        let response: AnthropicModelList =
            serde_json::from_str(r#"{"data":[{"id":"claude-test"}]}"#).unwrap();
        assert_eq!(response.data[0].id, "claude-test");
    }
}
