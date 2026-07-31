//! PostgreSQL implementation of the gateway persistence boundary.

pub mod admin;
mod entitlements;
mod identity;
mod security_store;
mod v03;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_store::{GatewayStore, StoreError, TenantRecord};
use gateway_types::{
    QuotaOwner, QuotaReservation, TokenUsage, UsageEvent, UsageStatus, VirtualKeyRecord,
};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use uuid::Uuid;

/// Durable PostgreSQL gateway store.
#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    /// Connect and run embedded migrations.
    pub async fn connect(database_url: &str) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "PostgreSQL connection failed");
                StoreError::Unavailable
            })?;
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "PostgreSQL migration failed");
                StoreError::Unavailable
            })?;
        Ok(Self { pool })
    }

    /// Borrow the underlying pool for readiness checks and operational tooling.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl GatewayStore for PostgresStore {
    async fn ping(&self) -> Result<(), StoreError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| StoreError::Unavailable)
    }

    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO tenants (id, name, daily_token_limit) VALUES ($1, $1, $2) \
             ON CONFLICT (id) DO UPDATE SET daily_token_limit = EXCLUDED.daily_token_limit",
        )
        .bind(tenant.id)
        .bind(to_i64(tenant.daily_token_limit)?)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(map_sqlx)
    }

    async fn find_tenant(&self, tenant_id: &str) -> Result<Option<TenantRecord>, StoreError> {
        sqlx::query("SELECT id, daily_token_limit FROM tenants WHERE id = $1")
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx)?
            .map(|row| {
                Ok(TenantRecord {
                    id: row.try_get("id").map_err(|_| StoreError::Unavailable)?,
                    daily_token_limit: from_i64(
                        row.try_get("daily_token_limit")
                            .map_err(|_| StoreError::Unavailable)?,
                    )?,
                })
            })
            .transpose()
    }

    async fn plan_requests_per_minute(&self, tenant_id: &str) -> Result<Option<u64>, StoreError> {
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT (limits->>'requests_per_minute')::BIGINT FROM tenant_plan_profiles WHERE tenant_id=$1",
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .flatten()
        .map(from_i64)
        .transpose()
    }

    async fn plan_resource_usage(
        &self,
        tenant_id: &str,
        resource: &str,
    ) -> Result<Option<(u64, u64)>, StoreError> {
        let count_sql = match resource {
            "members" => {
                "SELECT (SELECT count(*) FROM tenant_memberships WHERE tenant_id=$1)+(SELECT count(*) FROM tenant_invitations WHERE tenant_id=$1 AND accepted_at IS NULL AND expires_at>now())"
            }
            "active_api_keys" => {
                "SELECT count(*) FROM virtual_keys WHERE tenant_id=$1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>now())"
            }
            "mcp_servers" => "SELECT count(*) FROM mcp_servers WHERE tenant_id=$1 AND enabled",
            "security_patterns" => {
                "SELECT count(*) FROM security_custom_patterns WHERE tenant_id=$1 AND enabled"
            }
            "fallback_targets" => {
                "SELECT count(*) FROM model_route_targets t JOIN model_routes r ON r.id=t.route_id WHERE r.tenant_id=$1 AND t.enabled AND t.priority>1"
            }
            _ => return Err(StoreError::Unavailable),
        };
        let limit: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT (limits->>$2)::BIGINT FROM tenant_plan_profiles WHERE tenant_id=$1",
        )
        .bind(tenant_id)
        .bind(resource)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .flatten();
        let Some(limit) = limit else {
            return Ok(None);
        };
        let current: i64 = sqlx::query_scalar(count_sql)
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(Some((from_i64(current)?, from_i64(limit)?)))
    }

    async fn plan_feature_enabled(
        &self,
        tenant_id: &str,
        feature: &str,
    ) -> Result<Option<bool>, StoreError> {
        match feature {
            "custom_security_policy"
            | "output_inspection"
            | "mcp_result_inspection"
            | "human_approval" => {}
            _ => return Err(StoreError::Unavailable),
        }
        sqlx::query_scalar::<_, Option<bool>>(
            "SELECT (features->>$2)::boolean FROM tenant_plan_profiles WHERE tenant_id=$1",
        )
        .bind(tenant_id)
        .bind(feature)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)
        .map(Option::flatten)
    }

    async fn insert_virtual_key(&self, key: VirtualKeyRecord) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO virtual_keys \
             (id, display_name, lookup_prefix, secret_hash, tenant_id, project_id, user_id, scopes, expires_at, revoked_at, daily_token_limit, allowed_models, daily_request_limit, monthly_budget) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        )
        .bind(key.id)
        .bind(key.display_name)
        .bind(key.lookup_prefix)
        .bind(key.secret_hash)
        .bind(key.tenant_id)
        .bind(key.project_id)
        .bind(key.user_id)
        .bind(key.scopes)
        .bind(key.expires_at)
        .bind(key.revoked_at)
        .bind(to_i64(key.daily_token_limit)?)
        .bind(key.allowed_models)
        .bind(key.daily_request_limit.map(to_i64).transpose()?)
        .bind(key.monthly_budget)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(map_sqlx)
    }

    async fn find_virtual_key_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<VirtualKeyRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT id, display_name, lookup_prefix, secret_hash, tenant_id, project_id, user_id, scopes, \
             expires_at, revoked_at, daily_token_limit, allowed_models, daily_request_limit, monthly_budget \
             FROM virtual_keys v WHERE lookup_prefix = $1 \
             AND NOT EXISTS (SELECT 1 FROM plan_resource_suspensions s \
                 WHERE s.tenant_id=v.tenant_id AND s.resource_kind='active_api_keys' \
                   AND s.resource_id=v.id::text AND s.restored_at IS NULL)",
        )
        .bind(prefix)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(virtual_key_from_row).transpose()
    }

    async fn touch_virtual_key(&self, key_id: Uuid) -> Result<(), StoreError> {
        sqlx::query("UPDATE virtual_keys SET last_used_at=now() WHERE id=$1")
            .bind(key_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_sqlx)
    }

    async fn revoke_virtual_key(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        key_id: Uuid,
    ) -> Result<bool, StoreError> {
        sqlx::query(
            "UPDATE virtual_keys SET revoked_at = COALESCE(revoked_at, now()) \
             WHERE tenant_id = $1 AND id = $2 AND ($3::text IS NULL OR project_id = $3)",
        )
        .bind(tenant_id)
        .bind(key_id)
        .bind(project_id)
        .execute(&self.pool)
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(map_sqlx)
    }

    async fn reserve_quota(&self, reservation: QuotaReservation) -> Result<bool, StoreError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx)?;
        // A managed plan cap is tenant-wide even when callers spread traffic
        // across many virtual keys. Locking the projection row serializes the
        // usage + pending reservation check.
        let monthly_plan_limit = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT (limits->>'routed_tokens_per_month')::BIGINT FROM tenant_plan_profiles \
             WHERE tenant_id=$1 FOR UPDATE",
        )
        .bind(&reservation.tenant_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_sqlx)?
        .flatten();
        if let Some(monthly_plan_limit) = monthly_plan_limit {
            let used: i64 = sqlx::query_scalar(
                "SELECT COALESCE(SUM(total_tokens),0)::BIGINT FROM usage_events \
                 WHERE tenant_id=$1 AND occurred_at>=date_trunc('month',now())",
            )
            .bind(&reservation.tenant_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx)?;
            let pending: i64 = sqlx::query_scalar(
                "SELECT COALESCE(SUM(prompt_tokens+completion_tokens),0)::BIGINT \
                 FROM quota_reservations WHERE tenant_id=$1",
            )
            .bind(&reservation.tenant_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx)?;
            let requested = to_i64(reservation.reserved_tokens())?;
            if used.saturating_add(pending).saturating_add(requested) > monthly_plan_limit {
                transaction.rollback().await.map_err(map_sqlx)?;
                return Ok(false);
            }
        }
        let limit: i64 = match &reservation.owner {
            QuotaOwner::Tenant(id) => sqlx::query_scalar(
                "SELECT daily_token_limit FROM tenants WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx)?,
            QuotaOwner::VirtualKey(id) => sqlx::query_scalar(
                "SELECT daily_token_limit FROM virtual_keys \
                 WHERE id = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now()) FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx)?,
        }
        .ok_or(StoreError::NotFound)?;
        if let QuotaOwner::VirtualKey(id) = &reservation.owner {
            let constraints = sqlx::query(
                "SELECT daily_request_limit, monthly_budget FROM virtual_keys
                 WHERE id=$1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())",
            )
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_sqlx)?
            .ok_or(StoreError::NotFound)?;
            let principal = format!("virtual-key:{id}");
            if let Some(request_limit) = constraints
                .try_get::<Option<i64>, _>("daily_request_limit")
                .map_err(|_| StoreError::Unavailable)?
            {
                let requests: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM usage_events
                     WHERE principal_id=$1 AND occurred_at>=date_trunc('day',now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'",
                )
                .bind(&principal)
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx)?;
                if requests >= request_limit {
                    transaction.rollback().await.map_err(map_sqlx)?;
                    return Ok(false);
                }
            }
            if let Some(budget) = constraints
                .try_get::<Option<rust_decimal::Decimal>, _>("monthly_budget")
                .map_err(|_| StoreError::Unavailable)?
            {
                let spent: rust_decimal::Decimal = sqlx::query_scalar(
                    "SELECT COALESCE(SUM(estimated_cost),0) FROM usage_events
                     WHERE principal_id=$1 AND occurred_at>=date_trunc('month',now())",
                )
                .bind(&principal)
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_sqlx)?;
                if spent >= budget {
                    transaction.rollback().await.map_err(map_sqlx)?;
                    return Ok(false);
                }
            }
        }

        let start_of_day = "date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'";
        let used: i64 = match &reservation.owner {
            QuotaOwner::Tenant(_) => sqlx::query_scalar(&format!(
                "SELECT COALESCE(SUM(total_tokens), 0)::BIGINT FROM usage_events \
                 WHERE tenant_id = $1 AND occurred_at >= {start_of_day}"
            ))
            .bind(&reservation.tenant_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx)?,
            QuotaOwner::VirtualKey(id) => sqlx::query_scalar(&format!(
                "SELECT COALESCE(SUM(total_tokens), 0)::BIGINT FROM usage_events \
                 WHERE principal_id = $1 AND occurred_at >= {start_of_day}"
            ))
            .bind(format!("virtual-key:{id}"))
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_sqlx)?,
        };
        let pending: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0)::BIGINT \
             FROM quota_reservations WHERE owner_kind = $1 AND owner_id = $2",
        )
        .bind(reservation.owner.kind())
        .bind(reservation.owner.id())
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        let requested = to_i64(reservation.reserved_tokens())?;
        if used.saturating_add(pending).saturating_add(requested) > limit {
            transaction.rollback().await.map_err(map_sqlx)?;
            return Ok(false);
        }
        if !enforce_scope_limits(&mut transaction, &reservation, requested).await? {
            transaction.rollback().await.map_err(map_sqlx)?;
            return Ok(false);
        }
        sqlx::query(
            "INSERT INTO quota_reservations \
             (reservation_id, request_id, owner_kind, owner_id, tenant_id, project_id, principal_id, user_id, provider, requested_model, upstream_model, prompt_tokens, completion_tokens, expires_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        )
        .bind(reservation.reservation_id)
        .bind(reservation.request_id)
        .bind(reservation.owner.kind())
        .bind(reservation.owner.id())
        .bind(&reservation.tenant_id)
        .bind(&reservation.project_id)
        .bind(&reservation.principal_id)
        .bind(&reservation.user_id)
        .bind(&reservation.provider)
        .bind(&reservation.requested_model)
        .bind(&reservation.upstream_model)
        .bind(to_i64(reservation.prompt_tokens)?)
        .bind(to_i64(reservation.completion_tokens)?)
        .bind(reservation.expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        transaction.commit().await.map_err(map_sqlx)?;
        Ok(true)
    }

    async fn finalize_usage(
        &self,
        reservation_id: Uuid,
        event: UsageEvent,
    ) -> Result<(), StoreError> {
        let mut transaction = self.pool.begin().await.map_err(map_sqlx)?;
        let webhook_payload = serde_json::json!({
            "event_id": event.event_id,
            "request_id": event.request_id,
            "operation": "inference",
            "tenant_id": event.tenant_id,
            "project_id": event.project_id,
            "principal_id": event.principal_id,
            "provider": event.provider,
            "requested_model": event.requested_model,
            "upstream_model": event.upstream_model,
            "usage": event.usage,
            "estimated_cost": event.estimated_cost,
            "status": status_str(event.status),
            "occurred_at": event.occurred_at,
        });
        sqlx::query(
            "INSERT INTO usage_events \
             (event_id, request_id, tenant_id, project_id, principal_id, user_id, provider, requested_model, upstream_model, prompt_tokens, completion_tokens, total_tokens, estimated_cost, usage_estimated, status, latency_ms, occurred_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) \
             ON CONFLICT (request_id) DO NOTHING",
        )
        .bind(event.event_id)
        .bind(event.request_id)
        .bind(&event.tenant_id)
        .bind(&event.project_id)
        .bind(&event.principal_id)
        .bind(&event.user_id)
        .bind(&event.provider)
        .bind(&event.requested_model)
        .bind(&event.upstream_model)
        .bind(to_i64(event.usage.prompt_tokens)?)
        .bind(to_i64(event.usage.completion_tokens)?)
        .bind(to_i64(event.usage.total_tokens())?)
        .bind(event.estimated_cost)
        .bind(event.usage.estimated)
        .bind(status_str(event.status))
        .bind(event.latency_ms.map(to_i64).transpose()?)
        .bind(event.occurred_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        let webhooks = sqlx::query(
            "SELECT webhook_id FROM billing_webhooks WHERE tenant_id=$1 AND enabled=true",
        )
        .bind(&event.tenant_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_sqlx)?;
        for webhook in webhooks {
            sqlx::query("INSERT INTO billing_outbox (event_id,tenant_id,webhook_id,source_request_id,payload) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (webhook_id,source_request_id) WHERE source_request_id IS NOT NULL DO NOTHING")
                .bind(Uuid::now_v7()).bind(&event.tenant_id).bind(webhook.try_get::<Uuid,_>("webhook_id").map_err(|_| StoreError::Unavailable)?).bind(event.request_id).bind(&webhook_payload).execute(&mut *transaction).await.map_err(map_sqlx)?;
        }
        sqlx::query("DELETE FROM quota_reservations WHERE reservation_id = $1")
            .bind(reservation_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_sqlx)?;
        transaction.commit().await.map_err(map_sqlx)
    }

    async fn release_reservation(&self, reservation_id: Uuid) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM quota_reservations WHERE reservation_id = $1")
            .bind(reservation_id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_sqlx)
    }

    async fn expired_reservations(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<QuotaReservation>, StoreError> {
        let rows = sqlx::query(
            "SELECT reservation_id, request_id, owner_kind, owner_id, tenant_id, project_id, principal_id, \
             user_id, provider, requested_model, upstream_model, prompt_tokens, completion_tokens, expires_at \
             FROM quota_reservations WHERE expires_at <= $1 ORDER BY expires_at LIMIT 100",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(reservation_from_row).collect()
    }

    async fn usage_by_request(&self, request_id: Uuid) -> Result<Option<UsageEvent>, StoreError> {
        sqlx::query(
            "SELECT event_id, request_id, tenant_id, project_id, principal_id, user_id, provider, requested_model, \
             upstream_model, prompt_tokens, completion_tokens, estimated_cost, usage_estimated, status, latency_ms, occurred_at \
             FROM usage_events WHERE request_id = $1",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .map(usage_from_row)
        .transpose()
    }
}

fn virtual_key_from_row(row: sqlx::postgres::PgRow) -> Result<VirtualKeyRecord, StoreError> {
    Ok(VirtualKeyRecord {
        id: row.try_get("id").map_err(|_| StoreError::Unavailable)?,
        display_name: row
            .try_get("display_name")
            .map_err(|_| StoreError::Unavailable)?,
        lookup_prefix: row
            .try_get("lookup_prefix")
            .map_err(|_| StoreError::Unavailable)?,
        secret_hash: row
            .try_get("secret_hash")
            .map_err(|_| StoreError::Unavailable)?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| StoreError::Unavailable)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| StoreError::Unavailable)?,
        user_id: row
            .try_get("user_id")
            .map_err(|_| StoreError::Unavailable)?,
        scopes: row.try_get("scopes").map_err(|_| StoreError::Unavailable)?,
        expires_at: row
            .try_get("expires_at")
            .map_err(|_| StoreError::Unavailable)?,
        revoked_at: row
            .try_get("revoked_at")
            .map_err(|_| StoreError::Unavailable)?,
        daily_token_limit: from_i64(
            row.try_get("daily_token_limit")
                .map_err(|_| StoreError::Unavailable)?,
        )?,
        allowed_models: row
            .try_get("allowed_models")
            .map_err(|_| StoreError::Unavailable)?,
        daily_request_limit: row
            .try_get::<Option<i64>, _>("daily_request_limit")
            .map_err(|_| StoreError::Unavailable)?
            .map(from_i64)
            .transpose()?,
        monthly_budget: row
            .try_get("monthly_budget")
            .map_err(|_| StoreError::Unavailable)?,
    })
}

fn reservation_from_row(row: sqlx::postgres::PgRow) -> Result<QuotaReservation, StoreError> {
    let kind: String = row
        .try_get("owner_kind")
        .map_err(|_| StoreError::Unavailable)?;
    let owner_id: String = row
        .try_get("owner_id")
        .map_err(|_| StoreError::Unavailable)?;
    let owner = match kind.as_str() {
        "tenant" => QuotaOwner::Tenant(owner_id),
        "virtual_key" => {
            QuotaOwner::VirtualKey(owner_id.parse().map_err(|_| StoreError::Unavailable)?)
        }
        _ => return Err(StoreError::Unavailable),
    };
    Ok(QuotaReservation {
        reservation_id: row
            .try_get("reservation_id")
            .map_err(|_| StoreError::Unavailable)?,
        request_id: row
            .try_get("request_id")
            .map_err(|_| StoreError::Unavailable)?,
        owner,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| StoreError::Unavailable)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| StoreError::Unavailable)?,
        principal_id: row
            .try_get("principal_id")
            .map_err(|_| StoreError::Unavailable)?,
        user_id: row
            .try_get("user_id")
            .map_err(|_| StoreError::Unavailable)?,
        provider: row
            .try_get("provider")
            .map_err(|_| StoreError::Unavailable)?,
        requested_model: row
            .try_get("requested_model")
            .map_err(|_| StoreError::Unavailable)?,
        upstream_model: row
            .try_get("upstream_model")
            .map_err(|_| StoreError::Unavailable)?,
        prompt_tokens: from_i64(
            row.try_get("prompt_tokens")
                .map_err(|_| StoreError::Unavailable)?,
        )?,
        completion_tokens: from_i64(
            row.try_get("completion_tokens")
                .map_err(|_| StoreError::Unavailable)?,
        )?,
        expires_at: row
            .try_get("expires_at")
            .map_err(|_| StoreError::Unavailable)?,
    })
}

fn usage_from_row(row: sqlx::postgres::PgRow) -> Result<UsageEvent, StoreError> {
    let status: String = row.try_get("status").map_err(|_| StoreError::Unavailable)?;
    Ok(UsageEvent {
        event_id: row
            .try_get("event_id")
            .map_err(|_| StoreError::Unavailable)?,
        request_id: row
            .try_get("request_id")
            .map_err(|_| StoreError::Unavailable)?,
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|_| StoreError::Unavailable)?,
        project_id: row
            .try_get("project_id")
            .map_err(|_| StoreError::Unavailable)?,
        principal_id: row
            .try_get("principal_id")
            .map_err(|_| StoreError::Unavailable)?,
        user_id: row
            .try_get("user_id")
            .map_err(|_| StoreError::Unavailable)?,
        provider: row
            .try_get("provider")
            .map_err(|_| StoreError::Unavailable)?,
        requested_model: row
            .try_get("requested_model")
            .map_err(|_| StoreError::Unavailable)?,
        upstream_model: row
            .try_get("upstream_model")
            .map_err(|_| StoreError::Unavailable)?,
        usage: TokenUsage {
            prompt_tokens: from_i64(
                row.try_get("prompt_tokens")
                    .map_err(|_| StoreError::Unavailable)?,
            )?,
            completion_tokens: from_i64(
                row.try_get("completion_tokens")
                    .map_err(|_| StoreError::Unavailable)?,
            )?,
            estimated: row
                .try_get("usage_estimated")
                .map_err(|_| StoreError::Unavailable)?,
        },
        estimated_cost: row
            .try_get("estimated_cost")
            .map_err(|_| StoreError::Unavailable)?,
        status: match status.as_str() {
            "succeeded" => UsageStatus::Succeeded,
            "provider_failed" => UsageStatus::ProviderFailed,
            "interrupted" => UsageStatus::Interrupted,
            _ => return Err(StoreError::Unavailable),
        },
        latency_ms: row
            .try_get::<Option<i64>, _>("latency_ms")
            .map_err(|_| StoreError::Unavailable)?
            .map(from_i64)
            .transpose()?,
        occurred_at: row
            .try_get("occurred_at")
            .map_err(|_| StoreError::Unavailable)?,
    })
}

fn status_str(status: UsageStatus) -> &'static str {
    match status {
        UsageStatus::Succeeded => "succeeded",
        UsageStatus::ProviderFailed => "provider_failed",
        UsageStatus::Interrupted => "interrupted",
    }
}

fn to_i64(value: u64) -> Result<i64, StoreError> {
    value.try_into().map_err(|_| StoreError::Conflict)
}

fn from_i64(value: i64) -> Result<u64, StoreError> {
    value.try_into().map_err(|_| StoreError::Unavailable)
}

fn map_sqlx(error: sqlx::Error) -> StoreError {
    match error {
        sqlx::Error::RowNotFound => StoreError::NotFound,
        sqlx::Error::Database(ref database) if database.is_unique_violation() => {
            StoreError::Conflict
        }
        _ => StoreError::Unavailable,
    }
}

async fn enforce_scope_limits(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    reservation: &QuotaReservation,
    requested: i64,
) -> Result<bool, StoreError> {
    let virtual_key_id = reservation.owner.id();
    let limits = sqlx::query(
        "SELECT scope_kind,scope_id,period,token_limit,cost_limit,concurrent_limit,requests_per_minute \
         FROM quota_limits WHERE tenant_id=$1 AND (\
           (scope_kind='tenant' AND scope_id=$1) OR \
           (scope_kind='project' AND scope_id=$2) OR \
           (scope_kind='principal' AND scope_id=$3) OR \
           (scope_kind='virtual_key' AND scope_id=$4)) \
         UNION ALL SELECT body->>'scope_kind',body->>'scope_id',body->>'period',
           NULLIF(body->>'token_limit','')::BIGINT,NULLIF(body->>'cost_limit','')::NUMERIC,
           NULLIF(body->>'concurrent_limit','')::BIGINT,
           NULLIF(body->>'requests_per_minute','')::BIGINT
         FROM admin_resources WHERE kind='quota_limits' AND tenant_id=$1
           AND enabled=true AND retired_at IS NULL AND (\
           (body->>'scope_kind'='tenant' AND body->>'scope_id'=$1) OR \
           (body->>'scope_kind'='project' AND body->>'scope_id'=$2) OR \
           (body->>'scope_kind'='principal' AND body->>'scope_id'=$3) OR \
           (body->>'scope_kind'='virtual_key' AND body->>'scope_id'=$4))",
    )
    .bind(&reservation.tenant_id)
    .bind(reservation.project_id.as_deref().unwrap_or(""))
    .bind(&reservation.principal_id)
    .bind(&virtual_key_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_sqlx)?;

    for limit in limits {
        let kind: String = limit
            .try_get("scope_kind")
            .map_err(|_| StoreError::Unavailable)?;
        let scope_id: String = limit
            .try_get("scope_id")
            .map_err(|_| StoreError::Unavailable)?;
        let period: String = limit
            .try_get("period")
            .map_err(|_| StoreError::Unavailable)?;
        let (usage_column, usage_id, reservation_clause, reservation_id) = match kind.as_str() {
            "tenant" => ("tenant_id", scope_id.clone(), "tenant_id", scope_id.clone()),
            "project" => (
                "project_id",
                scope_id.clone(),
                "project_id",
                scope_id.clone(),
            ),
            "principal" => (
                "principal_id",
                scope_id.clone(),
                "principal_id",
                scope_id.clone(),
            ),
            "virtual_key" => (
                "principal_id",
                format!("virtual-key:{scope_id}"),
                "owner_id",
                scope_id.clone(),
            ),
            _ => return Err(StoreError::Unavailable),
        };
        let period_start = match period.as_str() {
            "minute" => "date_trunc('minute', now())",
            "day" => "date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'",
            "month" => "date_trunc('month', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'",
            _ => return Err(StoreError::Unavailable),
        };
        let used_tokens: i64 = sqlx::query_scalar(&format!(
            "SELECT COALESCE(SUM(total_tokens),0)::BIGINT FROM usage_events WHERE {usage_column}=$1 AND occurred_at >= {period_start}"
        ))
        .bind(&usage_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx)?;
        let pending_tokens: i64 = sqlx::query_scalar(&format!(
            "SELECT COALESCE(SUM(prompt_tokens+completion_tokens),0)::BIGINT FROM quota_reservations WHERE {reservation_clause}=$1"
        ))
        .bind(&reservation_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx)?;
        if limit
            .try_get::<Option<i64>, _>("token_limit")
            .map_err(|_| StoreError::Unavailable)?
            .is_some_and(|value| {
                used_tokens
                    .saturating_add(pending_tokens)
                    .saturating_add(requested)
                    > value
            })
        {
            return Ok(false);
        }
        let active: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*)::BIGINT FROM quota_reservations WHERE {reservation_clause}=$1 AND expires_at>now()"
        ))
        .bind(&reservation_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx)?;
        if limit
            .try_get::<Option<i64>, _>("concurrent_limit")
            .map_err(|_| StoreError::Unavailable)?
            .is_some_and(|value| active >= value)
        {
            return Ok(false);
        }
        let recent_requests: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*)::BIGINT FROM usage_events WHERE {usage_column}=$1 AND occurred_at >= date_trunc('minute',now())"
        ))
        .bind(&usage_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx)?;
        if limit
            .try_get::<Option<i64>, _>("requests_per_minute")
            .map_err(|_| StoreError::Unavailable)?
            .is_some_and(|value| recent_requests >= value)
        {
            return Ok(false);
        }
        let cost_exceeded: bool = sqlx::query_scalar(&format!(
            "SELECT COALESCE(SUM(estimated_cost),0) > COALESCE($2,999999999999::numeric) FROM usage_events WHERE {usage_column}=$1 AND occurred_at >= {period_start}"
        ))
        .bind(&usage_id)
        .bind(limit.try_get::<Option<rust_decimal::Decimal>, _>("cost_limit").map_err(|_| StoreError::Unavailable)?)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_sqlx)?;
        if cost_exceeded {
            return Ok(false);
        }
    }
    Ok(true)
}
