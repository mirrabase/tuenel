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
            "SELECT r.kind,r.id,r.tenant_id,r.body FROM admin_resources r
             WHERE r.kind IN ('providers','model_routes')
               AND r.enabled=true AND r.retired_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM plan_resource_suspensions s
                   WHERE s.tenant_id=r.tenant_id AND s.resource_kind=r.kind
                     AND s.resource_id=r.id AND s.restored_at IS NULL)
             ORDER BY r.kind,r.id",
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
                'retired_at',retired_at,'created_at',created_at,'updated_at',updated_at,
                'plan_suspended',EXISTS(SELECT 1 FROM plan_resource_suspensions s
                    WHERE s.tenant_id=admin_resources.tenant_id AND s.resource_kind=admin_resources.kind
                      AND s.resource_id=admin_resources.id AND s.restored_at IS NULL)
             ) AS resource
             FROM admin_resources
             WHERE kind=$1 AND retired_at IS NULL
               AND ($2::text IS NULL OR tenant_id=$2)
               AND ($3::text IS NULL OR body::text ILIKE '%' || $3 || '%')
               AND ($6::text IS NULL OR kind='projects' AND id=$6
                    OR body->>'project_id'=$6
                    OR (body->>'scope_kind'='project' AND body->>'scope_id'=$6))
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
        .bind(&query.project_id)
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
        let enabled = object
            .remove("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
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
        if let Some(tenant_id) = tenant_id.as_deref() {
            let limit_key = match kind {
                ResourceKind::Projects => Some("projects"),
                ResourceKind::Providers => Some("providers"),
                ResourceKind::Policies | ResourceKind::QuotaLimits => Some("budget_rules"),
                ResourceKind::ModelRoutes
                    if body
                        .get("priority")
                        .and_then(Value::as_u64)
                        .is_some_and(|value| value > 1) =>
                {
                    Some("fallback_targets")
                }
                _ => None,
            };
            if let Some(limit_key) = limit_key {
                let limit = sqlx::query_scalar::<_, Option<i64>>(
                    "SELECT (limits->>$2)::BIGINT FROM tenant_plan_profiles WHERE tenant_id=$1 FOR UPDATE",
                )
                .bind(tenant_id)
                .bind(limit_key)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(admin_sqlx)?
                .flatten();
                if let Some(limit) = limit {
                    let current: i64 = if kind == ResourceKind::ModelRoutes {
                        sqlx::query_scalar("SELECT count(*) FROM admin_resources WHERE tenant_id=$1 AND kind='model_routes' AND retired_at IS NULL AND (body->>'priority')::int>1")
                            .bind(tenant_id).fetch_one(&mut *transaction).await.map_err(admin_sqlx)?
                    } else if matches!(kind, ResourceKind::Policies | ResourceKind::QuotaLimits) {
                        sqlx::query_scalar("SELECT count(*) FROM admin_resources WHERE tenant_id=$1 AND kind IN ('policies','quota_limits') AND retired_at IS NULL")
                            .bind(tenant_id).fetch_one(&mut *transaction).await.map_err(admin_sqlx)?
                    } else {
                        sqlx::query_scalar("SELECT count(*) FROM admin_resources WHERE tenant_id=$1 AND kind=$2 AND retired_at IS NULL")
                            .bind(tenant_id).bind(kind.as_str()).fetch_one(&mut *transaction).await.map_err(admin_sqlx)?
                    };
                    if current >= limit {
                        transaction.rollback().await.map_err(admin_sqlx)?;
                        return Err(AdminError::PlanLimit);
                    }
                }
            }
        }
        let resource: Value = sqlx::query_scalar(
            "INSERT INTO admin_resources(kind,id,tenant_id,body,enabled)
             VALUES($1,$2,$3,$4,$5)
             RETURNING body || jsonb_build_object(
                'id',id,'tenant_id',tenant_id,'version',version,'enabled',enabled,
                'retired_at',retired_at,'created_at',created_at,'updated_at',updated_at
             )",
        )
        .bind(kind.as_str())
        .bind(&id)
        .bind(&tenant_id)
        .bind(&body)
        .bind(enabled)
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
        let object = body.as_object_mut().ok_or(AdminError::Invalid)?;
        object.remove("credential");
        let enabled = object.remove("enabled").and_then(|value| value.as_bool());
        let mut transaction = self.pool().begin().await.map_err(admin_sqlx)?;
        let row = sqlx::query(
            "UPDATE admin_resources
             SET body=body || $4,enabled=COALESCE($7,enabled),version=version+1,updated_at=now()
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
        .bind(enabled)
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
            OperationalView::VirtualKeys => {
                let rows = sqlx::query(
                    "SELECT jsonb_build_object('id',id,'display_name',display_name,
                        'key_prefix',lookup_prefix,'tenant_id',tenant_id,'project_id',project_id,
                        'user_id',user_id,'scopes',scopes,'expires_at',expires_at,
                        'revoked_at',revoked_at,'daily_token_limit',daily_token_limit,
                        'allowed_models',allowed_models,'daily_request_limit',daily_request_limit,
                        'monthly_budget',monthly_budget,'created_at',created_at,
                        'last_used_at',last_used_at) AS value
                     FROM virtual_keys WHERE ($1::text IS NULL OR tenant_id=$1)
                       AND ($3::text IS NULL OR project_id=$3)
                     ORDER BY created_at DESC,id DESC LIMIT $2",
                )
                .bind(&query.tenant_id)
                .bind(i64::from(query.limit()) + 1)
                .bind(&query.project_id)
                .fetch_all(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                page(rows, query.limit()).and_then(|page| {
                    serde_json::to_value(page).map_err(|_| AdminError::Unavailable)
                })
            }
            OperationalView::UsageEvents => {
                let rows = sqlx::query(
                    "SELECT (to_jsonb(u)-'metadata') || jsonb_build_object(
                        'api_key_name',CASE WHEN u.principal_id LIKE 'virtual-key:%' THEN
                            (SELECT COALESCE(v.display_name,v.lookup_prefix) FROM virtual_keys v
                             WHERE v.id::text=split_part(u.principal_id,':',2))
                            ELSE NULL END) AS value FROM usage_events u
                     WHERE ($1::text IS NULL OR tenant_id=$1)
                       AND ($3::text IS NULL OR project_id=$3)
                       AND occurred_at >= GREATEST(
                           COALESCE($4::timestamptz,'-infinity'::timestamptz),
                           COALESCE(now()-make_interval(days=>(SELECT (limits->>'history_days')::int FROM tenant_plan_profiles WHERE tenant_id=$1)),'-infinity'::timestamptz))
                       AND ($5::timestamptz IS NULL OR occurred_at <= $5::timestamptz)
                     ORDER BY occurred_at DESC,event_id DESC LIMIT $2",
                )
                .bind(&query.tenant_id)
                .bind(i64::from(query.limit()) + 1)
                .bind(&query.project_id)
                .bind(&query.from)
                .bind(&query.to)
                .fetch_all(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                Ok(json!({
                    "data": page(rows, query.limit())?.data,
                    "next_cursor": null
                }))
            }
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
            OperationalView::AuditEvents => {
                let rows = sqlx::query(
                    "SELECT to_jsonb(a) AS value FROM audit_events a
                     WHERE ($1::text IS NULL OR tenant_id=$1)
                       AND ($3::text IS NULL OR project_id=$3
                            OR payload->>'resource_id'=$3)
                       AND occurred_at >= GREATEST(
                           COALESCE($4::timestamptz,'-infinity'::timestamptz),
                           COALESCE(now()-make_interval(days=>(SELECT (limits->>'history_days')::int FROM tenant_plan_profiles WHERE tenant_id=$1)),'-infinity'::timestamptz))
                       AND ($5::timestamptz IS NULL OR occurred_at <= $5::timestamptz)
                     ORDER BY occurred_at DESC,event_id DESC LIMIT $2",
                )
                .bind(&query.tenant_id)
                .bind(i64::from(query.limit()) + 1)
                .bind(&query.project_id)
                .bind(&query.from)
                .bind(&query.to)
                .fetch_all(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                Ok(json!({"data": page(rows, query.limit())?.data, "next_cursor": null}))
            }
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
            OperationalView::BillingOverview => {
                let value = sqlx::query_scalar(
                    "SELECT jsonb_build_object(
                        'configured',COALESCE(b.configured,false),
                        'plan_name',b.plan_name,'billing_cycle',b.billing_cycle,
                        'request_allowance',b.request_allowance,
                        'token_allowance',b.token_allowance,
                        'payment_status',b.payment_status,
                        'upgrade_url',b.upgrade_url,'manage_url',b.manage_url,
                        'current_requests',(SELECT COUNT(*) FROM usage_events u
                            WHERE u.tenant_id=t.id
                              AND u.occurred_at>=date_trunc('month',now())))
                     FROM tenants t LEFT JOIN organization_billing b ON b.tenant_id=t.id
                     WHERE t.id=$1",
                )
                .bind(&query.tenant_id)
                .fetch_optional(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?
                .ok_or(AdminError::NotFound)?;
                Ok(value)
            }
            OperationalView::BillingInvoices => {
                let rows = sqlx::query(
                    "SELECT to_jsonb(i) AS value FROM organization_invoices i
                     WHERE ($1::text IS NULL OR tenant_id=$1)
                     ORDER BY issued_at DESC,id DESC LIMIT $2",
                )
                .bind(&query.tenant_id)
                .bind(i64::from(query.limit()) + 1)
                .fetch_all(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                page(rows, query.limit()).and_then(|page| {
                    serde_json::to_value(page).map_err(|_| AdminError::Unavailable)
                })
            }
            OperationalView::UsageSummary => {
                let value = sqlx::query_scalar(
                    "SELECT jsonb_build_object(
                        'requests',COUNT(*),'input_tokens',COALESCE(SUM(prompt_tokens),0),
                        'output_tokens',COALESCE(SUM(completion_tokens),0),
                        'total_tokens',COALESCE(SUM(total_tokens),0),
                        'estimated_cost',COALESCE(SUM(estimated_cost),0),
                        'average_cost_per_request',CASE WHEN COUNT(*)=0 THEN 0
                            ELSE COALESCE(SUM(estimated_cost),0)/COUNT(*) END,
                        'successful_requests',COUNT(*) FILTER (WHERE status='succeeded'),
                        'p95_latency_ms',percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms)
                            FILTER (WHERE latency_ms IS NOT NULL),
                        'success_rate',CASE WHEN COUNT(*)=0 THEN 0
                            ELSE 100.0*COUNT(*) FILTER (WHERE status='succeeded')/COUNT(*) END)
                     FROM usage_events
                     WHERE ($1::text IS NULL OR tenant_id=$1)
                       AND ($2::text IS NULL OR project_id=$2)
                       AND ($3::timestamptz IS NULL OR occurred_at >= $3::timestamptz)
                       AND ($4::timestamptz IS NULL OR occurred_at <= $4::timestamptz)",
                )
                .bind(&query.tenant_id)
                .bind(&query.project_id)
                .bind(&query.from)
                .bind(&query.to)
                .fetch_one(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                Ok(value)
            }
            OperationalView::UsageSeries => {
                let rows = sqlx::query(
                    "SELECT jsonb_build_object('time',date_trunc('hour',occurred_at),
                        'requests',COUNT(*),'input_tokens',COALESCE(SUM(prompt_tokens),0),
                        'output_tokens',COALESCE(SUM(completion_tokens),0),
                        'tokens',COALESCE(SUM(total_tokens),0),
                        'cost',COALESCE(SUM(estimated_cost),0)) AS value
                     FROM usage_events WHERE ($1::text IS NULL OR tenant_id=$1)
                       AND ($3::text IS NULL OR project_id=$3)
                       AND ($4::timestamptz IS NULL OR occurred_at >= $4::timestamptz)
                       AND ($5::timestamptz IS NULL OR occurred_at <= $5::timestamptz)
                     GROUP BY date_trunc('hour',occurred_at) ORDER BY date_trunc('hour',occurred_at) DESC LIMIT $2",
                )
                .bind(&query.tenant_id)
                .bind(i64::from(query.limit()))
                .bind(&query.project_id)
                .bind(&query.from)
                .bind(&query.to)
                .fetch_all(self.pool())
                .await
                .map_err(|_| AdminError::Unavailable)?;
                Ok(json!({"data": values(rows)?, "next_cursor": null}))
            }
            OperationalView::UsageBreakdowns => usage_breakdowns(self, query).await,
            OperationalView::ProviderHealth => provider_health(self, query).await,
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

async fn usage_breakdowns(store: &PostgresStore, query: &ListQuery) -> Result<Value, AdminError> {
    let projects = breakdown_rows(
        store,
        "SELECT jsonb_build_object(
            'id',COALESCE(u.project_id,'unattributed'),
            'name',COALESCE(p.body->>'name',u.project_id,'Unattributed'),
            'requests',COUNT(*),'input_tokens',COALESCE(SUM(u.prompt_tokens),0),
            'output_tokens',COALESCE(SUM(u.completion_tokens),0),
            'estimated_cost',COALESCE(SUM(u.estimated_cost),0),
            'success_rate',100.0*COUNT(*) FILTER (WHERE u.status='succeeded')/COUNT(*)) AS value
         FROM usage_events u LEFT JOIN admin_resources p
           ON p.kind='projects' AND p.id=u.project_id
         WHERE ($1::text IS NULL OR u.tenant_id=$1)
           AND ($2::text IS NULL OR u.project_id=$2)
           AND ($3::timestamptz IS NULL OR u.occurred_at >= $3::timestamptz)
           AND ($4::timestamptz IS NULL OR u.occurred_at <= $4::timestamptz)
         GROUP BY u.project_id,p.body->>'name' ORDER BY COUNT(*) DESC",
        query,
    )
    .await?;
    let providers = breakdown_rows(
        store,
        "SELECT jsonb_build_object(
            'id',provider,'name',provider,'requests',COUNT(*),
            'input_tokens',COALESCE(SUM(prompt_tokens),0),
            'output_tokens',COALESCE(SUM(completion_tokens),0),
            'estimated_cost',COALESCE(SUM(estimated_cost),0),
            'success_rate',100.0*COUNT(*) FILTER (WHERE status='succeeded')/COUNT(*)) AS value
         FROM usage_events WHERE ($1::text IS NULL OR tenant_id=$1)
           AND ($2::text IS NULL OR project_id=$2)
           AND ($3::timestamptz IS NULL OR occurred_at >= $3::timestamptz)
           AND ($4::timestamptz IS NULL OR occurred_at <= $4::timestamptz)
         GROUP BY provider ORDER BY COUNT(*) DESC",
        query,
    )
    .await?;
    let models = breakdown_rows(
        store,
        "SELECT jsonb_build_object(
            'id',requested_model,'name',requested_model,'requests',COUNT(*),
            'input_tokens',COALESCE(SUM(prompt_tokens),0),
            'output_tokens',COALESCE(SUM(completion_tokens),0),
            'estimated_cost',COALESCE(SUM(estimated_cost),0),
            'success_rate',100.0*COUNT(*) FILTER (WHERE status='succeeded')/COUNT(*)) AS value
         FROM usage_events WHERE ($1::text IS NULL OR tenant_id=$1)
           AND ($2::text IS NULL OR project_id=$2)
           AND ($3::timestamptz IS NULL OR occurred_at >= $3::timestamptz)
           AND ($4::timestamptz IS NULL OR occurred_at <= $4::timestamptz)
         GROUP BY requested_model ORDER BY COUNT(*) DESC",
        query,
    )
    .await?;
    let upstream_models = breakdown_rows(
        store,
        "SELECT jsonb_build_object(
            'id',upstream_model,'name',upstream_model,'requests',COUNT(*),
            'input_tokens',COALESCE(SUM(prompt_tokens),0),
            'output_tokens',COALESCE(SUM(completion_tokens),0),
            'estimated_cost',COALESCE(SUM(estimated_cost),0),
            'success_rate',100.0*COUNT(*) FILTER (WHERE status='succeeded')/COUNT(*)) AS value
         FROM usage_events WHERE ($1::text IS NULL OR tenant_id=$1)
           AND ($2::text IS NULL OR project_id=$2)
           AND ($3::timestamptz IS NULL OR occurred_at >= $3::timestamptz)
           AND ($4::timestamptz IS NULL OR occurred_at <= $4::timestamptz)
         GROUP BY upstream_model ORDER BY COUNT(*) DESC",
        query,
    )
    .await?;
    let statuses = breakdown_rows(
        store,
        "SELECT jsonb_build_object(
            'id',status,'name',status,'requests',COUNT(*),
            'input_tokens',COALESCE(SUM(prompt_tokens),0),
            'output_tokens',COALESCE(SUM(completion_tokens),0),
            'estimated_cost',COALESCE(SUM(estimated_cost),0)) AS value
         FROM usage_events WHERE ($1::text IS NULL OR tenant_id=$1)
           AND ($2::text IS NULL OR project_id=$2)
           AND ($3::timestamptz IS NULL OR occurred_at >= $3::timestamptz)
           AND ($4::timestamptz IS NULL OR occurred_at <= $4::timestamptz)
         GROUP BY status ORDER BY COUNT(*) DESC",
        query,
    )
    .await?;
    let api_keys = breakdown_rows(
        store,
        "SELECT jsonb_build_object(
            'id',principal_id,
            'name',COALESCE((SELECT COALESCE(v.display_name,v.lookup_prefix)
                FROM virtual_keys v
                WHERE v.id::text=split_part(u.principal_id,':',2)
                  AND v.tenant_id=u.tenant_id),'Session / JWT'),
            'requests',COUNT(*),'input_tokens',COALESCE(SUM(prompt_tokens),0),
            'output_tokens',COALESCE(SUM(completion_tokens),0),
            'estimated_cost',COALESCE(SUM(estimated_cost),0),
            'success_rate',100.0*COUNT(*) FILTER (WHERE status='succeeded')/COUNT(*)) AS value
         FROM usage_events u WHERE ($1::text IS NULL OR tenant_id=$1)
           AND ($2::text IS NULL OR project_id=$2)
           AND ($3::timestamptz IS NULL OR occurred_at >= $3::timestamptz)
           AND ($4::timestamptz IS NULL OR occurred_at <= $4::timestamptz)
         GROUP BY tenant_id,principal_id ORDER BY COUNT(*) DESC",
        query,
    )
    .await?;
    Ok(json!({
        "projects": projects,
        "providers": providers,
        "models": models,
        "upstream_models": upstream_models,
        "api_keys": api_keys,
        "statuses": statuses
    }))
}

async fn breakdown_rows(
    store: &PostgresStore,
    sql: &str,
    query: &ListQuery,
) -> Result<Vec<Value>, AdminError> {
    sqlx::query(sql)
        .bind(&query.tenant_id)
        .bind(&query.project_id)
        .bind(&query.from)
        .bind(&query.to)
        .fetch_all(store.pool())
        .await
        .map_err(|_| AdminError::Unavailable)
        .and_then(values)
}

async fn provider_health(store: &PostgresStore, query: &ListQuery) -> Result<Value, AdminError> {
    sqlx::query_scalar(
        "WITH enabled_providers AS (
            SELECT r.id,COALESCE(r.body->>'name',r.id) AS name
            FROM admin_resources r
            WHERE r.kind='providers' AND r.enabled AND r.retired_at IS NULL
              AND ($1::text IS NULL OR r.tenant_id=$1)
        ),
        scoped_usage AS (
            SELECT u.* FROM usage_events u
            WHERE ($1::text IS NULL OR u.tenant_id=$1)
              AND ($2::text IS NULL OR u.project_id=$2)
              AND ($3::timestamptz IS NULL OR u.occurred_at >= $3::timestamptz)
              AND ($4::timestamptz IS NULL OR u.occurred_at <= $4::timestamptz)
              AND ($5::text IS NULL OR u.provider=$5)
        ),
        filtered_usage AS (
            SELECT * FROM scoped_usage
            WHERE ($6::text IS NULL OR upstream_model=$6)
        ),
        selected_providers AS (
            SELECT p.* FROM enabled_providers p
            WHERE ($5::text IS NULL OR p.id=$5)
              AND ($6::text IS NULL
                   OR EXISTS (SELECT 1 FROM filtered_usage u WHERE u.provider=p.id))
        ),
        provider_metrics AS (
            SELECT p.id,p.name,COALESCE(h.status,'unknown') AS current_status,
                COUNT(u.event_id) AS requests,
                CASE WHEN COUNT(u.event_id)=0 THEN NULL
                     ELSE 100.0*COUNT(u.event_id) FILTER (WHERE u.status='succeeded')
                         /COUNT(u.event_id) END AS success_rate,
                CASE WHEN COUNT(u.event_id)=0 THEN NULL
                     ELSE 100.0*COUNT(u.event_id) FILTER (WHERE u.status<>'succeeded')
                         /COUNT(u.event_id) END AS error_rate,
                percentile_cont(0.5) WITHIN GROUP (ORDER BY u.latency_ms)
                    FILTER (WHERE u.latency_ms IS NOT NULL) AS p50_latency_ms,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY u.latency_ms)
                    FILTER (WHERE u.latency_ms IS NOT NULL) AS p95_latency_ms,
                MAX(u.occurred_at) FILTER (WHERE u.status='succeeded')
                    AS last_successful_request,
                MAX(u.occurred_at) FILTER (WHERE u.status<>'succeeded') AS last_failure
            FROM selected_providers p
            LEFT JOIN provider_health h ON h.provider_id=p.id
            LEFT JOIN filtered_usage u ON u.provider=p.id
            GROUP BY p.id,p.name,h.status
        ),
        series AS (
            SELECT date_trunc('hour',occurred_at) AS time,COUNT(*) AS requests,
                COUNT(*) FILTER (WHERE status<>'succeeded') AS errors,
                100.0*COUNT(*) FILTER (WHERE status='succeeded')/COUNT(*) AS availability,
                percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms)
                    FILTER (WHERE latency_ms IS NOT NULL) AS p50_latency_ms,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms)
                    FILTER (WHERE latency_ms IS NOT NULL) AS p95_latency_ms
            FROM filtered_usage
            GROUP BY date_trunc('hour',occurred_at)
        )
        SELECT jsonb_build_object(
            'summary',jsonb_build_object(
                'healthy_providers',COUNT(*) FILTER (WHERE current_status='healthy'),
                'degraded_providers',COUNT(*) FILTER (WHERE current_status<>'healthy'),
                'average_success_rate',AVG(success_rate),
                'average_p95_latency_ms',AVG(p95_latency_ms)),
            'providers',COALESCE(jsonb_agg(jsonb_build_object(
                'provider_id',id,'name',name,'current_status',current_status,
                'success_rate',success_rate,'error_rate',error_rate,
                'p50_latency_ms',p50_latency_ms,'p95_latency_ms',p95_latency_ms,
                'last_successful_request',last_successful_request,
                'last_failure',last_failure) ORDER BY name)
                FILTER (WHERE id IS NOT NULL),'[]'::jsonb),
            'series',(SELECT COALESCE(jsonb_agg(jsonb_build_object(
                'time',time,'availability',availability,
                'p50_latency_ms',p50_latency_ms,'p95_latency_ms',p95_latency_ms,
                'requests',requests,'errors',errors) ORDER BY time),'[]'::jsonb)
                FROM series),
            'filters',jsonb_build_object(
                'providers',(SELECT COALESCE(jsonb_agg(jsonb_build_object(
                    'id',id,'name',name) ORDER BY name),'[]'::jsonb)
                    FROM enabled_providers),
                'models',(SELECT COALESCE(jsonb_agg(model ORDER BY model),'[]'::jsonb)
                    FROM (SELECT DISTINCT upstream_model AS model
                          FROM scoped_usage WHERE upstream_model<>'') models))
        ) FROM provider_metrics",
    )
    .bind(&query.tenant_id)
    .bind(&query.project_id)
    .bind(&query.from)
    .bind(&query.to)
    .bind(&query.provider_id)
    .bind(&query.model)
    .fetch_one(store.pool())
    .await
    .map_err(|_| AdminError::Unavailable)
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
