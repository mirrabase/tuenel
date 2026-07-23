//! Redis-backed bounded counters and concurrency reservations.

use async_trait::async_trait;
use gateway_mcp::{McpError, McpPolicy, McpQuota, McpQuotaReservation};
use gateway_quota::{InferenceQuotaCounter, QuotaError};
use gateway_types::{McpServerId, Principal};

#[derive(Clone)]
pub struct RedisQuotaStore {
    client: redis::Client,
    default_concurrent: u64,
}

impl RedisQuotaStore {
    pub fn new(url: &str, default_concurrent: u64) -> Result<Self, RedisQuotaError> {
        Ok(Self {
            client: redis::Client::open(url).map_err(|_| RedisQuotaError::Unavailable)?,
            default_concurrent: default_concurrent.max(1),
        })
    }
    pub async fn ping(&self) -> Result<(), RedisQuotaError> {
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| RedisQuotaError::Unavailable)?;
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
            .map_err(|_| RedisQuotaError::Unavailable)
            .map(|_| ())
    }
}

#[async_trait]
impl McpQuota for RedisQuotaStore {
    async fn reserve(
        &self,
        principal: &Principal,
        server_id: McpServerId,
        tool_name: &str,
        policy: &McpPolicy,
    ) -> Result<McpQuotaReservation, McpError> {
        let minute = chrono_like_minute();
        let day = chrono_like_day();
        let rpm_key = format!(
            "mcp:rpm:{}:{}:{minute}",
            principal.tenant_id, principal.principal_id
        );
        let daily_key = format!(
            "mcp:daily:{}:{}:{day}",
            principal.tenant_id, principal.principal_id
        );
        let server_key = format!("mcp:concurrent:{}:{server_id}", principal.tenant_id);
        let tool_key = format!(
            "mcp:concurrent:{}:{server_id}:{}",
            principal.tenant_id,
            safe_key(tool_name)
        );
        let limit = policy.maximum_calls_per_minute.unwrap_or(1_000_000);
        let server_concurrent = policy
            .maximum_server_concurrent_calls
            .unwrap_or(self.default_concurrent);
        let tool_concurrent = policy
            .maximum_tool_concurrent_calls
            .unwrap_or(self.default_concurrent);
        let script = redis::Script::new(
            r#"
            local rpm = redis.call('INCR', KEYS[1])
            if rpm == 1 then redis.call('EXPIRE', KEYS[1], 120) end
            if rpm > tonumber(ARGV[1]) then redis.call('DECR', KEYS[1]); return 0 end
            local daily = redis.call('INCR', KEYS[4])
            if daily == 1 then redis.call('EXPIRE', KEYS[4], 172800) end
            if daily > tonumber(ARGV[4]) then redis.call('DECR', KEYS[1]); redis.call('DECR', KEYS[4]); return 0 end
            local server = redis.call('INCR', KEYS[2])
            if server == 1 then redis.call('EXPIRE', KEYS[2], tonumber(ARGV[3])) end
            if server > tonumber(ARGV[2]) then redis.call('DECR', KEYS[1]); redis.call('DECR', KEYS[2]); redis.call('DECR', KEYS[4]); return 0 end
            local tool = redis.call('INCR', KEYS[3])
            if tool == 1 then redis.call('EXPIRE', KEYS[3], tonumber(ARGV[3])) end
            if tool > tonumber(ARGV[5]) then redis.call('DECR', KEYS[1]); redis.call('DECR', KEYS[2]); redis.call('DECR', KEYS[3]); redis.call('DECR', KEYS[4]); return 0 end
            return 1
        "#,
        );
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| McpError::Unavailable)?;
        let accepted: i32 = script
            .key(&rpm_key)
            .key(&server_key)
            .key(&tool_key)
            .key(&daily_key)
            .arg(limit)
            .arg(server_concurrent)
            .arg(policy.maximum_execution_ms.unwrap_or(30_000) / 1_000 + 30)
            .arg(policy.maximum_calls_per_day.unwrap_or(1_000_000))
            .arg(tool_concurrent)
            .invoke_async(&mut connection)
            .await
            .map_err(|_| McpError::Unavailable)?;
        if accepted != 1 {
            return Err(McpError::QuotaExceeded);
        }
        Ok(McpQuotaReservation {
            key: format!("{server_key}\n{tool_key}"),
        })
    }

    async fn release(&self, reservation: McpQuotaReservation) -> Result<(), McpError> {
        let mut keys = reservation.key.splitn(2, '\n');
        let server = keys.next().ok_or(McpError::Invalid)?;
        let tool = keys.next().ok_or(McpError::Invalid)?;
        let script = redis::Script::new(
            "for i,key in ipairs(KEYS) do local value=redis.call('GET',key); if value and tonumber(value)>0 then redis.call('DECR',key) end end return 1",
        );
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| McpError::Unavailable)?;
        script
            .key(server)
            .key(tool)
            .invoke_async::<i32>(&mut connection)
            .await
            .map_err(|_| McpError::Unavailable)
            .map(|_| ())
    }
}

fn chrono_like_minute() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 60
}
fn chrono_like_day() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400
}
fn safe_key(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
                char::from(byte)
            } else {
                '_'
            }
        })
        .take(128)
        .collect()
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum RedisQuotaError {
    #[error("Redis quota store unavailable")]
    Unavailable,
}

#[async_trait]
impl InferenceQuotaCounter for RedisQuotaStore {
    async fn reserve(&self, principal: &Principal) -> Result<String, QuotaError> {
        let minute = chrono_like_minute();
        let rpm = format!(
            "inference:rpm:{}:{}:{minute}",
            principal.tenant_id, principal.principal_id
        );
        let concurrent = format!(
            "inference:concurrent:{}:{}",
            principal.tenant_id, principal.principal_id
        );
        let script = redis::Script::new(
            r#"local rpm=redis.call('INCR',KEYS[1]);if rpm==1 then redis.call('EXPIRE',KEYS[1],120) end;if rpm>600 then redis.call('DECR',KEYS[1]);return 0 end;local active=redis.call('INCR',KEYS[2]);if active==1 then redis.call('EXPIRE',KEYS[2],300) end;if active>tonumber(ARGV[1]) then redis.call('DECR',KEYS[1]);redis.call('DECR',KEYS[2]);return 0 end;return 1"#,
        );
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| QuotaError::Unavailable)?;
        let accepted: i32 = script
            .key(&rpm)
            .key(&concurrent)
            .arg(self.default_concurrent)
            .invoke_async(&mut connection)
            .await
            .map_err(|_| QuotaError::Unavailable)?;
        if accepted == 1 {
            Ok(concurrent)
        } else {
            Err(QuotaError::Exceeded)
        }
    }
    async fn release(&self, reservation_key: &str) -> Result<(), QuotaError> {
        let script = redis::Script::new(
            "local value=redis.call('GET',KEYS[1]);if value and tonumber(value)>0 then return redis.call('DECR',KEYS[1]) end return 0",
        );
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| QuotaError::Unavailable)?;
        script
            .key(reservation_key)
            .invoke_async::<i32>(&mut connection)
            .await
            .map_err(|_| QuotaError::Unavailable)
            .map(|_| ())
    }
}
