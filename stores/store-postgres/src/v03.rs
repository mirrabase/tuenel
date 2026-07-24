use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_approval::{ApprovalDecision, ApprovalError, ApprovalRepository, ExecutionClaim};
use gateway_billing::{BillingDelivery, BillingError, BillingRepository};
use gateway_events::{AuditEvent, AuditRepository, EventError};
use gateway_incidents::{IncidentError, IncidentRepository, IncidentTimelineEntry};
use gateway_mcp::{
    McpError, McpHealth, McpHealthRepository, McpHealthStatus, McpPolicy, McpPolicyAdministration,
    McpPolicyRecord, McpPolicyRepository, McpRepository, McpServerRecord, McpUsageEvent,
    McpUsageRepository,
};
use gateway_policy::{Policy, PolicyError, PolicyResolver};
use gateway_pricing::{ModelPrice, PricingCatalog, PricingError};
use gateway_providers::{
    ProviderError, ProviderHealth, ProviderHealthRepository, ProviderHealthStatus,
};
use gateway_secrets::{SecretError, SecretMaterialRecord, SecretRepository};
use gateway_types::{
    ApprovalId, ApprovalRequest, ApprovalResourceType, ApprovalStatus, GatewayMcpTool, IncidentId,
    IncidentStatus, McpServerId, McpTransportType, Principal, SecretRef, SecurityCategory,
    SecurityIncident, SecuritySeverity, ToolRiskLevel,
};
use sqlx::Row;
use uuid::Uuid;

use super::PostgresStore;

impl PostgresStore {
    pub async fn upsert_provider(
        &self,
        provider_id: &str,
        provider_type: &str,
        base_url: &str,
    ) -> Result<(), gateway_store::StoreError> {
        sqlx::query("INSERT INTO providers (id,provider_type,base_url,enabled) VALUES ($1,$2,$3,true) ON CONFLICT (id) DO UPDATE SET provider_type=EXCLUDED.provider_type,base_url=EXCLUDED.base_url,enabled=true,updated_at=now()").bind(provider_id).bind(provider_type).bind(base_url).execute(&self.pool).await.map_err(|_|gateway_store::StoreError::Unavailable).map(|_|())
    }
}

#[async_trait]
impl AuditRepository for PostgresStore {
    async fn append(&self, event: AuditEvent) -> Result<(), EventError> {
        sqlx::query("INSERT INTO audit_events(event_id,idempotency_key,tenant_id,project_id,principal_id,request_id,event_type,payload,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(idempotency_key) DO NOTHING")
            .bind(event.event_id).bind(event.idempotency_key).bind(event.tenant_id).bind(event.project_id).bind(event.principal_id).bind(event.request_id).bind(event.event_type).bind(event.payload).bind(event.occurred_at)
            .execute(&self.pool).await.map(|_| ()).map_err(|_| EventError::Unavailable)
    }
}

#[async_trait]
impl SecretRepository for PostgresStore {
    async fn insert_secret(&self, record: SecretMaterialRecord) -> Result<(), SecretError> {
        sqlx::query("INSERT INTO secret_material (secret_ref, tenant_id, purpose, nonce, ciphertext) VALUES ($1,$2,$3,$4,$5)").bind(record.secret_ref.0).bind(record.tenant_id).bind(record.purpose).bind(record.nonce).bind(record.ciphertext).execute(&self.pool).await.map_err(|_| SecretError::Unavailable).map(|_| ())
    }
    async fn secret(
        &self,
        tenant_id: &str,
        secret_ref: &SecretRef,
    ) -> Result<Option<SecretMaterialRecord>, SecretError> {
        sqlx::query("SELECT secret_ref, tenant_id, purpose, nonce, ciphertext FROM secret_material WHERE tenant_id=$1 AND secret_ref=$2").bind(tenant_id).bind(&secret_ref.0).fetch_optional(&self.pool).await.map_err(|_| SecretError::Unavailable)?.map(|row| Ok(SecretMaterialRecord { secret_ref: SecretRef(row.try_get("secret_ref").map_err(|_| SecretError::Unavailable)?), tenant_id: row.try_get("tenant_id").map_err(|_| SecretError::Unavailable)?, purpose: row.try_get("purpose").map_err(|_| SecretError::Unavailable)?, nonce: row.try_get("nonce").map_err(|_| SecretError::Unavailable)?, ciphertext: row.try_get("ciphertext").map_err(|_| SecretError::Unavailable)? })).transpose()
    }
    async fn delete_secret(
        &self,
        tenant_id: &str,
        secret_ref: &SecretRef,
    ) -> Result<(), SecretError> {
        sqlx::query("DELETE FROM secret_material WHERE tenant_id=$1 AND secret_ref=$2")
            .bind(tenant_id)
            .bind(&secret_ref.0)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| SecretError::Unavailable)
    }
}

#[async_trait]
impl BillingRepository for PostgresStore {
    async fn claim_due(&self, now: DateTime<Utc>) -> Result<Option<BillingDelivery>, BillingError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| BillingError::Unavailable)?;
        let row = sqlx::query("SELECT o.event_id,o.tenant_id,o.payload,o.attempt_count,w.url,w.secret_ref,w.maximum_attempts FROM billing_outbox o JOIN billing_webhooks w ON w.webhook_id=o.webhook_id WHERE o.delivered_at IS NULL AND o.next_attempt_at<=$1 AND o.attempt_count<w.maximum_attempts AND w.enabled=true AND w.secret_ref IS NOT NULL ORDER BY o.next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1").bind(now).fetch_optional(&mut *transaction).await.map_err(|_| BillingError::Unavailable)?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|_| BillingError::Unavailable)?;
            return Ok(None);
        };
        let event_id: Uuid = row
            .try_get("event_id")
            .map_err(|_| BillingError::Unavailable)?;
        let attempt_count: i32 = row
            .try_get("attempt_count")
            .map_err(|_| BillingError::Unavailable)?;
        let delay_seconds =
            2_i64.saturating_pow(u32::try_from(attempt_count).unwrap_or(16).min(16));
        sqlx::query("UPDATE billing_outbox SET attempt_count=attempt_count+1,next_attempt_at=$2 + make_interval(secs=>$3) WHERE event_id=$1").bind(event_id).bind(now).bind(delay_seconds).execute(&mut *transaction).await.map_err(|_| BillingError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| BillingError::Unavailable)?;
        Ok(Some(BillingDelivery {
            event_id,
            tenant_id: row
                .try_get("tenant_id")
                .map_err(|_| BillingError::Unavailable)?,
            url: row.try_get("url").map_err(|_| BillingError::Unavailable)?,
            secret_ref: SecretRef(
                row.try_get("secret_ref")
                    .map_err(|_| BillingError::Unavailable)?,
            ),
            payload: row
                .try_get("payload")
                .map_err(|_| BillingError::Unavailable)?,
            attempt_count: u32::try_from(attempt_count + 1).unwrap_or_default(),
            maximum_attempts: u32::try_from(
                row.try_get::<i32, _>("maximum_attempts")
                    .map_err(|_| BillingError::Unavailable)?,
            )
            .unwrap_or_default(),
        }))
    }

    async fn finish_delivery(
        &self,
        event_id: Uuid,
        status_code: Option<u16>,
        error: Option<&str>,
        delivered: bool,
    ) -> Result<(), BillingError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| BillingError::Unavailable)?;
        sqlx::query("INSERT INTO billing_delivery_attempts (attempt_id,event_id,status_code,error) VALUES ($1,$2,$3,$4)").bind(Uuid::now_v7()).bind(event_id).bind(status_code.map(i32::from)).bind(error.map(|value| value.chars().take(512).collect::<String>())).execute(&mut *transaction).await.map_err(|_| BillingError::Unavailable)?;
        sqlx::query("UPDATE billing_outbox SET delivered_at=CASE WHEN $2 THEN now() ELSE delivered_at END,last_error=$3 WHERE event_id=$1").bind(event_id).bind(delivered).bind(error.map(|value| value.chars().take(512).collect::<String>())).execute(&mut *transaction).await.map_err(|_| BillingError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| BillingError::Unavailable)
    }
}

#[async_trait]
impl PolicyResolver for PostgresStore {
    async fn resolve(&self, principal: &Principal) -> Result<Policy, PolicyError> {
        let rows = sqlx::query("SELECT allowed_models, denied_models, allowed_operations, max_output_tokens, concurrent_requests, daily_token_limit, monthly_token_limit FROM policies WHERE tenant_id=$1 AND ((scope_kind='global') OR (scope_kind='tenant' AND scope_id=$1) OR (scope_kind='project' AND scope_id=$2) OR (scope_kind='principal' AND scope_id=$3) OR (scope_kind='virtual_key' AND scope_id=$4)) ORDER BY CASE scope_kind WHEN 'global' THEN 1 WHEN 'tenant' THEN 2 WHEN 'project' THEN 3 WHEN 'principal' THEN 4 ELSE 5 END")
            .bind(&principal.tenant_id).bind(principal.project_id.as_deref().unwrap_or("")).bind(&principal.principal_id).bind(principal.virtual_key_id.map(|id| id.to_string()).unwrap_or_default()).fetch_all(&self.pool).await.map_err(|_| PolicyError::Unavailable)?;
        let mut policy = Policy::default();
        for row in rows {
            let next = Policy {
                allowed_models: row
                    .try_get("allowed_models")
                    .map_err(|_| PolicyError::Unavailable)?,
                denied_models: row
                    .try_get("denied_models")
                    .map_err(|_| PolicyError::Unavailable)?,
                allowed_operations: row
                    .try_get("allowed_operations")
                    .map_err(|_| PolicyError::Unavailable)?,
                max_output_tokens: row
                    .try_get::<Option<i64>, _>("max_output_tokens")
                    .map_err(|_| PolicyError::Unavailable)?
                    .and_then(|value| value.try_into().ok()),
                concurrent_requests: row
                    .try_get::<Option<i64>, _>("concurrent_requests")
                    .map_err(|_| PolicyError::Unavailable)?
                    .and_then(|value| value.try_into().ok()),
                daily_token_limit: row
                    .try_get::<Option<i64>, _>("daily_token_limit")
                    .map_err(|_| PolicyError::Unavailable)?
                    .and_then(|value| value.try_into().ok()),
                monthly_token_limit: row
                    .try_get::<Option<i64>, _>("monthly_token_limit")
                    .map_err(|_| PolicyError::Unavailable)?
                    .and_then(|value| value.try_into().ok()),
            };
            policy = policy.restrict_with(&next);
        }
        let rows = sqlx::query(
            "SELECT body FROM admin_resources WHERE kind='policies' AND enabled=true
             AND retired_at IS NULL AND (tenant_id IS NULL OR tenant_id=$1)
             AND ((body->>'scope_kind'='global')
               OR (body->>'scope_kind'='tenant' AND body->>'scope_id'=$1)
               OR (body->>'scope_kind'='project' AND body->>'scope_id'=$2)
               OR (body->>'scope_kind'='principal' AND body->>'scope_id'=$3)
               OR (body->>'scope_kind'='virtual_key' AND body->>'scope_id'=$4))
             ORDER BY CASE body->>'scope_kind' WHEN 'global' THEN 1 WHEN 'tenant' THEN 2
               WHEN 'project' THEN 3 WHEN 'principal' THEN 4 ELSE 5 END",
        )
        .bind(&principal.tenant_id)
        .bind(principal.project_id.as_deref().unwrap_or(""))
        .bind(&principal.principal_id)
        .bind(
            principal
                .virtual_key_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|_| PolicyError::Unavailable)?;
        for row in rows {
            let body: serde_json::Value =
                row.try_get("body").map_err(|_| PolicyError::Unavailable)?;
            let next = serde_json::from_value(body.get("policy").cloned().unwrap_or(body))
                .map_err(|_| PolicyError::Unavailable)?;
            policy = policy.restrict_with(&next);
        }
        Ok(policy)
    }
}

#[async_trait]
impl PricingCatalog for PostgresStore {
    async fn active_price(
        &self,
        provider_id: &str,
        upstream_model: &str,
        at: DateTime<Utc>,
    ) -> Result<Option<ModelPrice>, PricingError> {
        if let Some(row)=sqlx::query("SELECT * FROM model_prices WHERE provider_id=$1 AND upstream_model=$2 AND effective_from<=$3 AND (effective_until IS NULL OR effective_until>$3) AND enabled=true AND retired_at IS NULL ORDER BY effective_from DESC LIMIT 1").bind(provider_id).bind(upstream_model).bind(at).fetch_optional(&self.pool).await.map_err(|_|PricingError::Unavailable)? {
            return Ok(Some(ModelPrice{price_id:row.try_get("price_id").map_err(|_|PricingError::Unavailable)?,provider_id:row.try_get("provider_id").map_err(|_|PricingError::Unavailable)?,upstream_model:row.try_get("upstream_model").map_err(|_|PricingError::Unavailable)?,input_cost_per_million:row.try_get("input_cost_per_million").map_err(|_|PricingError::Unavailable)?,output_cost_per_million:row.try_get("output_cost_per_million").map_err(|_|PricingError::Unavailable)?,cached_input_cost_per_million:row.try_get("cached_input_cost_per_million").map_err(|_|PricingError::Unavailable)?,embedding_cost_per_million:row.try_get("embedding_cost_per_million").map_err(|_|PricingError::Unavailable)?,effective_from:row.try_get("effective_from").map_err(|_|PricingError::Unavailable)?,effective_until:row.try_get("effective_until").map_err(|_|PricingError::Unavailable)?}));
        }
        let row=sqlx::query("SELECT id,body FROM admin_resources WHERE kind='model_prices' AND enabled=true AND retired_at IS NULL AND body->>'provider_id'=$1 AND body->>'upstream_model'=$2 ORDER BY updated_at DESC LIMIT 1")
            .bind(provider_id).bind(upstream_model).fetch_optional(&self.pool).await.map_err(|_|PricingError::Unavailable)?;
        row.map(|row| {
            let id: String = row.try_get("id").map_err(|_| PricingError::Unavailable)?;
            let body: serde_json::Value =
                row.try_get("body").map_err(|_| PricingError::Unavailable)?;
            let value: AdminPrice =
                serde_json::from_value(body.get("price").cloned().unwrap_or(body))
                    .map_err(|_| PricingError::Unavailable)?;
            let price = ModelPrice {
                price_id: id.parse().map_err(|_| PricingError::Unavailable)?,
                provider_id: value.provider_id,
                upstream_model: value.upstream_model,
                input_cost_per_million: value.input_cost_per_million,
                output_cost_per_million: value.output_cost_per_million,
                cached_input_cost_per_million: value.cached_input_cost_per_million,
                embedding_cost_per_million: value.embedding_cost_per_million,
                effective_from: value.effective_from,
                effective_until: value.effective_until,
            };
            price
                .active_at(at)
                .then_some(price)
                .ok_or(PricingError::Unavailable)
        })
        .transpose()
    }
}

#[derive(serde::Deserialize)]
struct AdminPrice {
    provider_id: String,
    upstream_model: String,
    input_cost_per_million: rust_decimal::Decimal,
    output_cost_per_million: rust_decimal::Decimal,
    cached_input_cost_per_million: Option<rust_decimal::Decimal>,
    embedding_cost_per_million: Option<rust_decimal::Decimal>,
    effective_from: DateTime<Utc>,
    effective_until: Option<DateTime<Utc>>,
}

#[async_trait]
impl ProviderHealthRepository for PostgresStore {
    async fn record_provider_health(
        &self,
        provider_id: &str,
        health: ProviderHealth,
    ) -> Result<(), ProviderError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| ProviderError::Transport)?;
        sqlx::query(
            "INSERT INTO providers (id,tenant_id,provider_type,base_url,enabled)
             SELECT id,tenant_id,body->>'provider_type',body->>'base_url',enabled
             FROM admin_resources
             WHERE kind='providers' AND id=$1 AND retired_at IS NULL
             ON CONFLICT (id) DO UPDATE SET
               tenant_id=EXCLUDED.tenant_id,
               provider_type=EXCLUDED.provider_type,
               base_url=EXCLUDED.base_url,
               enabled=EXCLUDED.enabled,
               updated_at=now()",
        )
        .bind(provider_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ProviderError::Transport)?;
        sqlx::query(
            "INSERT INTO provider_health
               (provider_id,status,consecutive_failures,latest_success_at,latest_failure_at,updated_at)
             VALUES($1,$2,$3,$4,$5,now())
             ON CONFLICT (provider_id) DO UPDATE SET
               status=EXCLUDED.status,
               consecutive_failures=EXCLUDED.consecutive_failures,
               latest_success_at=EXCLUDED.latest_success_at,
               latest_failure_at=EXCLUDED.latest_failure_at,
               updated_at=now()",
        )
        .bind(provider_id)
        .bind(match health.status {
            ProviderHealthStatus::Healthy => "healthy",
            ProviderHealthStatus::Degraded => "degraded",
            ProviderHealthStatus::Unhealthy => "unhealthy",
            ProviderHealthStatus::Unknown => "unknown",
        })
        .bind(i32::try_from(health.consecutive_failures).unwrap_or(i32::MAX))
        .bind(health.latest_success_at)
        .bind(health.latest_failure_at)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ProviderError::Transport)?;
        transaction
            .commit()
            .await
            .map_err(|_| ProviderError::Transport)
    }
}

#[async_trait]
impl McpHealthRepository for PostgresStore {
    async fn record_health(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
        health: McpHealth,
    ) -> Result<(), McpError> {
        sqlx::query("INSERT INTO mcp_health_checks (health_id,tenant_id,server_id,status,latency_ms,checked_at) VALUES ($1,$2,$3,$4,$5,$6)").bind(Uuid::now_v7()).bind(tenant_id).bind(server_id.0).bind(health_name(health.status)).bind(health.latency_ms.and_then(|value| i64::try_from(value).ok())).bind(health.checked_at).execute(&self.pool).await.map_err(|_| McpError::Unavailable).map(|_| ())
    }
    async fn latest_health(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
    ) -> Result<Option<McpHealth>, McpError> {
        sqlx::query("SELECT status,latency_ms,checked_at FROM mcp_health_checks WHERE tenant_id=$1 AND server_id=$2 ORDER BY checked_at DESC LIMIT 1").bind(tenant_id).bind(server_id.0).fetch_optional(&self.pool).await.map_err(|_| McpError::Unavailable)?.map(health_from_row).transpose()
    }
}

#[async_trait]
impl McpRepository for PostgresStore {
    async fn insert_server(&self, server: McpServerRecord) -> Result<(), McpError> {
        write_server(self, server, false).await
    }
    async fn update_server(&self, server: McpServerRecord) -> Result<(), McpError> {
        write_server(self, server, true).await
    }
    async fn delete_server(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
    ) -> Result<bool, McpError> {
        sqlx::query("DELETE FROM mcp_servers WHERE tenant_id=$1 AND server_id=$2")
            .bind(tenant_id)
            .bind(server_id.0)
            .execute(&self.pool)
            .await
            .map_err(|_| McpError::Unavailable)
            .map(|result| result.rows_affected() > 0)
    }
    async fn server(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
    ) -> Result<Option<McpServerRecord>, McpError> {
        sqlx::query("SELECT * FROM mcp_servers WHERE tenant_id=$1 AND server_id=$2")
            .bind(tenant_id)
            .bind(server_id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| McpError::Unavailable)?
            .map(server_from_row)
            .transpose()
    }
    async fn servers(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
    ) -> Result<Vec<McpServerRecord>, McpError> {
        sqlx::query("SELECT * FROM mcp_servers WHERE tenant_id=$1 AND ($2::text IS NULL OR project_id IS NULL OR project_id=$2) ORDER BY name").bind(tenant_id).bind(project_id).fetch_all(&self.pool).await.map_err(|_| McpError::Unavailable)?.into_iter().map(server_from_row).collect()
    }
    async fn replace_tools(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
        tools: Vec<(GatewayMcpTool, String)>,
    ) -> Result<(), McpError> {
        let mut transaction = self.pool.begin().await.map_err(|_| McpError::Unavailable)?;
        sqlx::query("UPDATE mcp_tools SET active=false WHERE tenant_id=$1 AND server_id=$2")
            .bind(tenant_id)
            .bind(server_id.0)
            .execute(&mut *transaction)
            .await
            .map_err(|_| McpError::Unavailable)?;
        for (tool, hash) in tools {
            sqlx::query("INSERT INTO mcp_tools (server_id,tenant_id,tool_name,description,input_schema,annotations,discovered_at,schema_hash,active) VALUES ($1,$2,$3,$4,$5,$6,now(),$7,true) ON CONFLICT (server_id,tool_name) DO UPDATE SET description=EXCLUDED.description,input_schema=EXCLUDED.input_schema,annotations=EXCLUDED.annotations,discovered_at=now(),schema_hash=EXCLUDED.schema_hash,active=true").bind(server_id.0).bind(tenant_id).bind(tool.tool_name).bind(tool.description).bind(tool.input_schema).bind(serde_json::to_value(tool.annotations).map_err(|_| McpError::Invalid)?).bind(hash).execute(&mut *transaction).await.map_err(|_| McpError::Unavailable)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| McpError::Unavailable)
    }
    async fn tools(
        &self,
        tenant_id: &str,
        server_id: Option<McpServerId>,
    ) -> Result<Vec<GatewayMcpTool>, McpError> {
        sqlx::query("SELECT server_id,tool_name,description,input_schema,annotations FROM mcp_tools WHERE tenant_id=$1 AND active=true AND ($2::uuid IS NULL OR server_id=$2) ORDER BY server_id,tool_name").bind(tenant_id).bind(server_id.map(|id| id.0)).fetch_all(&self.pool).await.map_err(|_| McpError::Unavailable)?.into_iter().map(|row| Ok(GatewayMcpTool { server_id: McpServerId(row.try_get("server_id").map_err(|_| McpError::Unavailable)?), tool_name: row.try_get("tool_name").map_err(|_| McpError::Unavailable)?, description: row.try_get("description").map_err(|_| McpError::Unavailable)?, input_schema: row.try_get("input_schema").map_err(|_| McpError::Unavailable)?, annotations: serde_json::from_value(row.try_get("annotations").map_err(|_| McpError::Unavailable)?).map_err(|_| McpError::Invalid)? })).collect()
    }
    async fn update_tool_annotations(
        &self,
        tenant_id: &str,
        server_id: McpServerId,
        tool_name: &str,
        annotations: gateway_types::ToolAnnotations,
    ) -> Result<bool, McpError> {
        let value = serde_json::to_value(&annotations).map_err(|_| McpError::Invalid)?;
        let mut transaction = self.pool.begin().await.map_err(|_| McpError::Unavailable)?;
        let updated=sqlx::query("UPDATE mcp_tools SET annotations=$4 WHERE tenant_id=$1 AND server_id=$2 AND tool_name=$3 AND active=true").bind(tenant_id).bind(server_id.0).bind(tool_name).bind(&value).execute(&mut *transaction).await.map_err(|_|McpError::Unavailable)?.rows_affected()==1;
        if updated {
            sqlx::query("INSERT INTO mcp_tool_annotations(annotation_id,tenant_id,server_id,tool_name,risk_level,metadata) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(server_id,tool_name) DO UPDATE SET risk_level=EXCLUDED.risk_level,metadata=EXCLUDED.metadata")
                .bind(Uuid::now_v7()).bind(tenant_id).bind(server_id.0).bind(tool_name)
                .bind(annotations.administrator_risk.map(risk_name).unwrap_or("unknown"))
                .bind(value).execute(&mut *transaction).await.map_err(|_|McpError::Unavailable)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| McpError::Unavailable)?;
        Ok(updated)
    }
}

#[async_trait]
impl McpPolicyRepository for PostgresStore {
    async fn resolved_policy(&self, principal: &Principal) -> Result<McpPolicy, McpError> {
        let rows = sqlx::query("SELECT p.rules FROM mcp_policies p JOIN mcp_policy_bindings b ON b.policy_id=p.policy_id WHERE p.tenant_id=$1 AND ((b.scope_kind='global') OR (b.scope_kind='tenant' AND b.scope_id=$1) OR (b.scope_kind='project' AND b.scope_id=$2) OR (b.scope_kind='principal' AND b.scope_id=$3) OR (b.scope_kind='virtual_key' AND b.scope_id=$4)) ORDER BY CASE b.scope_kind WHEN 'global' THEN 1 WHEN 'tenant' THEN 2 WHEN 'project' THEN 3 WHEN 'principal' THEN 4 ELSE 5 END").bind(&principal.tenant_id).bind(principal.project_id.as_deref().unwrap_or("")).bind(&principal.principal_id).bind(principal.virtual_key_id.map(|id| id.to_string()).unwrap_or_default()).fetch_all(&self.pool).await.map_err(|_| McpError::Unavailable)?;
        let mut policy = McpPolicy::default();
        for row in rows {
            let next =
                serde_json::from_value(row.try_get("rules").map_err(|_| McpError::Unavailable)?)
                    .map_err(|_| McpError::Invalid)?;
            policy = policy.restrict_with(&next);
        }
        Ok(policy)
    }
}

#[async_trait]
impl McpPolicyAdministration for PostgresStore {
    async fn insert_mcp_policy(&self, record: McpPolicyRecord) -> Result<(), McpError> {
        let mut transaction = self.pool.begin().await.map_err(|_| McpError::Unavailable)?;
        sqlx::query("INSERT INTO mcp_policies (policy_id,tenant_id,name,rules,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6)").bind(record.policy_id.0).bind(&record.tenant_id).bind(record.name).bind(serde_json::to_value(record.policy).map_err(|_|McpError::Invalid)?).bind(record.created_at).bind(record.updated_at).execute(&mut *transaction).await.map_err(|_|McpError::Unavailable)?;
        sqlx::query("INSERT INTO mcp_policy_bindings (binding_id,tenant_id,policy_id,scope_kind,scope_id) VALUES ($1,$2,$3,$4,$5)").bind(Uuid::now_v7()).bind(record.tenant_id).bind(record.policy_id.0).bind(record.scope_kind).bind(record.scope_id).execute(&mut *transaction).await.map_err(|_|McpError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| McpError::Unavailable)
    }
    async fn mcp_policies(&self, tenant_id: &str) -> Result<Vec<McpPolicyRecord>, McpError> {
        sqlx::query("SELECT p.policy_id,p.tenant_id,p.name,p.rules,p.created_at,p.updated_at,b.scope_kind,b.scope_id FROM mcp_policies p JOIN mcp_policy_bindings b ON b.policy_id=p.policy_id WHERE p.tenant_id=$1 ORDER BY p.name").bind(tenant_id).fetch_all(&self.pool).await.map_err(|_|McpError::Unavailable)?.into_iter().map(|row|Ok(McpPolicyRecord{policy_id:gateway_types::McpPolicyId(row.try_get("policy_id").map_err(|_|McpError::Unavailable)?),tenant_id:row.try_get("tenant_id").map_err(|_|McpError::Unavailable)?,name:row.try_get("name").map_err(|_|McpError::Unavailable)?,policy:serde_json::from_value(row.try_get("rules").map_err(|_|McpError::Unavailable)?).map_err(|_|McpError::Invalid)?,scope_kind:row.try_get("scope_kind").map_err(|_|McpError::Unavailable)?,scope_id:row.try_get("scope_id").map_err(|_|McpError::Unavailable)?,created_at:row.try_get("created_at").map_err(|_|McpError::Unavailable)?,updated_at:row.try_get("updated_at").map_err(|_|McpError::Unavailable)?})).collect()
    }
    async fn update_mcp_policy(&self, record: McpPolicyRecord) -> Result<(), McpError> {
        let mut transaction = self.pool.begin().await.map_err(|_| McpError::Unavailable)?;
        sqlx::query("UPDATE mcp_policies SET name=$3,rules=$4,updated_at=now() WHERE tenant_id=$1 AND policy_id=$2").bind(&record.tenant_id).bind(record.policy_id.0).bind(record.name).bind(serde_json::to_value(record.policy).map_err(|_|McpError::Invalid)?).execute(&mut *transaction).await.map_err(|_|McpError::Unavailable)?;
        sqlx::query("UPDATE mcp_policy_bindings SET scope_kind=$3,scope_id=$4 WHERE tenant_id=$1 AND policy_id=$2").bind(record.tenant_id).bind(record.policy_id.0).bind(record.scope_kind).bind(record.scope_id).execute(&mut *transaction).await.map_err(|_|McpError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| McpError::Unavailable)
    }
    async fn delete_mcp_policy(
        &self,
        tenant_id: &str,
        policy_id: gateway_types::McpPolicyId,
    ) -> Result<bool, McpError> {
        sqlx::query("DELETE FROM mcp_policies WHERE tenant_id=$1 AND policy_id=$2")
            .bind(tenant_id)
            .bind(policy_id.0)
            .execute(&self.pool)
            .await
            .map_err(|_| McpError::Unavailable)
            .map(|result| result.rows_affected() > 0)
    }
}

#[async_trait]
impl McpUsageRepository for PostgresStore {
    async fn record_mcp_usage(&self, event: McpUsageEvent) -> Result<(), McpError> {
        sqlx::query("INSERT INTO mcp_invocations (invocation_id,request_id,tenant_id,project_id,principal_id,server_id,tool_name,request_hash,request_bytes,response_bytes,duration_ms,risk_level,approval_required,status,created_at,completed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,'',$8,$9,$10,$11,$12,$13,$14,$14) ON CONFLICT (request_id) DO NOTHING").bind(event.event_id).bind(event.request_id).bind(event.tenant_id).bind(event.project_id).bind(event.principal_id).bind(event.details.server_id.0).bind(event.details.tool_name).bind(i64::try_from(event.details.request_bytes).unwrap_or(i64::MAX)).bind(i64::try_from(event.details.response_bytes).unwrap_or(i64::MAX)).bind(i64::try_from(event.details.duration_ms).unwrap_or(i64::MAX)).bind(risk_name(event.details.risk_level)).bind(event.details.approval_required).bind(event.status).bind(event.occurred_at).execute(&self.pool).await.map_err(|_| McpError::Unavailable).map(|_| ())
    }
}

#[async_trait]
impl ApprovalRepository for PostgresStore {
    async fn insert_approval(&self, request: ApprovalRequest) -> Result<(), ApprovalError> {
        sqlx::query("INSERT INTO approval_requests (approval_id,tenant_id,project_id,principal_id,request_id,resource_type,resource_id,action,sanitized_arguments,risk_level,status,request_hash,expires_at,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)").bind(request.approval_id.0).bind(request.tenant_id).bind(request.project_id).bind(request.principal_id).bind(request.request_id).bind(resource_name(request.resource_type)).bind(request.resource_id).bind(request.action).bind(request.sanitized_arguments).bind(risk_name(request.risk_level)).bind(status_name(request.status)).bind(request.request_hash).bind(request.expires_at).bind(request.created_at).execute(&self.pool).await.map_err(|_| ApprovalError::Unavailable).map(|_| ())
    }
    async fn approval(
        &self,
        tenant_id: &str,
        approval_id: ApprovalId,
    ) -> Result<Option<ApprovalRequest>, ApprovalError> {
        sqlx::query("SELECT * FROM approval_requests WHERE tenant_id=$1 AND approval_id=$2")
            .bind(tenant_id)
            .bind(approval_id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| ApprovalError::Unavailable)?
            .map(approval_from_row)
            .transpose()
    }
    async fn list_approvals(
        &self,
        tenant_id: &str,
        status: Option<ApprovalStatus>,
        limit: u32,
    ) -> Result<Vec<ApprovalRequest>, ApprovalError> {
        sqlx::query("SELECT * FROM approval_requests WHERE tenant_id=$1 AND ($2::text IS NULL OR status=$2) ORDER BY created_at DESC LIMIT $3").bind(tenant_id).bind(status.map(status_name)).bind(i64::from(limit)).fetch_all(&self.pool).await.map_err(|_| ApprovalError::Unavailable)?.into_iter().map(approval_from_row).collect()
    }
    async fn decide_approval(
        &self,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRequest, ApprovalError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| ApprovalError::Unavailable)?;
        let row = sqlx::query("UPDATE approval_requests SET status=$3,updated_at=$4 WHERE tenant_id=$1 AND approval_id=$2 AND status='pending' AND expires_at>$4 RETURNING *").bind(&decision.tenant_id).bind(decision.approval_id.0).bind(status_name(decision.status)).bind(decision.decided_at).fetch_optional(&mut *transaction).await.map_err(|_| ApprovalError::Unavailable)?.ok_or(ApprovalError::Replay)?;
        sqlx::query("INSERT INTO approval_decisions (decision_id,approval_id,tenant_id,decided_by,decision,sanitized_reason,decided_at) VALUES ($1,$2,$3,$4,$5,$6,$7)").bind(Uuid::now_v7()).bind(decision.approval_id.0).bind(decision.tenant_id).bind(decision.decided_by).bind(status_name(decision.status)).bind(decision.sanitized_reason).bind(decision.decided_at).execute(&mut *transaction).await.map_err(|_| ApprovalError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| ApprovalError::Unavailable)?;
        approval_from_row(row)
    }
    async fn expire_approvals(&self, now: DateTime<Utc>) -> Result<u64, ApprovalError> {
        sqlx::query("UPDATE approval_requests SET status='expired',updated_at=$1 WHERE status='pending' AND expires_at<=$1").bind(now).execute(&self.pool).await.map_err(|_| ApprovalError::Unavailable).map(|result| result.rows_affected())
    }
    async fn claim_execution(
        &self,
        request: &ApprovalRequest,
        principal_id: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<ExecutionClaim, ApprovalError> {
        let inserted = sqlx::query("INSERT INTO approval_executions (execution_id,approval_id,tenant_id,principal_id,idempotency_key,request_hash,status) VALUES ($1,$2,$3,$4,$5,$6,'claimed') ON CONFLICT DO NOTHING").bind(Uuid::now_v7()).bind(request.approval_id.0).bind(&request.tenant_id).bind(principal_id).bind(idempotency_key).bind(request_hash).execute(&self.pool).await.map_err(|_| ApprovalError::Unavailable)?;
        if inserted.rows_affected() == 1 {
            return Ok(ExecutionClaim::Claimed);
        }
        let row=sqlx::query("SELECT principal_id,idempotency_key,request_hash,status,result FROM approval_executions WHERE tenant_id=$1 AND approval_id=$2").bind(&request.tenant_id).bind(request.approval_id.0).fetch_one(&self.pool).await.map_err(|_| ApprovalError::Unavailable)?;
        if row
            .try_get::<String, _>("principal_id")
            .map_err(|_| ApprovalError::Unavailable)?
            != principal_id
            || row
                .try_get::<String, _>("idempotency_key")
                .map_err(|_| ApprovalError::Unavailable)?
                != idempotency_key
            || row
                .try_get::<String, _>("request_hash")
                .map_err(|_| ApprovalError::Unavailable)?
                != request_hash
        {
            return Err(ApprovalError::Replay);
        }
        match row
            .try_get::<String, _>("status")
            .map_err(|_| ApprovalError::Unavailable)?
            .as_str()
        {
            "completed" => Ok(ExecutionClaim::Completed(
                row.try_get("result")
                    .map_err(|_| ApprovalError::Unavailable)?,
            )),
            "indeterminate" | "claimed" => Ok(ExecutionClaim::Indeterminate),
            _ => Err(ApprovalError::Replay),
        }
    }
    async fn complete_execution(
        &self,
        approval_id: ApprovalId,
        idempotency_key: &str,
        result: serde_json::Value,
    ) -> Result<(), ApprovalError> {
        sqlx::query("UPDATE approval_executions SET status='completed',result=$3,completed_at=now() WHERE approval_id=$1 AND idempotency_key=$2 AND status='claimed'").bind(approval_id.0).bind(idempotency_key).bind(result).execute(&self.pool).await.map_err(|_| ApprovalError::Unavailable).map(|_| ())
    }
    async fn fail_execution(
        &self,
        approval_id: ApprovalId,
        idempotency_key: &str,
        indeterminate: bool,
    ) -> Result<(), ApprovalError> {
        sqlx::query("UPDATE approval_executions SET status=$3,completed_at=now() WHERE approval_id=$1 AND idempotency_key=$2 AND status='claimed'").bind(approval_id.0).bind(idempotency_key).bind(if indeterminate { "indeterminate" } else { "failed" }).execute(&self.pool).await.map_err(|_| ApprovalError::Unavailable).map(|_| ())
    }
}

#[async_trait]
impl IncidentRepository for PostgresStore {
    async fn insert_incident(&self, incident: SecurityIncident) -> Result<(), IncidentError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| IncidentError::Unavailable)?;
        sqlx::query("INSERT INTO security_incidents (incident_id,tenant_id,project_id,principal_id,request_id,category,severity,status,risk_score,sanitized_summary,created_at,resolved_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)").bind(incident.incident_id.0).bind(&incident.tenant_id).bind(incident.project_id).bind(incident.principal_id).bind(incident.request_id).bind(category_name(incident.category)).bind(severity_name(incident.severity)).bind(incident_name(incident.status)).bind(i16::from(incident.risk_score)).bind(incident.sanitized_summary).bind(incident.created_at).bind(incident.resolved_at).execute(&mut *transaction).await.map_err(|_| IncidentError::Unavailable)?;
        sqlx::query("INSERT INTO security_incident_timeline(entry_id,incident_id,tenant_id,status,actor,sanitized_note,occurred_at) VALUES($1,$2,$3,$4,'gateway',NULL,$5)").bind(Uuid::now_v7()).bind(incident.incident_id.0).bind(&incident.tenant_id).bind(incident_name(incident.status)).bind(incident.created_at).execute(&mut *transaction).await.map_err(|_|IncidentError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| IncidentError::Unavailable)
    }
    async fn incident(
        &self,
        tenant_id: &str,
        incident_id: IncidentId,
    ) -> Result<Option<SecurityIncident>, IncidentError> {
        sqlx::query("SELECT * FROM security_incidents WHERE tenant_id=$1 AND incident_id=$2")
            .bind(tenant_id)
            .bind(incident_id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| IncidentError::Unavailable)?
            .map(incident_from_row)
            .transpose()
    }
    async fn list_incidents(
        &self,
        tenant_id: &str,
        status: Option<IncidentStatus>,
        limit: u32,
    ) -> Result<Vec<SecurityIncident>, IncidentError> {
        sqlx::query("SELECT * FROM security_incidents WHERE tenant_id=$1 AND ($2::text IS NULL OR status=$2) ORDER BY created_at DESC LIMIT $3").bind(tenant_id).bind(status.map(incident_name)).bind(i64::from(limit)).fetch_all(&self.pool).await.map_err(|_| IncidentError::Unavailable)?.into_iter().map(incident_from_row).collect()
    }
    async fn update_incident(
        &self,
        tenant_id: &str,
        entry: IncidentTimelineEntry,
    ) -> Result<SecurityIncident, IncidentError> {
        let resolved = matches!(
            entry.status,
            IncidentStatus::Resolved | IncidentStatus::Ignored
        )
        .then_some(entry.occurred_at);
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| IncidentError::Unavailable)?;
        let row=sqlx::query("UPDATE security_incidents SET status=$3,resolved_at=$4 WHERE tenant_id=$1 AND incident_id=$2 RETURNING *").bind(tenant_id).bind(entry.incident_id.0).bind(incident_name(entry.status)).bind(resolved).fetch_optional(&mut *transaction).await.map_err(|_| IncidentError::Unavailable)?.ok_or(IncidentError::NotFound)?;
        sqlx::query("INSERT INTO security_incident_timeline(entry_id,incident_id,tenant_id,status,actor,sanitized_note,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(entry.entry_id).bind(entry.incident_id.0).bind(tenant_id).bind(incident_name(entry.status)).bind(entry.actor).bind(entry.sanitized_note).bind(entry.occurred_at).execute(&mut *transaction).await.map_err(|_|IncidentError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| IncidentError::Unavailable)?;
        incident_from_row(row)
    }
    async fn incident_timeline(
        &self,
        tenant_id: &str,
        incident_id: IncidentId,
    ) -> Result<Vec<IncidentTimelineEntry>, IncidentError> {
        sqlx::query("SELECT entry_id,incident_id,status,actor,sanitized_note,occurred_at FROM security_incident_timeline WHERE tenant_id=$1 AND incident_id=$2 ORDER BY occurred_at,entry_id")
            .bind(tenant_id).bind(incident_id.0).fetch_all(&self.pool).await.map_err(|_|IncidentError::Unavailable)?
            .into_iter().map(|row|Ok(IncidentTimelineEntry{
                entry_id:row.try_get("entry_id").map_err(|_|IncidentError::Unavailable)?,
                incident_id:IncidentId(row.try_get("incident_id").map_err(|_|IncidentError::Unavailable)?),
                status:parse_incident(&row.try_get::<String,_>("status").map_err(|_|IncidentError::Unavailable)?),
                actor:row.try_get("actor").map_err(|_|IncidentError::Unavailable)?,
                sanitized_note:row.try_get("sanitized_note").map_err(|_|IncidentError::Unavailable)?,
                occurred_at:row.try_get("occurred_at").map_err(|_|IncidentError::Unavailable)?,
            })).collect()
    }
}

async fn write_server(
    store: &PostgresStore,
    server: McpServerRecord,
    update: bool,
) -> Result<(), McpError> {
    let arguments = serde_json::to_value(server.arguments).map_err(|_| McpError::Invalid)?;
    let environment =
        serde_json::to_value(server.environment_secret_refs).map_err(|_| McpError::Invalid)?;
    let credential = server.credential_ref.map(|value| value.0);
    let metadata = serde_json::to_value(server.metadata).map_err(|_| McpError::Invalid)?;
    let mut query = sqlx::query(if update { "UPDATE mcp_servers SET project_id=$3,name=$4,description=$5,transport_type=$6,endpoint=$7,command=$8,arguments=$9,environment_secret_refs=$10,credential_ref=$11,enabled=$12,metadata=$13,updated_at=now() WHERE tenant_id=$1 AND server_id=$2" } else { "INSERT INTO mcp_servers (tenant_id,server_id,project_id,name,description,transport_type,endpoint,command,arguments,environment_secret_refs,credential_ref,enabled,metadata,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)" })
        .bind(server.tenant_id).bind(server.server_id.0).bind(server.project_id).bind(server.name).bind(server.description).bind(transport_name(server.transport_type)).bind(server.endpoint).bind(server.command).bind(arguments).bind(environment).bind(credential).bind(server.enabled).bind(metadata);
    if !update {
        query = query.bind(server.created_at).bind(server.updated_at);
    }
    query
        .execute(&store.pool)
        .await
        .map_err(|_| McpError::Unavailable)
        .map(|_| ())
}
fn server_from_row(row: sqlx::postgres::PgRow) -> Result<McpServerRecord, McpError> {
    Ok(McpServerRecord {
        server_id: McpServerId(
            row.try_get("server_id")
                .map_err(|_| McpError::Unavailable)?,
        ),
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| McpError::Unavailable)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| McpError::Unavailable)?,
        name: row.try_get("name").map_err(|_| McpError::Unavailable)?,
        description: row
            .try_get("description")
            .map_err(|_| McpError::Unavailable)?,
        transport_type: parse_transport(
            &row.try_get::<String, _>("transport_type")
                .map_err(|_| McpError::Unavailable)?,
        )?,
        endpoint: row.try_get("endpoint").map_err(|_| McpError::Unavailable)?,
        command: row.try_get("command").map_err(|_| McpError::Unavailable)?,
        arguments: serde_json::from_value(
            row.try_get("arguments")
                .map_err(|_| McpError::Unavailable)?,
        )
        .map_err(|_| McpError::Invalid)?,
        environment_secret_refs: serde_json::from_value(
            row.try_get("environment_secret_refs")
                .map_err(|_| McpError::Unavailable)?,
        )
        .map_err(|_| McpError::Invalid)?,
        credential_ref: row
            .try_get::<Option<String>, _>("credential_ref")
            .map_err(|_| McpError::Unavailable)?
            .map(SecretRef),
        enabled: row.try_get("enabled").map_err(|_| McpError::Unavailable)?,
        metadata: serde_json::from_value(
            row.try_get("metadata").map_err(|_| McpError::Unavailable)?,
        )
        .map_err(|_| McpError::Invalid)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| McpError::Unavailable)?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|_| McpError::Unavailable)?,
    })
}
fn health_from_row(row: sqlx::postgres::PgRow) -> Result<McpHealth, McpError> {
    Ok(McpHealth {
        status: match row
            .try_get::<String, _>("status")
            .map_err(|_| McpError::Unavailable)?
            .as_str()
        {
            "healthy" => McpHealthStatus::Healthy,
            "degraded" => McpHealthStatus::Degraded,
            "unhealthy" => McpHealthStatus::Unhealthy,
            _ => McpHealthStatus::Unknown,
        },
        latency_ms: row
            .try_get::<Option<i64>, _>("latency_ms")
            .map_err(|_| McpError::Unavailable)?
            .and_then(|v| v.try_into().ok()),
        checked_at: row
            .try_get("checked_at")
            .map_err(|_| McpError::Unavailable)?,
    })
}
fn approval_from_row(row: sqlx::postgres::PgRow) -> Result<ApprovalRequest, ApprovalError> {
    Ok(ApprovalRequest {
        approval_id: ApprovalId(
            row.try_get("approval_id")
                .map_err(|_| ApprovalError::Unavailable)?,
        ),
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| ApprovalError::Unavailable)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| ApprovalError::Unavailable)?,
        principal_id: row
            .try_get("principal_id")
            .map_err(|_| ApprovalError::Unavailable)?,
        request_id: row
            .try_get("request_id")
            .map_err(|_| ApprovalError::Unavailable)?,
        resource_type: match row
            .try_get::<String, _>("resource_type")
            .map_err(|_| ApprovalError::Unavailable)?
            .as_str()
        {
            "mcp_tool" => ApprovalResourceType::McpTool,
            _ => ApprovalResourceType::InferenceRequest,
        },
        resource_id: row
            .try_get("resource_id")
            .map_err(|_| ApprovalError::Unavailable)?,
        action: row
            .try_get("action")
            .map_err(|_| ApprovalError::Unavailable)?,
        sanitized_arguments: row
            .try_get("sanitized_arguments")
            .map_err(|_| ApprovalError::Unavailable)?,
        risk_level: parse_risk(
            &row.try_get::<String, _>("risk_level")
                .map_err(|_| ApprovalError::Unavailable)?,
        ),
        status: parse_status(
            &row.try_get::<String, _>("status")
                .map_err(|_| ApprovalError::Unavailable)?,
        ),
        request_hash: row
            .try_get("request_hash")
            .map_err(|_| ApprovalError::Unavailable)?,
        expires_at: row
            .try_get("expires_at")
            .map_err(|_| ApprovalError::Unavailable)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| ApprovalError::Unavailable)?,
    })
}
fn incident_from_row(row: sqlx::postgres::PgRow) -> Result<SecurityIncident, IncidentError> {
    Ok(SecurityIncident {
        incident_id: IncidentId(
            row.try_get("incident_id")
                .map_err(|_| IncidentError::Unavailable)?,
        ),
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| IncidentError::Unavailable)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| IncidentError::Unavailable)?,
        principal_id: row
            .try_get("principal_id")
            .map_err(|_| IncidentError::Unavailable)?,
        request_id: row
            .try_get("request_id")
            .map_err(|_| IncidentError::Unavailable)?,
        category: parse_category(
            &row.try_get::<String, _>("category")
                .map_err(|_| IncidentError::Unavailable)?,
        ),
        severity: parse_severity(
            &row.try_get::<String, _>("severity")
                .map_err(|_| IncidentError::Unavailable)?,
        ),
        status: parse_incident(
            &row.try_get::<String, _>("status")
                .map_err(|_| IncidentError::Unavailable)?,
        ),
        risk_score: row
            .try_get::<i16, _>("risk_score")
            .map_err(|_| IncidentError::Unavailable)?
            .try_into()
            .map_err(|_| IncidentError::Unavailable)?,
        sanitized_summary: row
            .try_get("sanitized_summary")
            .map_err(|_| IncidentError::Unavailable)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| IncidentError::Unavailable)?,
        resolved_at: row
            .try_get("resolved_at")
            .map_err(|_| IncidentError::Unavailable)?,
    })
}
fn transport_name(value: McpTransportType) -> &'static str {
    match value {
        McpTransportType::Stdio => "stdio",
        McpTransportType::StreamableHttp => "streamable_http",
    }
}
fn parse_transport(value: &str) -> Result<McpTransportType, McpError> {
    match value {
        "stdio" => Ok(McpTransportType::Stdio),
        "streamable_http" => Ok(McpTransportType::StreamableHttp),
        _ => Err(McpError::Invalid),
    }
}
fn health_name(value: McpHealthStatus) -> &'static str {
    match value {
        McpHealthStatus::Healthy => "healthy",
        McpHealthStatus::Degraded => "degraded",
        McpHealthStatus::Unhealthy => "unhealthy",
        McpHealthStatus::Unknown => "unknown",
    }
}
fn risk_name(value: ToolRiskLevel) -> &'static str {
    match value {
        ToolRiskLevel::ReadOnly => "read_only",
        ToolRiskLevel::Mutating => "mutating",
        ToolRiskLevel::Destructive => "destructive",
        ToolRiskLevel::Privileged => "privileged",
        ToolRiskLevel::Unknown => "unknown",
    }
}
fn parse_risk(value: &str) -> ToolRiskLevel {
    match value {
        "read_only" => ToolRiskLevel::ReadOnly,
        "mutating" => ToolRiskLevel::Mutating,
        "destructive" => ToolRiskLevel::Destructive,
        "privileged" => ToolRiskLevel::Privileged,
        _ => ToolRiskLevel::Unknown,
    }
}
fn status_name(value: ApprovalStatus) -> &'static str {
    match value {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Rejected => "rejected",
        ApprovalStatus::Expired => "expired",
        ApprovalStatus::Cancelled => "cancelled",
    }
}
fn parse_status(value: &str) -> ApprovalStatus {
    match value {
        "approved" => ApprovalStatus::Approved,
        "rejected" => ApprovalStatus::Rejected,
        "expired" => ApprovalStatus::Expired,
        "cancelled" => ApprovalStatus::Cancelled,
        _ => ApprovalStatus::Pending,
    }
}
fn resource_name(value: ApprovalResourceType) -> &'static str {
    match value {
        ApprovalResourceType::McpTool => "mcp_tool",
        ApprovalResourceType::InferenceRequest => "inference_request",
    }
}
fn incident_name(value: IncidentStatus) -> &'static str {
    match value {
        IncidentStatus::Open => "open",
        IncidentStatus::Acknowledged => "acknowledged",
        IncidentStatus::Resolved => "resolved",
        IncidentStatus::Ignored => "ignored",
    }
}
fn parse_incident(value: &str) -> IncidentStatus {
    match value {
        "acknowledged" => IncidentStatus::Acknowledged,
        "resolved" => IncidentStatus::Resolved,
        "ignored" => IncidentStatus::Ignored,
        _ => IncidentStatus::Open,
    }
}
fn severity_name(value: SecuritySeverity) -> &'static str {
    match value {
        SecuritySeverity::Low => "low",
        SecuritySeverity::Medium => "medium",
        SecuritySeverity::High => "high",
        SecuritySeverity::Critical => "critical",
    }
}
fn parse_severity(value: &str) -> SecuritySeverity {
    match value {
        "medium" => SecuritySeverity::Medium,
        "high" => SecuritySeverity::High,
        "critical" => SecuritySeverity::Critical,
        _ => SecuritySeverity::Low,
    }
}
fn category_name(value: SecurityCategory) -> &'static str {
    match value {
        SecurityCategory::PromptInjection => "prompt_injection",
        SecurityCategory::JailbreakAttempt => "jailbreak_attempt",
        SecurityCategory::SecretExposure => "secret_exposure",
        SecurityCategory::CredentialExposure => "credential_exposure",
        SecurityCategory::SensitivePersonalData => "sensitive_personal_data",
        SecurityCategory::FinancialData => "financial_data",
        SecurityCategory::SourceCodeSecret => "source_code_secret",
        SecurityCategory::DataExfiltrationAttempt => "data_exfiltration_attempt",
        SecurityCategory::PolicyViolation => "policy_violation",
        SecurityCategory::SuspiciousToolArgument => "suspicious_tool_argument",
        SecurityCategory::SuspiciousToolResult => "suspicious_tool_result",
    }
}
fn parse_category(value: &str) -> SecurityCategory {
    match value {
        "prompt_injection" => SecurityCategory::PromptInjection,
        "jailbreak_attempt" => SecurityCategory::JailbreakAttempt,
        "credential_exposure" => SecurityCategory::CredentialExposure,
        "sensitive_personal_data" => SecurityCategory::SensitivePersonalData,
        "financial_data" => SecurityCategory::FinancialData,
        "source_code_secret" => SecurityCategory::SourceCodeSecret,
        "data_exfiltration_attempt" => SecurityCategory::DataExfiltrationAttempt,
        "policy_violation" => SecurityCategory::PolicyViolation,
        "suspicious_tool_argument" => SecurityCategory::SuspiciousToolArgument,
        "suspicious_tool_result" => SecurityCategory::SuspiciousToolResult,
        _ => SecurityCategory::SecretExposure,
    }
}
