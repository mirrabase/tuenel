use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum McpError {
    #[error("MCP server not found")]
    ServerNotFound,
    #[error("MCP server is not allowed")]
    ServerNotAllowed,
    #[error("MCP tool is not allowed")]
    ToolNotAllowed,
    #[error("MCP tool is unavailable")]
    ToolUnavailable,
    #[error("MCP quota exceeded")]
    QuotaExceeded,
    #[error("MCP invocation requires approval")]
    ApprovalRequired(gateway_types::ApprovalId),
    #[error("MCP invocation approval was rejected")]
    ApprovalRejected,
    #[error("MCP invocation approval expired")]
    ApprovalExpired,
    #[error("MCP transport failed")]
    Transport,
    #[error("MCP request timed out")]
    Timeout,
    #[error("MCP payload is invalid")]
    Invalid,
    #[error("MCP payload is too large")]
    TooLarge,
    #[error("MCP persistence unavailable")]
    Unavailable,
}
