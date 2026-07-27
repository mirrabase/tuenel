//! Environment configuration for gateway v0.3.

use std::{env, net::SocketAddr, str::FromStr, time::Duration};

use base64::Engine as _;
use rust_decimal::Decimal;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use url::Url;

const DEVELOPMENT_MASTER_KEY: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";

/// Fully validated gateway settings.
#[derive(Clone, Debug)]
pub struct Settings {
    /// HTTP listen address.
    pub bind_addr: SocketAddr,
    /// PostgreSQL connection URL.
    pub database_url: SecretString,
    /// Redis URL used only for counters, reservations, and caches.
    pub redis_url: SecretString,
    /// Base64-encoded 32-byte credential encryption key.
    pub credentials_master_key: SecretString,
    /// Expected JWT issuer.
    pub oidc_issuer: Option<String>,
    /// Expected JWT audience.
    pub oidc_audience: Option<String>,
    /// JWKS endpoint for the issuer.
    pub oidc_jwks_url: Option<Url>,
    /// Role required by tenant administration routes.
    pub oidc_admin_role: String,
    /// OpenAI-compatible upstream base URL.
    pub upstream_base_url: Option<Url>,
    /// Optional upstream bearer key.
    pub upstream_api_key: Option<SecretString>,
    /// Public model alias.
    pub model_alias: String,
    /// Upstream model name.
    pub upstream_model: Option<String>,
    pub anthropic_base_url: Url,
    pub anthropic_api_key: Option<SecretString>,
    pub anthropic_model: Option<String>,
    pub gemini_base_url: Url,
    pub gemini_api_key: Option<SecretString>,
    pub gemini_model: Option<String>,
    /// Request deadline.
    pub request_timeout: Duration,
    /// Maximum accepted output-token request.
    pub max_output_tokens: u32,
    /// Default output-token allowance.
    pub default_output_tokens: u32,
    /// Maximum JSON request body size.
    pub max_body_bytes: usize,
    /// Input price in USD per million tokens.
    pub input_cost_per_million: Decimal,
    /// Output price in USD per million tokens.
    pub output_cost_per_million: Decimal,
    /// Default Virtual Key daily token limit.
    pub default_virtual_key_daily_tokens: u64,
    /// Reservation lifetime.
    pub reservation_ttl: Duration,
    pub mcp_enabled: bool,
    pub mcp_discovery_cache: Duration,
    pub mcp_maximum_schema_bytes: usize,
    pub mcp_tool_timeout: Duration,
    pub mcp_allow_private_http_endpoints: bool,
    pub mcp_allowed_stdio_commands: Vec<String>,
    pub mcp_maximum_response_bytes: usize,
    pub approval_enabled: bool,
    pub approval_expiration: Duration,
    pub security_enabled: bool,
}

impl Settings {
    /// Load and validate settings from process environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let settings = Self {
            bind_addr: parse_or("GATEWAY_BIND_ADDR", "0.0.0.0:8080")?,
            database_url: secret(required("DATABASE_URL")?),
            redis_url: secret(required("REDIS_URL")?),
            credentials_master_key: secret(required("GATEWAY_CREDENTIALS_MASTER_KEY")?),
            oidc_issuer: optional("OIDC_ISSUER"),
            oidc_audience: optional("OIDC_AUDIENCE"),
            oidc_jwks_url: parse_optional("OIDC_JWKS_URL")?,
            oidc_admin_role: value_or("OIDC_ADMIN_ROLE", "gateway_admin"),
            upstream_base_url: parse_optional("UPSTREAM_BASE_URL")?,
            upstream_api_key: env::var("UPSTREAM_API_KEY")
                .ok()
                .filter(|value| !value.is_empty())
                .map(secret),
            model_alias: value_or("GATEWAY_MODEL_ALIAS", "gateway-default"),
            upstream_model: optional("UPSTREAM_MODEL"),
            anthropic_base_url: parse_or("ANTHROPIC_BASE_URL", "https://api.anthropic.com/")?,
            anthropic_api_key: optional_secret("ANTHROPIC_API_KEY"),
            anthropic_model: optional("ANTHROPIC_MODEL"),
            gemini_base_url: parse_or(
                "GEMINI_BASE_URL",
                "https://generativelanguage.googleapis.com/",
            )?,
            gemini_api_key: optional_secret("GEMINI_API_KEY"),
            gemini_model: optional("GEMINI_MODEL"),
            request_timeout: Duration::from_secs(parse_or("REQUEST_TIMEOUT_SECONDS", "120")?),
            max_output_tokens: parse_or("MAX_OUTPUT_TOKENS", "4096")?,
            default_output_tokens: parse_or("DEFAULT_OUTPUT_TOKENS", "1024")?,
            max_body_bytes: parse_or("MAX_BODY_BYTES", "1048576")?,
            input_cost_per_million: parse_or("INPUT_COST_PER_MILLION_USD", "0")?,
            output_cost_per_million: parse_or("OUTPUT_COST_PER_MILLION_USD", "0")?,
            default_virtual_key_daily_tokens: parse_or(
                "DEFAULT_VIRTUAL_KEY_DAILY_TOKENS",
                "100000",
            )?,
            reservation_ttl: Duration::from_secs(parse_or("QUOTA_RESERVATION_TTL_SECONDS", "300")?),
            mcp_enabled: parse_or("MCP_ENABLED", "true")?,
            mcp_discovery_cache: Duration::from_secs(parse_or(
                "MCP_DISCOVERY_CACHE_SECONDS",
                "300",
            )?),
            mcp_maximum_schema_bytes: parse_or("MCP_MAXIMUM_SCHEMA_BYTES", "262144")?,
            mcp_tool_timeout: Duration::from_secs(parse_or(
                "MCP_DEFAULT_TOOL_TIMEOUT_SECONDS",
                "30",
            )?),
            mcp_allow_private_http_endpoints: parse_or(
                "MCP_ALLOW_PRIVATE_HTTP_ENDPOINTS",
                "false",
            )?,
            mcp_allowed_stdio_commands: value_or("MCP_ALLOWED_STDIO_COMMANDS", "node,python,uvx")
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
            mcp_maximum_response_bytes: parse_or("MCP_MAXIMUM_RESPONSE_BYTES", "1048576")?,
            approval_enabled: parse_or("APPROVAL_ENABLED", "true")?,
            approval_expiration: Duration::from_secs(parse_or(
                "APPROVAL_DEFAULT_EXPIRATION_SECONDS",
                "900",
            )?),
            security_enabled: parse_or("SECURITY_ENABLED", "true")?,
        };

        if settings.default_output_tokens == 0
            || settings.default_output_tokens > settings.max_output_tokens
        {
            return Err(ConfigError::Invalid(
                "DEFAULT_OUTPUT_TOKENS must be between 1 and MAX_OUTPUT_TOKENS".into(),
            ));
        }
        complete_group(
            "OIDC_ISSUER, OIDC_AUDIENCE, and OIDC_JWKS_URL",
            &[
                settings.oidc_issuer.is_some(),
                settings.oidc_audience.is_some(),
                settings.oidc_jwks_url.is_some(),
            ],
        )?;
        complete_group(
            "UPSTREAM_BASE_URL and UPSTREAM_MODEL",
            &[
                settings.upstream_base_url.is_some(),
                settings.upstream_model.is_some(),
            ],
        )?;
        if settings
            .upstream_base_url
            .as_ref()
            .is_some_and(|url| !matches!(url.scheme(), "http" | "https"))
        {
            return Err(ConfigError::Invalid(
                "UPSTREAM_BASE_URL must use http or https".into(),
            ));
        }
        for (name, url) in [
            ("ANTHROPIC_BASE_URL", &settings.anthropic_base_url),
            ("GEMINI_BASE_URL", &settings.gemini_base_url),
        ] {
            if !matches!(url.scheme(), "http" | "https") {
                return Err(ConfigError::Invalid(format!(
                    "{name} must use http or https"
                )));
            }
        }
        for name in ["DATABASE_URL", "REDIS_URL"] {
            let value = if name == "DATABASE_URL" {
                settings.database_url.expose_secret()
            } else {
                settings.redis_url.expose_secret()
            };
            Url::parse(value)
                .map_err(|_| ConfigError::Invalid(format!("{name} must be a valid URL")))?;
        }
        let key = base64::engine::general_purpose::STANDARD
            .decode(settings.credentials_master_key.expose_secret())
            .map_err(|_| {
                ConfigError::Invalid("GATEWAY_CREDENTIALS_MASTER_KEY must be base64".into())
            })?;
        if key.len() != 32 {
            return Err(ConfigError::Invalid(
                "GATEWAY_CREDENTIALS_MASTER_KEY must decode to 32 bytes".into(),
            ));
        }
        if settings.max_body_bytes == 0
            || settings.mcp_maximum_schema_bytes == 0
            || settings.mcp_maximum_response_bytes == 0
            || settings.mcp_allowed_stdio_commands.is_empty()
            || settings.mcp_tool_timeout.is_zero()
            || settings.approval_expiration.is_zero()
        {
            return Err(ConfigError::Invalid(
                "body, MCP, and approval limits must be non-zero".into(),
            ));
        }
        Ok(settings)
    }

    /// Warn when the public development key is used without exposing it.
    pub fn development_secret_warning(&self) -> Option<&'static str> {
        development_secret_warning(self.credentials_master_key.expose_secret())
    }
}

/// Configuration loading failure.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required environment variable was absent or empty.
    #[error("missing required environment variable {0}")]
    Missing(&'static str),
    /// A value could not be parsed or violated an invariant.
    #[error("invalid configuration: {0}")]
    Invalid(String),
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ConfigError::Missing(name))
}

fn value_or(name: &'static str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn parse_optional<T>(name: &'static str) -> Result<Option<T>, ConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    optional(name)
        .map(|value| {
            value.parse().map_err(|error: T::Err| {
                ConfigError::Invalid(format!("{name} could not be parsed: {error}"))
            })
        })
        .transpose()
}

fn parse_or<T>(name: &'static str, default: &str) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    value_or(name, default).parse().map_err(|error: T::Err| {
        ConfigError::Invalid(format!("{name} could not be parsed: {error}"))
    })
}

fn secret(value: String) -> SecretString {
    SecretString::from(value)
}

fn optional(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}
fn optional_secret(name: &'static str) -> Option<SecretString> {
    optional(name).map(secret)
}

fn development_secret_warning(value: &str) -> Option<&'static str> {
    (value == DEVELOPMENT_MASTER_KEY).then_some(
        "GATEWAY_CREDENTIALS_MASTER_KEY uses the known development example; replace it before production",
    )
}

fn complete_group(name: &str, configured: &[bool]) -> Result<(), ConfigError> {
    if configured.iter().any(|value| *value) && !configured.iter().all(|value| *value) {
        return Err(ConfigError::Invalid(format!(
            "{name} must be configured together"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_secret_warning_never_contains_the_secret() {
        let warning = development_secret_warning(DEVELOPMENT_MASTER_KEY).unwrap();
        assert!(warning.contains("GATEWAY_CREDENTIALS_MASTER_KEY"));
        assert!(!warning.contains(DEVELOPMENT_MASTER_KEY));
    }

    #[test]
    fn optional_configuration_groups_reject_partial_values() {
        assert!(complete_group("OIDC", &[false, false, false]).is_ok());
        assert!(complete_group("OIDC", &[true, true, true]).is_ok());
        assert!(complete_group("OIDC", &[true, false, true]).is_err());
    }
}
