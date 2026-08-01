//! OpenAI-compatible upstream adapter for OpenAI, vLLM, Ollama, and LocalAI.

use std::time::Duration;

use async_stream::try_stream;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use gateway_providers::{
    GatewayStream, ModelProvider, ProviderCapabilities, ProviderContext, ProviderError,
    ProviderHealth, ProviderHealthStatus,
};
use gateway_types::{
    GatewayEmbeddingRequest, GatewayEmbeddingResponse, GatewayInferenceRequest, GatewayMessage,
    GatewayRequest, GatewayResponse, GatewayStreamEvent, MessageRole, TokenUsage,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use url::Url;

/// OpenAI-compatible HTTP provider.
pub struct OpenAiCompatibleProvider {
    id: String,
    endpoint: Url,
    responses_endpoint: Url,
    embeddings_endpoint: Url,
    models_endpoint: Url,
    api_key: Option<SecretString>,
    client: reqwest::Client,
    dialect: OpenAiDialect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenAiDialect {
    Compatible,
    Official,
}

impl OpenAiCompatibleProvider {
    /// Build an adapter with bounded connect and total request timeouts.
    pub fn new(
        id: String,
        base_url: Url,
        api_key: Option<SecretString>,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        Self::with_dialect(id, base_url, api_key, timeout, OpenAiDialect::Compatible)
    }

    /// Build the official OpenAI adapter. It uses the modern OpenAI request dialect.
    pub fn new_openai(
        id: String,
        base_url: Url,
        api_key: SecretString,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        Self::with_dialect(
            id,
            base_url,
            Some(api_key),
            timeout,
            OpenAiDialect::Official,
        )
    }

    fn with_dialect(
        id: String,
        mut base_url: Url,
        api_key: Option<SecretString>,
        timeout: Duration,
        dialect: OpenAiDialect,
    ) -> Result<Self, ProviderError> {
        if !base_url.path().ends_with('/') {
            let path = format!("{}/", base_url.path());
            base_url.set_path(&path);
        }
        let endpoint = base_url
            .join("chat/completions")
            .map_err(|_| ProviderError::Protocol)?;
        let responses_endpoint = base_url
            .join("responses")
            .map_err(|_| ProviderError::Protocol)?;
        let embeddings_endpoint = base_url
            .join("embeddings")
            .map_err(|_| ProviderError::Protocol)?;
        let models_endpoint = base_url
            .join("models")
            .map_err(|_| ProviderError::Protocol)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(timeout)
            .build()
            .map_err(|_| ProviderError::Transport)?;
        Ok(Self {
            id,
            endpoint,
            responses_endpoint,
            embeddings_endpoint,
            models_endpoint,
            api_key,
            client,
            dialect,
        })
    }

    fn request(&self, body: &ProviderRequest) -> reqwest::RequestBuilder {
        self.request_at(self.endpoint.clone(), body)
    }

    fn request_at<T: Serialize>(&self, endpoint: Url, body: &T) -> reqwest::RequestBuilder {
        let request = self.client.post(endpoint).json(body);
        match &self.api_key {
            Some(key) => request.bearer_auth(key.expose_secret()),
            None => request,
        }
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            chat_completions: true,
            responses: true,
            embeddings: true,
            streaming: true,
            tool_calling: true,
            parallel_tool_calls: true,
            structured_output: true,
            usage_in_stream: true,
            ..ProviderCapabilities::default()
        }
    }

    async fn health_check(&self) -> Result<ProviderHealth, ProviderError> {
        let request = self.client.get(self.models_endpoint.clone());
        let request = match &self.api_key {
            Some(key) => request.bearer_auth(key.expose_secret()),
            None => request,
        };
        require_success(request.send().await.map_err(map_reqwest)?).await?;
        Ok(ProviderHealth {
            status: ProviderHealthStatus::Healthy,
            consecutive_failures: 0,
            latest_success_at: Some(chrono::Utc::now()),
            latest_failure_at: None,
        })
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let request = self.client.get(self.models_endpoint.clone());
        let request = match &self.api_key {
            Some(key) => request.bearer_auth(key.expose_secret()),
            None => request,
        };
        let response = require_success(request.send().await.map_err(map_reqwest)?).await?;
        let response: ProviderModelList =
            response.json().await.map_err(|_| ProviderError::Protocol)?;
        Ok(response.data.into_iter().map(|model| model.id).collect())
    }

    async fn infer(
        &self,
        context: ProviderContext,
        request: GatewayInferenceRequest,
    ) -> Result<GatewayResponse, ProviderError> {
        let body = ResponsesRequest::from_gateway(&context.upstream_model, &request, false);
        let response = self
            .request_at(self.responses_endpoint.clone(), &body)
            .send()
            .await
            .map_err(map_reqwest)?;
        let response = require_success(response).await?;
        let response: ResponsesResponse =
            response.json().await.map_err(|_| ProviderError::Protocol)?;
        let content = response.text_content().ok_or(ProviderError::Protocol)?;
        Ok(GatewayResponse {
            id: response.id,
            model: request.requested_model,
            content,
            finish_reason: response.status,
            usage: response.usage.map(Into::into).unwrap_or(TokenUsage {
                prompt_tokens: request
                    .messages
                    .iter()
                    .map(|m| m.content.len() as u64 + 8)
                    .sum(),
                completion_tokens: 0,
                estimated: true,
            }),
        })
    }

    async fn embed(
        &self,
        context: ProviderContext,
        request: GatewayEmbeddingRequest,
    ) -> Result<GatewayEmbeddingResponse, ProviderError> {
        let body = EmbeddingsRequest {
            model: context.upstream_model.clone(),
            input: request.inputs,
            dimensions: request.dimensions,
        };
        let response = self
            .request_at(self.embeddings_endpoint.clone(), &body)
            .send()
            .await
            .map_err(map_reqwest)?;
        let response = require_success(response).await?;
        let response: EmbeddingsResponse =
            response.json().await.map_err(|_| ProviderError::Protocol)?;
        Ok(GatewayEmbeddingResponse {
            embeddings: response
                .data
                .into_iter()
                .map(|item| item.embedding)
                .collect(),
            model: request.requested_model,
            upstream_model: context.upstream_model,
            usage: response.usage.map(Into::into).unwrap_or_default(),
            provider_metadata: Default::default(),
        })
    }

    async fn execute(
        &self,
        context: ProviderContext,
        request: GatewayRequest,
    ) -> Result<GatewayResponse, ProviderError> {
        let prompt_upper = request.prompt_token_upper_bound();
        let body =
            ProviderRequest::from_gateway(&context.upstream_model, &request, false, self.dialect);
        let response = self.request(&body).send().await.map_err(map_reqwest)?;
        let response = require_success(response).await?;
        let response: ProviderResponse =
            response.json().await.map_err(|_| ProviderError::Protocol)?;
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or(ProviderError::Protocol)?;
        let content = choice.message.content.unwrap_or_default();
        let usage = response.usage.map(Into::into).unwrap_or(TokenUsage {
            prompt_tokens: prompt_upper,
            completion_tokens: content.len() as u64,
            estimated: true,
        });
        Ok(GatewayResponse {
            id: response.id,
            model: request.model,
            content,
            finish_reason: choice.finish_reason,
            usage,
        })
    }

    async fn stream(
        &self,
        context: ProviderContext,
        request: GatewayRequest,
    ) -> Result<GatewayStream, ProviderError> {
        let body =
            ProviderRequest::from_gateway(&context.upstream_model, &request, true, self.dialect);
        let response = self.request(&body).send().await.map_err(map_reqwest)?;
        let response = require_success(response).await?;
        let mut source = response.bytes_stream().eventsource();
        let stream = try_stream! {
            let mut started = false;
            while let Some(event) = source.next().await {
                let event = event.map_err(|_| ProviderError::Protocol)?;
                if event.data == "[DONE]" {
                    break;
                }
                let chunk: ProviderStreamChunk =
                    serde_json::from_str(&event.data).map_err(|_| ProviderError::Protocol)?;
                if !started {
                    started = true;
                    yield GatewayStreamEvent::Started { id: chunk.id.clone() };
                }
                if let Some(usage) = chunk.usage {
                    yield GatewayStreamEvent::Usage(usage.into());
                }
                for choice in chunk.choices {
                    if let Some(content) = choice.delta.content {
                        yield GatewayStreamEvent::Delta { content };
                    }
                    if choice.finish_reason.is_some() {
                        yield GatewayStreamEvent::Finished { reason: choice.finish_reason };
                    }
                }
            }
        };
        Ok(Box::pin(stream))
    }
}

#[derive(Debug, Serialize)]
struct ResponsesRequest<'a> {
    model: &'a str,
    input: Vec<ResponsesInput<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    instructions: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<&'a gateway_types::ToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a gateway_types::ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    max_output_tokens: u32,
    stream: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "role", content = "content")]
enum ResponsesInput<'a> {
    #[serde(rename = "user")]
    User(&'a str),
    #[serde(rename = "assistant")]
    Assistant(&'a str),
}

impl<'a> ResponsesRequest<'a> {
    fn from_gateway(model: &'a str, request: &'a GatewayInferenceRequest, stream: bool) -> Self {
        let input = request
            .messages
            .iter()
            .map(|message| match message.role {
                MessageRole::Assistant => ResponsesInput::Assistant(&message.content),
                MessageRole::System | MessageRole::User => ResponsesInput::User(&message.content),
            })
            .collect();
        Self {
            model,
            input,
            instructions: request
                .instructions
                .iter()
                .map(|item| item.content.as_str())
                .collect(),
            tools: request.tools.iter().collect(),
            tool_choice: request.tool_choice.as_ref(),
            temperature: request.generation.temperature,
            top_p: request.generation.top_p,
            max_output_tokens: request.generation.max_output_tokens,
            stream,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResponsesResponse {
    id: String,
    #[serde(default)]
    output: Vec<ResponsesOutput>,
    status: Option<String>,
    usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponsesOutput {
    #[serde(default)]
    content: Vec<ResponsesContent>,
}

#[derive(Debug, Deserialize)]
struct ResponsesContent {
    text: Option<String>,
}

impl ResponsesResponse {
    fn text_content(&self) -> Option<String> {
        let text = self
            .output
            .iter()
            .flat_map(|item| item.content.iter())
            .filter_map(|item| item.text.as_deref())
            .collect::<String>();
        (!text.is_empty()).then_some(text)
    }
}

#[derive(Debug, Deserialize)]
struct ResponsesUsage {
    input_tokens: u64,
    output_tokens: u64,
}

impl From<ResponsesUsage> for TokenUsage {
    fn from(value: ResponsesUsage) -> Self {
        Self {
            prompt_tokens: value.input_tokens,
            completion_tokens: value.output_tokens,
            estimated: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct EmbeddingsRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingItem>,
    usage: Option<EmbeddingUsage>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingUsage {
    prompt_tokens: u64,
}

impl From<EmbeddingUsage> for gateway_types::ProviderUsage {
    fn from(value: EmbeddingUsage) -> Self {
        Self {
            input_tokens: value.prompt_tokens,
            output_tokens: 0,
            cached_input_tokens: 0,
        }
    }
}

#[derive(Debug, Serialize)]
struct ProviderRequest<'a> {
    model: &'a str,
    messages: Vec<ProviderMessage<'a>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
}

impl<'a> ProviderRequest<'a> {
    fn from_gateway(
        model: &'a str,
        request: &'a GatewayRequest,
        stream: bool,
        dialect: OpenAiDialect,
    ) -> Self {
        Self {
            model,
            messages: request.messages.iter().map(Into::into).collect(),
            stream,
            temperature: request.generation.temperature,
            top_p: request.generation.top_p,
            max_tokens: (dialect == OpenAiDialect::Compatible)
                .then_some(request.generation.max_output_tokens),
            max_completion_tokens: (dialect == OpenAiDialect::Official)
                .then_some(request.generation.max_output_tokens),
            stop: request.generation.stop.clone(),
            stream_options: stream.then_some(StreamOptions {
                include_usage: true,
            }),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProviderMessage<'a> {
    role: &'static str,
    content: &'a str,
}

impl<'a> From<&'a GatewayMessage> for ProviderMessage<'a> {
    fn from(message: &'a GatewayMessage) -> Self {
        Self {
            role: match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
            },
            content: &message.content,
        }
    }
}

#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Debug, Deserialize)]
struct ProviderResponse {
    id: String,
    choices: Vec<ProviderChoice>,
    usage: Option<ProviderUsage>,
}

#[derive(Debug, Deserialize)]
struct ProviderModelList {
    data: Vec<ProviderModel>,
}

#[derive(Debug, Deserialize)]
struct ProviderModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ProviderChoice {
    message: ProviderResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderStreamChunk {
    id: String,
    #[serde(default)]
    choices: Vec<ProviderStreamChoice>,
    usage: Option<ProviderUsage>,
}

#[derive(Debug, Deserialize)]
struct ProviderStreamChoice {
    #[serde(default)]
    delta: ProviderDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ProviderDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

impl From<ProviderUsage> for TokenUsage {
    fn from(usage: ProviderUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            estimated: false,
        }
    }
}

async fn require_success(response: reqwest::Response) -> Result<reqwest::Response, ProviderError> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else if status.as_u16() == 429 {
        Err(ProviderError::RateLimited)
    } else {
        let code = response
            .json::<OpenAiErrorEnvelope>()
            .await
            .ok()
            .and_then(|body| body.error.code.or(body.error.error_type))
            .filter(|code| {
                !code.is_empty()
                    && code.len() <= 100
                    && code
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
            .unwrap_or_else(|| "upstream_rejected".to_owned());
        Err(ProviderError::UpstreamRejected {
            status: status.as_u16(),
            code,
        })
    }
}

#[derive(Deserialize)]
struct OpenAiErrorEnvelope {
    error: OpenAiErrorBody,
}

#[derive(Deserialize)]
struct OpenAiErrorBody {
    code: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

fn map_reqwest(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Transport
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        http::{StatusCode, header::CONTENT_TYPE},
        response::IntoResponse,
        routing::{get, post},
    };
    use futures::StreamExt;
    use gateway_providers::{ModelProvider, ProviderContext, ProviderError, ProviderHealthStatus};
    use gateway_types::{
        GatewayMessage, GatewayRequest, GatewayStreamEvent, GenerationParameters, MessageRole,
    };
    use url::Url;
    use uuid::Uuid;

    use super::{OpenAiCompatibleProvider, OpenAiDialect, ProviderRequest};

    fn request(stream: bool) -> GatewayRequest {
        GatewayRequest {
            model: "gateway-default".into(),
            messages: vec![GatewayMessage {
                role: MessageRole::User,
                content: "hello".into(),
            }],
            stream,
            stream_include_usage: true,
            generation: GenerationParameters {
                max_output_tokens: 32,
                ..Default::default()
            },
        }
    }

    async fn provider(app: Router) -> OpenAiCompatibleProvider {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        OpenAiCompatibleProvider::new(
            "test".into(),
            Url::parse(&format!("http://{address}/v1/")).unwrap(),
            None,
            std::time::Duration::from_secs(5),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn maps_non_streaming_response() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                axum::Json(serde_json::json!({
                    "id":"chatcmpl-test",
                    "choices":[{"message":{"content":"hi"},"finish_reason":"stop"}],
                    "usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}
                }))
            }),
        );
        let provider = provider(app).await;
        let response = provider
            .execute(
                ProviderContext {
                    request_id: Uuid::new_v4(),
                    upstream_model: "upstream".into(),
                },
                request(false),
            )
            .await
            .unwrap();
        assert_eq!(response.content, "hi");
        assert_eq!(response.model, "gateway-default");
        assert!(!response.usage.estimated);
    }

    #[test]
    fn official_openai_uses_the_modern_completion_token_field() {
        let gateway_request = request(false);
        let official = serde_json::to_value(ProviderRequest::from_gateway(
            "gpt-5",
            &gateway_request,
            false,
            OpenAiDialect::Official,
        ))
        .unwrap();
        assert_eq!(official["max_completion_tokens"], 32);
        assert!(official.get("max_tokens").is_none());

        let compatible = serde_json::to_value(ProviderRequest::from_gateway(
            "qwen",
            &gateway_request,
            false,
            OpenAiDialect::Compatible,
        ))
        .unwrap();
        assert_eq!(compatible["max_tokens"], 32);
        assert!(compatible.get("max_completion_tokens").is_none());
    }

    #[tokio::test]
    async fn preserves_safe_structured_upstream_error_codes() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({
                        "error": {"type": "invalid_request_error", "code": "invalid_api_key"}
                    })),
                )
            }),
        );
        let error = provider(app)
            .await
            .execute(
                ProviderContext {
                    request_id: Uuid::new_v4(),
                    upstream_model: "gpt-5".into(),
                },
                request(false),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ProviderError::UpstreamRejected { status: 401, code }
                if code == "invalid_api_key"
        ));
    }

    #[tokio::test]
    async fn checks_provider_health_against_models_endpoint() {
        let app = Router::new().route(
            "/v1/models",
            get(|| async { axum::Json(serde_json::json!({"data": []})) }),
        );
        let health = provider(app).await.health_check().await.unwrap();
        assert_eq!(health.status, ProviderHealthStatus::Healthy);
        assert!(health.latest_success_at.is_some());
    }

    #[tokio::test]
    async fn lists_upstream_models() {
        let app = Router::new().route(
            "/v1/models",
            get(|| async {
                axum::Json(serde_json::json!({
                    "data": [{"id": "model-b"}, {"id": "model-a"}]
                }))
            }),
        );
        assert_eq!(
            provider(app).await.list_models().await.unwrap(),
            ["model-b", "model-a"]
        );
    }

    #[tokio::test]
    async fn parses_sse_without_buffering() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    [(CONTENT_TYPE, "text/event-stream")],
                    "data: {\"id\":\"chatcmpl-test\",\"choices\":[{\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chatcmpl-test\",\"choices\":[],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1}}\n\ndata: [DONE]\n\n",
                )
                    .into_response()
            }),
        );
        let provider = provider(app).await;
        let mut stream = provider
            .stream(
                ProviderContext {
                    request_id: Uuid::new_v4(),
                    upstream_model: "upstream".into(),
                },
                request(true),
            )
            .await
            .unwrap();
        let mut saw_delta = false;
        let mut saw_usage = false;
        while let Some(event) = stream.next().await {
            match event.unwrap() {
                GatewayStreamEvent::Delta { content } if content == "hi" => saw_delta = true,
                GatewayStreamEvent::Usage(usage) if usage.total_tokens() == 3 => saw_usage = true,
                _ => {}
            }
        }
        assert!(saw_delta && saw_usage);
    }
}
