use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_auth::{IdentityRepository, Membership, TenantRole, WebAuthError, WebUser};
use sqlx::Row;
use uuid::Uuid;

use crate::PostgresStore;

#[async_trait]
impl IdentityRepository for PostgresStore {
    async fn create_account(
        &self,
        user: &WebUser,
        tenant_id: Uuid,
        tenant_name: &str,
        session_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<Vec<Membership>, WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        sqlx::query(
            "INSERT INTO users (id,email,password_hash,gateway_admin) VALUES ($1,$2,$3,$4)",
        )
        .bind(user.id)
        .bind(&user.email)
        .bind(&user.password_hash)
        .bind(user.gateway_admin)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query("INSERT INTO tenants (id,name,daily_token_limit) VALUES ($1,$2,100000)")
            .bind(tenant_id.to_string())
            .bind(tenant_name)
            .execute(&mut *transaction)
            .await
            .map_err(map_error)?;
        sqlx::query(
            "INSERT INTO tenant_memberships (tenant_id,user_id,role) VALUES ($1,$2,'owner')",
        )
        .bind(tenant_id.to_string())
        .bind(user.id)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        insert_session(&mut transaction, user.id, session_hash, expires_at).await?;
        transaction.commit().await.map_err(map_error)?;
        Ok(vec![Membership {
            tenant_id,
            tenant_name: tenant_name.to_owned(),
            role: TenantRole::Owner,
        }])
    }

    async fn user_by_email(&self, email: &str) -> Result<Option<WebUser>, WebAuthError> {
        sqlx::query("SELECT id,email,password_hash,gateway_admin FROM users WHERE email=$1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_error)?
            .map(user_from_row)
            .transpose()
    }

    async fn create_session(
        &self,
        user_id: Uuid,
        session_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        insert_session(&mut transaction, user_id, session_hash, expires_at).await?;
        transaction.commit().await.map_err(map_error)
    }

    async fn revoke_session(&self, session_hash: &[u8]) -> Result<(), WebAuthError> {
        sqlx::query(
            "UPDATE web_sessions SET revoked_at=COALESCE(revoked_at,now()) WHERE token_hash=$1",
        )
        .bind(session_hash)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(map_error)
    }

    async fn memberships(&self, user_id: Uuid) -> Result<Vec<Membership>, WebAuthError> {
        sqlx::query("SELECT t.id,t.name,m.role FROM tenant_memberships m JOIN tenants t ON t.id=m.tenant_id WHERE m.user_id=$1 ORDER BY t.created_at,t.id")
            .bind(user_id).fetch_all(&self.pool).await.map_err(map_error)?
            .into_iter().map(|row| {
                let id: String = row.try_get("id").map_err(|_| WebAuthError::Unavailable)?;
                Ok(Membership {
                    tenant_id: Uuid::parse_str(&id).map_err(|_| WebAuthError::Unavailable)?,
                    tenant_name: row.try_get("name").map_err(|_| WebAuthError::Unavailable)?,
                    role: parse_role(&row.try_get::<String,_>("role").map_err(|_| WebAuthError::Unavailable)?)?,
                })
            }).collect()
    }

    async fn session_principal(
        &self,
        session_hash: &[u8],
        tenant_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<(WebUser, TenantRole)>, WebAuthError> {
        sqlx::query("SELECT u.id,u.email,u.password_hash,u.gateway_admin,m.role FROM web_sessions s JOIN users u ON u.id=s.user_id JOIN tenant_memberships m ON m.user_id=u.id AND m.tenant_id=$2 WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>$3")
            .bind(session_hash).bind(tenant_id.to_string()).bind(now)
            .fetch_optional(&self.pool).await.map_err(map_error)?
            .map(|row| {
                let role = parse_role(&row.try_get::<String,_>("role").map_err(|_| WebAuthError::Unavailable)?)?;
                Ok((user_from_row(row)?, role))
            }).transpose()
    }

    async fn session_user(
        &self,
        session_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<(WebUser, DateTime<Utc>)>, WebAuthError> {
        sqlx::query("SELECT u.id,u.email,u.password_hash,u.gateway_admin,s.expires_at FROM web_sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>$2")
            .bind(session_hash).bind(now).fetch_optional(&self.pool).await.map_err(map_error)?
            .map(|row| {
                let expires_at = row.try_get("expires_at").map_err(|_| WebAuthError::Unavailable)?;
                Ok((user_from_row(row)?, expires_at))
            }).transpose()
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_invitation(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        email: &str,
        role: TenantRole,
        token_hash: &[u8],
        invited_by: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Result<(), WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        sqlx::query("INSERT INTO tenant_invitations (id,tenant_id,email,role,token_hash,invited_by,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(id).bind(tenant_id.to_string()).bind(email).bind(role.as_str()).bind(token_hash).bind(invited_by).bind(expires_at)
            .execute(&mut *transaction).await.map_err(map_error)?;
        sqlx::query("INSERT INTO auth_outbox (event_id,invitation_id,event_type,payload) VALUES ($1,$2,'tenant.invitation.created',$3)")
            .bind(Uuid::now_v7()).bind(id).bind(serde_json::json!({"invitation_id":id,"tenant_id":tenant_id,"email":email,"role":role,"expires_at":expires_at}))
            .execute(&mut *transaction).await.map_err(map_error)?;
        transaction.commit().await.map_err(map_error)
    }

    async fn accept_invitation(
        &self,
        token_hash: &[u8],
        user_id: Uuid,
        email: &str,
        now: DateTime<Utc>,
    ) -> Result<Membership, WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        let row = sqlx::query("SELECT i.id,i.tenant_id,i.role,t.name FROM tenant_invitations i JOIN tenants t ON t.id=i.tenant_id WHERE i.token_hash=$1 AND i.email=$2 AND i.accepted_at IS NULL AND i.expires_at>$3 FOR UPDATE")
            .bind(token_hash).bind(email).bind(now).fetch_optional(&mut *transaction).await.map_err(map_error)?.ok_or(WebAuthError::Invalid)?;
        let tenant: String = row
            .try_get("tenant_id")
            .map_err(|_| WebAuthError::Unavailable)?;
        let role = parse_role(
            &row.try_get::<String, _>("role")
                .map_err(|_| WebAuthError::Unavailable)?,
        )?;
        sqlx::query("INSERT INTO tenant_memberships (tenant_id,user_id,role) VALUES ($1,$2,$3) ON CONFLICT (tenant_id,user_id) DO NOTHING")
            .bind(&tenant).bind(user_id).bind(role.as_str()).execute(&mut *transaction).await.map_err(map_error)?;
        sqlx::query("UPDATE tenant_invitations SET accepted_at=$2 WHERE id=$1")
            .bind(
                row.try_get::<Uuid, _>("id")
                    .map_err(|_| WebAuthError::Unavailable)?,
            )
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(map_error)?;
        let membership = Membership {
            tenant_id: Uuid::parse_str(&tenant).map_err(|_| WebAuthError::Unavailable)?,
            tenant_name: row.try_get("name").map_err(|_| WebAuthError::Unavailable)?,
            role,
        };
        transaction.commit().await.map_err(map_error)?;
        Ok(membership)
    }
}

async fn insert_session(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    session_hash: &[u8],
    expires_at: DateTime<Utc>,
) -> Result<(), WebAuthError> {
    sqlx::query("INSERT INTO web_sessions (id,user_id,token_hash,expires_at) VALUES ($1,$2,$3,$4)")
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(session_hash)
        .bind(expires_at)
        .execute(&mut **transaction)
        .await
        .map(|_| ())
        .map_err(map_error)
}

fn user_from_row(row: sqlx::postgres::PgRow) -> Result<WebUser, WebAuthError> {
    Ok(WebUser {
        id: row.try_get("id").map_err(|_| WebAuthError::Unavailable)?,
        email: row
            .try_get("email")
            .map_err(|_| WebAuthError::Unavailable)?,
        password_hash: row
            .try_get("password_hash")
            .map_err(|_| WebAuthError::Unavailable)?,
        gateway_admin: row
            .try_get("gateway_admin")
            .map_err(|_| WebAuthError::Unavailable)?,
    })
}

fn parse_role(value: &str) -> Result<TenantRole, WebAuthError> {
    match value {
        "owner" => Ok(TenantRole::Owner),
        "admin" => Ok(TenantRole::Admin),
        "engineer" => Ok(TenantRole::Engineer),
        "viewer" => Ok(TenantRole::Viewer),
        _ => Err(WebAuthError::Unavailable),
    }
}

fn map_error(error: sqlx::Error) -> WebAuthError {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        WebAuthError::Conflict
    } else {
        WebAuthError::Unavailable
    }
}
