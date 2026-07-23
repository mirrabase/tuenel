use async_trait::async_trait;
use gateway_admin::{
    AdminError, AdminRepository, ListQuery, Mutation, MutationContext, OperationalView, Page,
    ResourceKind,
};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::PostgresStore;

#[derive(Clone, Debug)]
pub struct RuntimeResource {
    pub kind: String,
    pub id: String,
    pub tenant_id: Option<String>,
    pub body: Value,
}

impl PostgresStore {
    pub async fn bootstrap_runtime_resource(
        &self,
        kind: ResourceKind,
        id: &str,
        body: Value,
    ) -> Result<(), AdminError> {
        sqlx::query(
            "INSERT INTO admin_resources(kind,id,tenant_id,body)
             VALUES($1,$2,NULL,$3) ON CONFLICT(kind,id) DO NOTHING",
        )
        .bind(kind.as_str())
        .bind(id)
        .bind(body)
        .execute(self.pool())
        .await
        .map_err(|_| AdminError::Unavailable)
        .map(|_| ())
    }

    pub async fn runtime_resources(&self) -> Result<Vec<RuntimeResource>, AdminError> {
        sqlx::query(
            "SELECT kind,id,tenant_id,body FROM admin_resources
             WHERE kind IN ('providers','model_routes')
               AND tenant_id IS NULL AND enabled=true AND retired_at IS NULL
             ORDER BY kind,id",
        )
        .fetch_all(self.pool())
        .await
        .map_err(|_| AdminError::Unavailable)?
        .into_iter()
        .map(|row| {
            Ok(RuntimeResource {
                kind: row.try_get("kind").map_err(|_| AdminError::Unavailable)?,
                id: row.try_get("id").map_err(|_| AdminError::Unavailable)?,
                tenant_id: row
                    .try_get("tenant_id")
                    .map_err(|_| AdminError::Unavailable)?,
                body: row.try_get("body").map_err(|_| AdminError::Unavailable)?,
            })
        })
        .collect()
    }
}

#[async_trait]
impl AdminRepository for PostgresStore {
    async fn resource_secret_ref(
        &self,
        kind: ResourceKind,
        id: &str,
    ) -> Result<Option<(String, gateway_types::SecretRef)>, AdminError> {
        let row = sqlx::query(
            "SELECT COALESCE(tenant_id,body->>'secret_tenant_id') AS tenant_id,
                    body->>'secret_ref' AS secret_ref
             FROM admin_resources WHERE kind=$1 AND id=$2",
        )
        .bind(kind.as_str())
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(|_| AdminError::Unavailable)?;
        Ok(row.and_then(|row| {
            let tenant_id = row
                .try_get::<Option<String>, _>("tenant_id")
                .ok()
                .flatten()?;
            let secret_ref = row
                .try_get::<Option<String>, _>("secret_ref")
                .ok()
                .flatten()?;
            Some((tenant_id, gateway_types::SecretRef(secret_ref)))
        }))
    }
    async fn list_resources(
        &self,
        kind: ResourceKind,
        query: &ListQuery,
    ) -> Result<Page, AdminError> {
        let rows = sqlx::query(
            "SELECT body || jsonb_build_object(
                'id',id,'tenant_id',tenant_id,'version',version,'enabled',enabled,
                'retired_at',retired_at,'created_at',created_at,'updated_at',updated_at
             ) AS resource
             FROM admin_resources
             WHERE kind=$1 AND retired_at IS NULL
               AND ($2::text IS NULL OR tenant_id=$2)
               AND ($3::text IS NULL OR body::text ILIKE '%' || $3 || '%')
               AND ($4::text IS NULL OR updated_at < (
                    SELECT updated_at FROM admin_resources WHERE kind=$1 AND id=$4
               ) OR (updated_at = (
                    SELECT updated_at FROM admin_resources WHERE kind=$1 AND id=$4
               ) AND id < $4))
             ORDER BY updated_at DESC,id DESC LIMIT $5",
        )
        .bind(kind.as_str())
        .bind(&query.tenant_id)
        .bind(
            query
                .query
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .bind(&query.cursor)
        .bind(i64::from(query.limit()) + 1)
        .fetch_all(self.pool())
        .await
        .map_err(|_| AdminError::Unavailable)?;
        page(rows, query.limit())
    }

    async fn create_resource(
        &self,
        kind: ResourceKind,
        mut body: Value,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError> {
        let object = body.as_object_mut().ok_or(AdminError::Invalid)?;
        let id = object
            .remove("id")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let tenant_id = object
            .remove("tenant_id")
            .and_then(|value| value.as_str().map(str::to_owned))
            .or_else(|| {
                (!context.gateway_admin)
                    .then(|| context.tenant_id.clone())
                    .flatten()
            });
        if id.trim().is_empty() || tenant_id.as_deref().is_some_and(str::is_empty) {
            return Err(AdminError::Invalid);
        }
        let mut transaction = self.pool().begin().await.map_err(admin_sqlx)?;
        let resource: Value = sqlx::query_scalar(
            "INSERT INTO admin_resources(kind,id,tenant_id,body)
             VALUES($1,$2,$3,$4)
             RETURNING body || jsonb_build_object(
                'id',id,'tenant_id',tenant_id,'version',version,'enabled',enabled,
                'retired_at',retired_at,'created_at',created_at,'updated_at',updated_at
             )",
        )
        .bind(kind.as_str())
        .bind(&id)
        .bind(&tenant_id)
        .bind(&body)
        .fetch_one(&mut *transaction)
        .await
        .map_err(admin_sqlx)?;
        let audit_id = audit(
            &mut transaction,
            kind,
            &id,
            "created",
            context,
            tenant_id.as_deref(),
        )
        .await?;
        transaction.commit().await.map_err(admin_sqlx)?;
        Ok(Mutation { resource, audit_id })
    }

    async fn update_resource(
        &self,
        kind: ResourceKind,
        id: &str,
        version: u64,
        mut body: Value,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError> {
        body.as_object_mut()
            .ok_or(AdminError::Invalid)?
            .remove("credential");
        let mut transaction = self.pool().begin().await.map_err(admin_sqlx)?;
        let row = sqlx::query(
            "UPDATE admin_resources
             SET body=body || $4,version=version+1,updated_at=now()
             WHERE kind=$1 AND id=$2 AND version=$3 AND retired_at IS NULL
               AND ($5 OR tenant_id=$6)
             RETURNING tenant_id,body || jsonb_build_object(
                'id',id,'tenant_id',tenant_id,'version',version,'enabled',enabled,
                'retired_at',retired_at,'created_at',created_at,'updated_at',updated_at
             ) AS resource",
        )
        .bind(kind.as_str())
        .bind(id)
        .bind(i64::try_from(version).map_err(|_| AdminError::Invalid)?)
        .bind(&body)
        .bind(context.gateway_admin)
        .bind(&context.tenant_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(admin_sqlx)?;
        let row = match row {
            Some(row) => row,
            None => return Err(resource_miss(&mut transaction, kind, id).await?),
        };
        let tenant_id: Option<String> = row
            .try_get("tenant_id")
            .map_err(|_| AdminError::Unavailable)?;
        let audit_id = audit(
            &mut transaction,
            kind,
            id,
            "updated",
            context,
            tenant_id.as_deref(),
        )
        .await?;
        let resource = row
            .try_get("resource")
            .map_err(|_| AdminError::Unavailable)?;
        transaction.commit().await.map_err(admin_sqlx)?;
        Ok(Mutation { resource, audit_id })
    }

    async fn retire_resource(
        &self,
        kind: ResourceKind,
        id: &str,
        version: u64,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError> {
        let mut transaction = self.pool().begin().await.map_err(admin_sqlx)?;
        let row = sqlx::query(
            "UPDATE admin_resources
             SET enabled=false,retired_at=COALESCE(retired_at,now()),version=version+1,updated_at=now()
             WHERE kind=$1 AND id=$2 AND version=$3
               AND ($4 OR tenant_id=$5)
             RETURNING tenant_id,body || jsonb_build_object(
                'id',id,'tenant_id',tenant_id,'version',version,'enabled',enabled,
                'retired_at',retired_at,'created_at',created_at,'updated_at',updated_at
             ) AS resource",
        )
        .bind(kind.as_str())
        .bind(id)
        .bind(i64::try_from(version).map_err(|_| AdminError::Invalid)?)
        .bind(context.gateway_admin)
        .bind(&context.tenant_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(admin_sqlx)?;
        let row = match row {
            Some(row) => row,
            None => return Err(resource_miss(&mut transaction, kind, id).await?),
        };
        let tenant_id: Option<String> = row
            .try_get("tenant_id")
            .map_err(|_| AdminError::Unavailable)?;
        let audit_id = audit(
            &mut transaction,
            kind,
            id,
            "retired",
            context,
            tenant_id.as_deref(),
        )
        .await?;
        let resource = row
            .try_get("resource")
            .map_err(|_| AdminError::Unavailable)?;
        transaction.commit().await.map_err(admin_sqlx)?;
        Ok(Mutation { resource, audit_id })
    }

    async fn operational(
        &self,
        view: OperationalView,
        query: &ListQuery,
    ) -> Result<Value, AdminError> {
        match view {
            OperationalView::Tenants => list_json(
                self,
                "SELECT jsonb_build_object('id',id,'name',name,'daily_token_limit',daily_token_limit) AS value
                 FROM tenants WHERE ($1::text IS NULL OR id=$1) ORDER BY id LIMIT $2",
                query,
            )
            .await,
            OperationalView::Members => list_json(
                self,
                "SELECT jsonb_build_object('tenant_id',m.tenant_id,'user_id',u.id,'email',u.email,'role',m.role,'created_at',m.created_at) AS value
                 FROM tenant_memberships m JOIN users u ON u.id=m.user_id
                 WHERE ($1::text IS NULL OR m.tenant_id=$1) ORDER BY m.created_at DESC,u.id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::VirtualKeys => list_json(
                self,
                "SELECT jsonb_build_object('id',id,'display_name',display_name,'key_prefix',lookup_prefix,'tenant_id',tenant_id,
                    'project_id',project_id,'user_id',user_id,'scopes',scopes,'expires_at',expires_at,
                    'revoked_at',revoked_at,'daily_token_limit',daily_token_limit,'created_at',created_at) AS value
                 FROM virtual_keys WHERE ($1::text IS NULL OR tenant_id=$1)
                 ORDER BY created_at DESC,id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::UsageEvents => list_json(
                self,
                "SELECT to_jsonb(u)-'metadata' AS value FROM usage_events u
                 WHERE ($1::text IS NULL OR tenant_id=$1) ORDER BY occurred_at DESC,event_id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::Reservations => list_json(
                self,
                "SELECT to_jsonb(q) AS value FROM quota_reservations q
                 WHERE ($1::text IS NULL OR tenant_id=$1) ORDER BY expires_at DESC,reservation_id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::McpInvocations => list_json(
                self,
                "SELECT to_jsonb(i)-'request_hash'-'result' AS value FROM mcp_invocations i
                 WHERE ($1::text IS NULL OR tenant_id=$1) ORDER BY created_at DESC,invocation_id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::AuditEvents => list_json(
                self,
                "SELECT to_jsonb(a) AS value FROM audit_events a
                 WHERE ($1::text IS NULL OR tenant_id=$1) ORDER BY occurred_at DESC,event_id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::BillingWebhooks => list_json(
                self,
                "SELECT jsonb_build_object('webhook_id',webhook_id,'tenant_id',tenant_id,'url',url,
                    'enabled',enabled,'maximum_attempts',maximum_attempts,'created_at',created_at) AS value
                 FROM billing_webhooks WHERE ($1::text IS NULL OR tenant_id=$1)
                 ORDER BY created_at DESC,webhook_id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::BillingOutbox => list_json(
                self,
                "SELECT jsonb_build_object('event_id',event_id,'tenant_id',tenant_id,'webhook_id',webhook_id,
                    'attempt_count',attempt_count,'next_attempt_at',next_attempt_at,'delivered_at',delivered_at,
                    'last_error',last_error,'created_at',created_at) AS value
                 FROM billing_outbox WHERE ($1::text IS NULL OR tenant_id=$1)
                 ORDER BY created_at DESC,event_id DESC LIMIT $2",
                query,
            )
            .await,
            OperationalView::UsageSummary => {
                let value = sqlx::query_scalar(
                    "SELECT jsonb_build_object(
                        'requests',COUNT(*),'input_tokens',COALESCE(SUM(prompt_tokens),0),
                        'output_tokens',COALESCE(SUM(completion_tokens),0),
                        'total_tokens',COALESCE(SUM(total_tokens),0),
                        'estimated_cost',COALESCE(SUM(estimated_cost),0))
                     FROM usage_events WHERE ($1::text IS NULL OR tenant_id=$1)",
                )
                .bind(&query.tenant_id)
                .fetch_one(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                Ok(value)
            }
            OperationalView::UsageSeries => {
                let rows = sqlx::query(
                    "SELECT jsonb_build_object('time',date_trunc('hour',occurred_at),
                        'requests',COUNT(*),'tokens',COALESCE(SUM(total_tokens),0),
                        'cost',COALESCE(SUM(estimated_cost),0)) AS value
                     FROM usage_events WHERE ($1::text IS NULL OR tenant_id=$1)
                     GROUP BY date_trunc('hour',occurred_at) ORDER BY date_trunc('hour',occurred_at) DESC LIMIT $2",
                )
                .bind(&query.tenant_id)
                .bind(i64::from(query.limit()))
                .fetch_all(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                Ok(json!({"data": values(rows)?, "next_cursor": null}))
            }
            OperationalView::Summary => summary(self, query, false).await,
            OperationalView::System => summary(self, query, true).await,
        }
    }

    async fn retry_billing(
        &self,
        event_id: Uuid,
        context: &MutationContext,
    ) -> Result<Mutation, AdminError> {
        let mut transaction = self.pool().begin().await.map_err(admin_sqlx)?;
        let row = sqlx::query(
            "UPDATE billing_outbox SET next_attempt_at=now(),last_error=NULL
             WHERE event_id=$1 AND delivered_at IS NULL AND ($2 OR tenant_id=$3)
             RETURNING tenant_id,jsonb_build_object('event_id',event_id,'tenant_id',tenant_id,
                'webhook_id',webhook_id,'attempt_count',attempt_count,'next_attempt_at',next_attempt_at,
                'delivered_at',delivered_at,'last_error',last_error,'created_at',created_at) AS resource",
        )
        .bind(event_id)
        .bind(context.gateway_admin)
        .bind(&context.tenant_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(admin_sqlx)?
        .ok_or(AdminError::NotFound)?;
        let tenant_id: String = row
            .try_get("tenant_id")
            .map_err(|_| AdminError::Unavailable)?;
        let audit_id = audit_raw(
            &mut transaction,
            "billing_outbox",
            &event_id.to_string(),
            "retry_scheduled",
            context,
            &tenant_id,
        )
        .await?;
        let resource = row
            .try_get("resource")
            .map_err(|_| AdminError::Unavailable)?;
        transaction.commit().await.map_err(admin_sqlx)?;
        Ok(Mutation { resource, audit_id })
    }
}

fn page(rows: Vec<sqlx::postgres::PgRow>, limit: u8) -> Result<Page, AdminError> {
    let mut data = values(rows)?;
    let next_cursor = (data.len() > usize::from(limit))
        .then(|| {
            data.pop()
                .and_then(|value| value.get("id")?.as_str().map(str::to_owned))
        })
        .flatten();
    Ok(Page { data, next_cursor })
}

fn values(rows: Vec<sqlx::postgres::PgRow>) -> Result<Vec<Value>, AdminError> {
    rows.into_iter()
        .map(|row| row.try_get("value").or_else(|_| row.try_get("resource")))
        .collect::<Result<_, _>>()
        .map_err(|_| AdminError::Unavailable)
}

async fn list_json(
    store: &PostgresStore,
    statement: &str,
    query: &ListQuery,
) -> Result<Value, AdminError> {
    let rows = sqlx::query(statement)
        .bind(&query.tenant_id)
        .bind(i64::from(query.limit()))
        .fetch_all(store.pool())
        .await
        .map_err(|_| AdminError::Unavailable)?;
    Ok(json!({"data": values(rows)?, "next_cursor": null}))
}

async fn summary(
    store: &PostgresStore,
    query: &ListQuery,
    system: bool,
) -> Result<Value, AdminError> {
    let usage: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('requests',COUNT(*),'tokens',COALESCE(SUM(total_tokens),0),
            'cost',COALESCE(SUM(estimated_cost),0)) FROM usage_events
         WHERE ($1::text IS NULL OR tenant_id=$1)",
    )
    .bind(&query.tenant_id)
    .fetch_one(store.pool())
    .await
    .map_err(|_| AdminError::Unavailable)?;
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_outbox WHERE delivered_at IS NULL
         AND ($1::text IS NULL OR tenant_id=$1)",
    )
    .bind(&query.tenant_id)
    .fetch_one(store.pool())
    .await
    .map_err(|_| AdminError::Unavailable)?;
    let mut value = json!({
        "generated_at": chrono::Utc::now(),
        "usage": usage,
        "billing_pending": pending,
        "partial": false
    });
    if system {
        let providers = sqlx::query(
            "SELECT jsonb_build_object('provider_id',provider_id,'status',status,
                'consecutive_failures',consecutive_failures,'rolling_latency_ms',rolling_latency_ms,
                'updated_at',updated_at) AS value FROM provider_health ORDER BY provider_id",
        )
        .fetch_all(store.pool())
        .await
        .map_err(|_| AdminError::Unavailable)?;
        value["providers"] = Value::Array(values(providers)?);
        value["runtime"] = json!({"state":"active","pending":false,"error":null});
    }
    Ok(value)
}

async fn resource_miss(
    transaction: &mut Transaction<'_, Postgres>,
    kind: ResourceKind,
    id: &str,
) -> Result<AdminError, AdminError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admin_resources WHERE kind=$1 AND id=$2)")
            .bind(kind.as_str())
            .bind(id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(admin_sqlx)?;
    Ok(if exists {
        AdminError::Conflict
    } else {
        AdminError::NotFound
    })
}

async fn audit(
    transaction: &mut Transaction<'_, Postgres>,
    kind: ResourceKind,
    id: &str,
    action: &str,
    context: &MutationContext,
    tenant_id: Option<&str>,
) -> Result<Uuid, AdminError> {
    audit_raw(
        transaction,
        kind.as_str(),
        id,
        action,
        context,
        tenant_id
            .or(context.tenant_id.as_deref())
            .ok_or(AdminError::Invalid)?,
    )
    .await
}

async fn audit_raw(
    transaction: &mut Transaction<'_, Postgres>,
    kind: &str,
    id: &str,
    action: &str,
    context: &MutationContext,
    tenant_id: &str,
) -> Result<Uuid, AdminError> {
    let audit_id = Uuid::now_v7();
    let key = format!("admin:{action}:{kind}:{id}:{}", context.request_id);
    sqlx::query_scalar(
        "INSERT INTO audit_events(event_id,idempotency_key,tenant_id,principal_id,request_id,event_type,payload)
         VALUES($1,$2,$3,$4,$5,$6,$7)
         ON CONFLICT(idempotency_key) DO UPDATE SET idempotency_key=EXCLUDED.idempotency_key
         RETURNING event_id",
    )
    .bind(audit_id)
    .bind(key)
    .bind(tenant_id)
    .bind(&context.actor)
    .bind(context.request_id)
    .bind(format!("admin.{kind}.{action}"))
    .bind(json!({"resource_kind":kind,"resource_id":id,"action":action}))
    .fetch_one(&mut **transaction)
    .await
    .map_err(admin_sqlx)
}

fn admin_sqlx(error: sqlx::Error) -> AdminError {
    if error
        .as_database_error()
        .and_then(|error| error.code())
        .as_deref()
        .is_some_and(|code| code == "23505")
    {
        AdminError::Conflict
    } else {
        AdminError::Unavailable
    }
}
