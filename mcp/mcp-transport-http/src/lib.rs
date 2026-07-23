//! Streamable HTTP MCP client transport with SSRF controls.

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use futures::StreamExt;
use gateway_mcp::{
    McpConnectionContext, McpError, McpHealth, McpHealthStatus, McpSession, McpTransport,
};
use gateway_types::{GatewayMcpInvocation, GatewayMcpResult, GatewayMcpTool, McpContentPart};
use reqwest::{
    Client, StatusCode, Url,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct HttpMcpTransport {
    state: Arc<RwLock<Option<HttpState>>>,
    timeout: Duration,
    allow_private: bool,
    maximum_response_bytes: usize,
}

#[derive(Clone)]
struct HttpState {
    client: Client,
    endpoint: Url,
    bearer: Option<String>,
    session_id: Option<String>,
}

impl HttpMcpTransport {
    pub fn new(
        timeout: Duration,
        allow_private: bool,
        maximum_response_bytes: usize,
    ) -> Result<Self, McpError> {
        Ok(Self {
            timeout,
            state: Default::default(),
            allow_private,
            maximum_response_bytes,
        })
    }

    async fn request(&self, method: &str, params: Value) -> Result<(Value, HeaderMap), McpError> {
        let state = self.state.read().await.clone().ok_or(McpError::Transport)?;
        validate_endpoint(&state.endpoint, self.allow_private).await?;
        let mut request = state.client.post(state.endpoint).header(CONTENT_TYPE, "application/json").header(ACCEPT, "application/json, text/event-stream").header("MCP-Protocol-Version", "2025-11-25").json(&json!({"jsonrpc":"2.0","id":Uuid::new_v4().to_string(),"method":method,"params":params}));
        if let Some(session) = state.session_id {
            request = request.header("MCP-Session-Id", session);
        }
        if let Some(bearer) = state.bearer {
            request = request.header(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {bearer}"))
                    .map_err(|_| McpError::Invalid)?,
            );
        }
        let response = request.send().await.map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(if response.status() == StatusCode::NOT_FOUND {
                McpError::ToolUnavailable
            } else {
                McpError::Transport
            });
        }
        let headers = response.headers().clone();
        if response
            .content_length()
            .is_some_and(|length| length > self.maximum_response_bytes as u64)
        {
            return Err(McpError::TooLarge);
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_reqwest)?;
            if bytes.len().saturating_add(chunk.len()) > self.maximum_response_bytes {
                return Err(McpError::TooLarge);
            }
            bytes.extend_from_slice(&chunk)
        }
        let value: Value = if headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream"))
        {
            parse_sse(&bytes)?
        } else {
            serde_json::from_slice(&bytes).map_err(|_| McpError::Invalid)?
        };
        if value.get("error").is_some() {
            return Err(McpError::Transport);
        }
        Ok((value.get("result").cloned().unwrap_or(Value::Null), headers))
    }
}

#[async_trait]
impl McpTransport for HttpMcpTransport {
    async fn initialize(&self, context: McpConnectionContext) -> Result<McpSession, McpError> {
        let endpoint = Url::parse(
            context
                .server
                .endpoint
                .as_deref()
                .ok_or(McpError::Invalid)?,
        )
        .map_err(|_| McpError::Invalid)?;
        let addresses = endpoint_addresses(&endpoint, self.allow_private).await?;
        let host = endpoint.host_str().ok_or(McpError::Invalid)?;
        let client = Client::builder()
            .timeout(self.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| McpError::Transport)?;
        *self.state.write().await = Some(HttpState {
            client,
            endpoint,
            bearer: context.credential.map(|value| value.expose().to_owned()),
            session_id: None,
        });
        let (_, headers) = self.request("initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"tuenel-gateway","version":"0.3"}})).await?;
        let remote_session_id = headers
            .get("MCP-Session-Id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if let Some(state) = self.state.write().await.as_mut() {
            state.session_id = remote_session_id.clone();
        }
        let session = McpSession {
            session_id: Uuid::now_v7(),
            server_id: context.server.server_id,
            protocol_version: "2025-11-25".into(),
            remote_session_id,
            created_at: chrono::Utc::now(),
            expires_at: None,
        };
        let _ = self.request("notifications/initialized", json!({})).await;
        Ok(session)
    }

    async fn list_tools(&self, session: &McpSession) -> Result<Vec<GatewayMcpTool>, McpError> {
        let (result, _) = self.request("tools/list", json!({})).await?;
        result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or(McpError::Invalid)?
            .iter()
            .map(|tool| {
                Ok(GatewayMcpTool {
                    server_id: session.server_id,
                    tool_name: string(tool, "name")?,
                    description: tool
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    input_schema: tool
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or_else(|| json!({"type":"object"})),
                    annotations: serde_json::from_value(
                        tool.get("annotations")
                            .cloned()
                            .unwrap_or_else(|| json!({})),
                    )
                    .unwrap_or_default(),
                })
            })
            .collect()
    }

    async fn invoke_tool(
        &self,
        _: &McpSession,
        invocation: GatewayMcpInvocation,
    ) -> Result<GatewayMcpResult, McpError> {
        let (result, _) = self
            .request(
                "tools/call",
                json!({"name":invocation.tool_name,"arguments":invocation.arguments}),
            )
            .await?;
        let content = result
            .get("content")
            .and_then(Value::as_array)
            .ok_or(McpError::Invalid)?
            .iter()
            .filter_map(content_part)
            .collect();
        Ok(GatewayMcpResult {
            content,
            is_error: result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            metadata: Default::default(),
        })
    }

    async fn health_check(&self) -> Result<McpHealth, McpError> {
        let started = Instant::now();
        self.request("ping", json!({})).await?;
        Ok(McpHealth {
            status: McpHealthStatus::Healthy,
            latency_ms: Some(started.elapsed().as_millis() as u64),
            checked_at: chrono::Utc::now(),
        })
    }

    async fn shutdown(&self) -> Result<(), McpError> {
        let state = self.state.read().await.clone();
        if let Some(state) = state.filter(|state| state.session_id.is_some()) {
            let mut request = state
                .client
                .delete(state.endpoint)
                .header("MCP-Protocol-Version", "2025-11-25")
                .header("MCP-Session-Id", state.session_id.unwrap_or_default());
            if let Some(bearer) = state.bearer {
                request = request.bearer_auth(bearer);
            }
            let _ = request.send().await;
        }
        *self.state.write().await = None;
        Ok(())
    }
}

pub async fn validate_endpoint(endpoint: &Url, allow_private: bool) -> Result<(), McpError> {
    endpoint_addresses(endpoint, allow_private)
        .await
        .map(|_| ())
}
async fn endpoint_addresses(
    endpoint: &Url,
    allow_private: bool,
) -> Result<Vec<SocketAddr>, McpError> {
    if !matches!(endpoint.scheme(), "http" | "https")
        || endpoint.username() != ""
        || endpoint.password().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(McpError::Invalid);
    }
    let host = endpoint.host_str().ok_or(McpError::Invalid)?;
    let port = endpoint.port_or_known_default().ok_or(McpError::Invalid)?;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| McpError::Transport)?
        .collect::<Vec<_>>();
    if addresses.is_empty()
        || (!allow_private && addresses.iter().any(|address| forbidden(address.ip())))
    {
        return Err(McpError::ServerNotAllowed);
    }
    Ok(addresses)
}

fn forbidden(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => {
            let octets = value.octets();
            value.is_private()
                || value.is_loopback()
                || value.is_link_local()
                || value.is_broadcast()
                || value.is_documentation()
                || value.is_unspecified()
                || value.is_multicast()
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 198 && (18..=19).contains(&octets[1]))
                || octets[0] >= 240
        }
        IpAddr::V6(value) => {
            value.is_loopback()
                || value.is_unspecified()
                || value.is_unique_local()
                || value.is_unicast_link_local()
                || value.is_multicast()
        }
    }
}
fn map_reqwest(error: reqwest::Error) -> McpError {
    if error.is_timeout() {
        McpError::Timeout
    } else {
        McpError::Transport
    }
}
fn string(value: &Value, field: &str) -> Result<String, McpError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(McpError::Invalid)
}
fn parse_sse(bytes: &[u8]) -> Result<Value, McpError> {
    std::str::from_utf8(bytes)
        .map_err(|_| McpError::Invalid)?
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .filter(|value| *value != "[DONE]")
        .find_map(|value| {
            serde_json::from_str::<Value>(value)
                .ok()
                .filter(|item| item.get("result").is_some() || item.get("error").is_some())
        })
        .ok_or(McpError::Invalid)
}
fn content_part(value: &Value) -> Option<McpContentPart> {
    match value.get("type")?.as_str()? {
        "text" => Some(McpContentPart::Text {
            text: value.get("text")?.as_str()?.to_owned(),
        }),
        "resource" => Some(McpContentPart::Resource {
            uri: value.pointer("/resource/uri")?.as_str()?.to_owned(),
            text: value
                .pointer("/resource/text")
                .and_then(Value::as_str)
                .map(str::to_owned),
            mime_type: value
                .pointer("/resource/mimeType")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        "image" | "audio" => Some(McpContentPart::Binary {
            mime_type: value.get("mimeType")?.as_str()?.to_owned(),
            data_base64: value.get("data")?.as_str()?.to_owned(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn denies_loopback_unless_explicitly_allowed() {
        let endpoint = Url::parse("http://127.0.0.1:1234/mcp").unwrap();
        assert_eq!(
            validate_endpoint(&endpoint, false).await.unwrap_err(),
            McpError::ServerNotAllowed
        );
        assert!(validate_endpoint(&endpoint, true).await.is_ok())
    }
    #[test]
    fn rejects_credentials_in_endpoint() {
        let endpoint = Url::parse("https://user:pass@example.com/mcp").unwrap();
        assert!(matches!(
            futures::executor::block_on(validate_endpoint(&endpoint, false)),
            Err(McpError::Invalid)
        ))
    }
}
