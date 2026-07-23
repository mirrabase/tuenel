//! Restricted stdio MCP client transport.

use std::{
    collections::HashSet,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use gateway_mcp::{
    McpConnectionContext, McpError, McpHealth, McpHealthStatus, McpSession, McpTransport,
};
use gateway_types::{GatewayMcpInvocation, GatewayMcpResult, GatewayMcpTool, McpContentPart};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
    time::timeout,
};
use uuid::Uuid;

pub struct StdioMcpTransport {
    allowed_commands: HashSet<String>,
    deadline: Duration,
    maximum_response_bytes: usize,
    state: Arc<Mutex<Option<ProcessState>>>,
    next_id: AtomicU64,
}
struct ProcessState {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

impl StdioMcpTransport {
    pub fn new(
        allowed_commands: impl IntoIterator<Item = String>,
        deadline: Duration,
        maximum_response_bytes: usize,
    ) -> Self {
        Self {
            allowed_commands: allowed_commands.into_iter().collect(),
            deadline,
            maximum_response_bytes,
            state: Default::default(),
            next_id: AtomicU64::new(1),
        }
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let mut guard = self.state.lock().await;
        let state = guard.as_mut().ok_or(McpError::Transport)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut encoded =
            serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
                .map_err(|_| McpError::Invalid)?;
        encoded.push(b'\n');
        timeout(self.deadline, state.stdin.write_all(&encoded))
            .await
            .map_err(|_| McpError::Timeout)?
            .map_err(|_| McpError::Transport)?;
        loop {
            let line = timeout(self.deadline, state.stdout.next_line())
                .await
                .map_err(|_| McpError::Timeout)?
                .map_err(|_| McpError::Transport)?
                .ok_or(McpError::Transport)?;
            if line.len() > self.maximum_response_bytes {
                return Err(McpError::TooLarge);
            }
            let value: Value = serde_json::from_str(&line).map_err(|_| McpError::Invalid)?;
            if value.get("id") == Some(&json!(id)) {
                if value.get("error").is_some() {
                    return Err(McpError::Transport);
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }
}

pub fn validate_command(command: &str, allowed_commands: &[String]) -> Result<(), McpError> {
    let executable = std::path::Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(McpError::Invalid)?;
    if allowed_commands
        .iter()
        .any(|allowed| allowed == command || allowed == executable)
    {
        Ok(())
    } else {
        Err(McpError::ServerNotAllowed)
    }
}

#[async_trait]
impl McpTransport for StdioMcpTransport {
    async fn initialize(&self, context: McpConnectionContext) -> Result<McpSession, McpError> {
        let command = context.server.command.as_deref().ok_or(McpError::Invalid)?;
        let executable = std::path::Path::new(command)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(McpError::Invalid)?;
        if !self.allowed_commands.contains(command) && !self.allowed_commands.contains(executable) {
            return Err(McpError::ServerNotAllowed);
        }
        let mut process = Command::new(command);
        process
            .args(&context.server.arguments)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (name, value) in context.environment {
            if !valid_environment_name(&name) {
                return Err(McpError::Invalid);
            }
            process.env(name, value.expose());
        }
        let mut child = process.spawn().map_err(|_| McpError::Transport)?;
        let stdin = child.stdin.take().ok_or(McpError::Transport)?;
        let stdout = child.stdout.take().ok_or(McpError::Transport)?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while lines.next_line().await.ok().flatten().is_some() {}
            });
        }
        *self.state.lock().await = Some(ProcessState {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
        });
        self.request("initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"tuenel-gateway","version":"0.3"}})).await?;
        let _ = self.request("notifications/initialized", json!({})).await;
        Ok(McpSession {
            session_id: Uuid::now_v7(),
            server_id: context.server.server_id,
            protocol_version: "2025-11-25".into(),
            remote_session_id: None,
            created_at: chrono::Utc::now(),
            expires_at: None,
        })
    }

    async fn list_tools(&self, session: &McpSession) -> Result<Vec<GatewayMcpTool>, McpError> {
        let result = self.request("tools/list", json!({})).await?;
        result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or(McpError::Invalid)?
            .iter()
            .map(|tool| {
                Ok(GatewayMcpTool {
                    server_id: session.server_id,
                    tool_name: tool
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or(McpError::Invalid)?
                        .to_owned(),
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
        let result = self
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
            .filter_map(|value| match value.get("type")?.as_str()? {
                "text" => Some(McpContentPart::Text {
                    text: value.get("text")?.as_str()?.to_owned(),
                }),
                _ => None,
            })
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
        if let Some(mut state) = self.state.lock().await.take() {
            drop(state.stdin);
            if timeout(Duration::from_secs(2), state.child.wait())
                .await
                .is_err()
            {
                state.child.kill().await.map_err(|_| McpError::Transport)?;
                let _ = state.child.wait().await;
            }
        }
        Ok(())
    }
}

fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        && !value.as_bytes()[0].is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_and_environment_allowlists_are_strict() {
        let allowed = vec!["node".into(), "python".into()];
        assert!(validate_command("node", &allowed).is_ok());
        assert_eq!(
            validate_command("sh", &allowed),
            Err(McpError::ServerNotAllowed)
        );
        assert!(valid_environment_name("SAFE_VALUE"));
        assert!(!valid_environment_name("1BAD"));
        assert!(!valid_environment_name("BAD-NAME"));
    }
}
