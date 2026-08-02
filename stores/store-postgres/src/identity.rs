use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_auth::{
    IdentityRepository, Membership, OnboardingDisplay, OnboardingFacts, Organization,
    OrganizationUpdate, PendingInvitation, TenantRole, VerificationResult, WebAuthError, WebUser,
};
use sqlx::Row;
use uuid::Uuid;

use crate::PostgresStore;

#[async_trait]
impl IdentityRepository for PostgresStore {
    async fn installation_initialized(&self) -> Result<bool, WebAuthError> {
        sqlx::query_scalar::<_, bool>(
            "SELECT initialized_at IS NOT NULL FROM installation_state WHERE singleton=true",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_error)
    }

    async fn bootstrap_account(
        &self,
        user: &WebUser,
        tenant_id: Uuid,
        tenant_name: &str,
        session_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<Vec<Membership>, WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        let initialized: bool = sqlx::query_scalar(
            "SELECT initialized_at IS NOT NULL FROM installation_state
             WHERE singleton=true FOR UPDATE",
        )
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_error)?;
        if initialized {
            return Err(WebAuthError::BootstrapConsumed);
        }
        sqlx::query(
            "INSERT INTO users (id,email,password_hash,gateway_admin) VALUES ($1,$2,$3,true)",
        )
        .bind(user.id)
        .bind(&user.email)
        .bind(&user.password_hash)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query(
            "INSERT INTO tenants (id,name,slug,daily_token_limit) VALUES ($1,$2,$3,100000)",
        )
        .bind(tenant_id.to_string())
        .bind(tenant_name)
        .bind(slug(tenant_name, tenant_id))
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
        sqlx::query(
            "UPDATE installation_state SET initialized_at=now(),initialized_by=$1
             WHERE singleton=true",
        )
        .bind(user.id)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(vec![Membership {
            tenant_id,
            tenant_name: tenant_name.to_owned(),
            role: TenantRole::Owner,
        }])
    }

    async fn create_tenant(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        tenant_name: &str,
    ) -> Result<Membership, WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        sqlx::query(
            "INSERT INTO tenants (id,name,slug,daily_token_limit) VALUES ($1,$2,$3,100000)",
        )
        .bind(tenant_id.to_string())
        .bind(tenant_name)
        .bind(slug(tenant_name, tenant_id))
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query(
            "INSERT INTO tenant_memberships (tenant_id,user_id,role) VALUES ($1,$2,'owner')",
        )
        .bind(tenant_id.to_string())
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(Membership {
            tenant_id,
            tenant_name: tenant_name.to_owned(),
            role: TenantRole::Owner,
        })
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

    async fn create_pending_registration(
        &self,
        email: &str,
        password_hash: &str,
        tenant_name: &str,
        terms_accepted_at: DateTime<Utc>,
        privacy_acknowledged_at: DateTime<Utc>,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), WebAuthError> {
        sqlx::query(
            "INSERT INTO pending_registrations (email,password_hash,tenant_name,terms_accepted_at,privacy_acknowledged_at,token_hash,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (email) DO UPDATE SET password_hash=EXCLUDED.password_hash,tenant_name=EXCLUDED.tenant_name,terms_accepted_at=EXCLUDED.terms_accepted_at,privacy_acknowledged_at=EXCLUDED.privacy_acknowledged_at,token_hash=EXCLUDED.token_hash,expires_at=EXCLUDED.expires_at,created_at=now()",
        )
        .bind(email)
        .bind(password_hash)
        .bind(tenant_name)
        .bind(terms_accepted_at)
        .bind(privacy_acknowledged_at)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(map_error)
    }

    async fn refresh_pending_registration(
        &self,
        email: &str,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<bool, WebAuthError> {
        sqlx::query("UPDATE pending_registrations SET token_hash=$2,expires_at=$3,created_at=now() WHERE email=$1")
            .bind(email).bind(token_hash).bind(expires_at).execute(&self.pool).await
            .map(|result| result.rows_affected() == 1).map_err(map_error)
    }

    async fn verify_pending_registration(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<VerificationResult, WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        let row = sqlx::query("SELECT email,password_hash,tenant_name,terms_accepted_at,privacy_acknowledged_at FROM pending_registrations WHERE token_hash=$1 AND expires_at>$2 FOR UPDATE")
            .bind(token_hash).bind(now).fetch_optional(&mut *transaction).await.map_err(map_error)?.ok_or(WebAuthError::Invalid)?;
        let email: String = row
            .try_get("email")
            .map_err(|_| WebAuthError::Unavailable)?;
        let password_hash: String = row
            .try_get("password_hash")
            .map_err(|_| WebAuthError::Unavailable)?;
        let tenant_name: String = row
            .try_get("tenant_name")
            .map_err(|_| WebAuthError::Unavailable)?;
        let terms_accepted_at: DateTime<Utc> = row
            .try_get("terms_accepted_at")
            .map_err(|_| WebAuthError::Unavailable)?;
        let privacy_acknowledged_at: DateTime<Utc> = row
            .try_get("privacy_acknowledged_at")
            .map_err(|_| WebAuthError::Unavailable)?;
        let user_id = Uuid::now_v7();
        let tenant_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO users (id,email,password_hash,gateway_admin) VALUES ($1,$2,$3,false)",
        )
        .bind(user_id)
        .bind(&email)
        .bind(password_hash)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query(
            "INSERT INTO user_legal_consents (user_id,policy,policy_url,policy_version,accepted_at) VALUES ($1,$2,$3,$4,$5),($1,$6,$7,$8,$9)",
        )
        .bind(user_id)
        .bind("terms_of_service")
        .bind(gateway_auth::TERMS_OF_SERVICE_URL)
        .bind(gateway_auth::TERMS_OF_SERVICE_VERSION)
        .bind(terms_accepted_at)
        .bind("privacy_policy")
        .bind(gateway_auth::PRIVACY_POLICY_URL)
        .bind(gateway_auth::PRIVACY_POLICY_VERSION)
        .bind(privacy_acknowledged_at)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query(
            "INSERT INTO tenants (id,name,slug,daily_token_limit) VALUES ($1,$2,$3,100000)",
        )
        .bind(tenant_id.to_string())
        .bind(&tenant_name)
        .bind(slug(&tenant_name, tenant_id))
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query(
            "INSERT INTO tenant_memberships (tenant_id,user_id,role) VALUES ($1,$2,'owner')",
        )
        .bind(tenant_id.to_string())
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query("DELETE FROM pending_registrations WHERE email=$1")
            .bind(&email)
            .execute(&mut *transaction)
            .await
            .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(VerificationResult { email })
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
        sqlx::query("SELECT t.id,t.name,m.role FROM tenant_memberships m JOIN tenants t ON t.id=m.tenant_id WHERE m.user_id=$1 AND NOT EXISTS (SELECT 1 FROM plan_resource_suspensions s WHERE s.tenant_id=m.tenant_id AND s.resource_kind='members' AND s.resource_id=m.user_id::text AND s.restored_at IS NULL) ORDER BY t.created_at,t.id")
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
        sqlx::query("SELECT u.id,u.email,u.password_hash,u.gateway_admin,m.role FROM web_sessions s JOIN users u ON u.id=s.user_id JOIN tenant_memberships m ON m.user_id=u.id AND m.tenant_id=$2 WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>$3 AND NOT EXISTS (SELECT 1 FROM plan_resource_suspensions p WHERE p.tenant_id=m.tenant_id AND p.resource_kind='members' AND p.resource_id=m.user_id::text AND p.restored_at IS NULL)")
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

    async fn register_invitation(
        &self,
        token_hash: &[u8],
        user: &WebUser,
        session_hash: &[u8],
        session_expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<Membership, WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        let row = sqlx::query(
            "SELECT i.id,i.email,i.tenant_id,i.role,t.name
             FROM tenant_invitations i
             JOIN tenants t ON t.id=i.tenant_id
             WHERE i.token_hash=$1 AND i.accepted_at IS NULL AND i.expires_at>$2
             FOR UPDATE",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_error)?
        .ok_or(WebAuthError::Invalid)?;
        let email: String = row
            .try_get("email")
            .map_err(|_| WebAuthError::Unavailable)?;
        let tenant: String = row
            .try_get("tenant_id")
            .map_err(|_| WebAuthError::Unavailable)?;
        let role = parse_role(
            &row.try_get::<String, _>("role")
                .map_err(|_| WebAuthError::Unavailable)?,
        )?;
        sqlx::query(
            "INSERT INTO users (id,email,password_hash,gateway_admin) VALUES ($1,$2,$3,false)",
        )
        .bind(user.id)
        .bind(&email)
        .bind(&user.password_hash)
        .execute(&mut *transaction)
        .await
        .map_err(map_error)?;
        sqlx::query("INSERT INTO tenant_memberships (tenant_id,user_id,role) VALUES ($1,$2,$3)")
            .bind(&tenant)
            .bind(user.id)
            .bind(role.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(map_error)?;
        sqlx::query("UPDATE tenant_invitations SET accepted_at=$2 WHERE id=$1")
            .bind(
                row.try_get::<Uuid, _>("id")
                    .map_err(|_| WebAuthError::Unavailable)?,
            )
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(map_error)?;
        insert_session(&mut transaction, user.id, session_hash, session_expires_at).await?;
        let membership = Membership {
            tenant_id: Uuid::parse_str(&tenant).map_err(|_| WebAuthError::Unavailable)?,
            tenant_name: row.try_get("name").map_err(|_| WebAuthError::Unavailable)?,
            role,
        };
        transaction.commit().await.map_err(map_error)?;
        Ok(membership)
    }

    async fn organization(&self, tenant_id: Uuid) -> Result<Option<Organization>, WebAuthError> {
        sqlx::query(
            "SELECT id,name,slug,default_environment,region,default_member_role,
                    default_provider_id,version FROM tenants WHERE id=$1",
        )
        .bind(tenant_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .map(organization_from_row)
        .transpose()
    }

    async fn update_organization(
        &self,
        tenant_id: Uuid,
        expected_version: u64,
        input: &OrganizationUpdate,
    ) -> Result<Organization, WebAuthError> {
        if let Some(provider_id) = input.default_provider_id.as_deref() {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM admin_resources
                 WHERE kind='providers' AND id=$1 AND tenant_id=$2 AND retired_at IS NULL)",
            )
            .bind(provider_id)
            .bind(tenant_id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(map_error)?;
            if !valid {
                return Err(WebAuthError::Invalid);
            }
        }
        sqlx::query(
            "UPDATE tenants SET name=$2,slug=$3,default_environment=$4,region=$5,
                    default_member_role=$6,default_provider_id=$7,version=version+1
             WHERE id=$1 AND version=$8
             RETURNING id,name,slug,default_environment,region,default_member_role,
                       default_provider_id,version",
        )
        .bind(tenant_id.to_string())
        .bind(input.name.trim())
        .bind(&input.slug)
        .bind(&input.default_environment)
        .bind(&input.region)
        .bind(input.default_member_role.as_str())
        .bind(&input.default_provider_id)
        .bind(i64::try_from(expected_version).map_err(|_| WebAuthError::Invalid)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .map(organization_from_row)
        .transpose()?
        .ok_or(WebAuthError::Conflict)
    }

    async fn delete_organization(&self, tenant_id: Uuid) -> Result<(), WebAuthError> {
        affected(
            sqlx::query("DELETE FROM tenants WHERE id=$1")
                .bind(tenant_id.to_string())
                .execute(&self.pool)
                .await
                .map_err(map_error)?,
        )
    }

    async fn pending_invitations(
        &self,
        tenant_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Vec<PendingInvitation>, WebAuthError> {
        sqlx::query(
            "SELECT id,email,role,expires_at,created_at FROM tenant_invitations
             WHERE tenant_id=$1 AND accepted_at IS NULL AND expires_at>$2
             ORDER BY created_at DESC",
        )
        .bind(tenant_id.to_string())
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(map_error)?
        .into_iter()
        .map(|row| {
            Ok(PendingInvitation {
                id: row.try_get("id").map_err(|_| WebAuthError::Unavailable)?,
                email: row
                    .try_get("email")
                    .map_err(|_| WebAuthError::Unavailable)?,
                role: parse_role(
                    &row.try_get::<String, _>("role")
                        .map_err(|_| WebAuthError::Unavailable)?,
                )?,
                expires_at: row
                    .try_get("expires_at")
                    .map_err(|_| WebAuthError::Unavailable)?,
                created_at: row
                    .try_get("created_at")
                    .map_err(|_| WebAuthError::Unavailable)?,
            })
        })
        .collect()
    }

    async fn revoke_invitation(
        &self,
        tenant_id: Uuid,
        invitation_id: Uuid,
    ) -> Result<(), WebAuthError> {
        affected(
            sqlx::query(
                "DELETE FROM tenant_invitations
                 WHERE id=$1 AND tenant_id=$2 AND accepted_at IS NULL",
            )
            .bind(invitation_id)
            .bind(tenant_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(map_error)?,
        )
    }

    async fn update_member(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: TenantRole,
    ) -> Result<(), WebAuthError> {
        affected(
            sqlx::query(
                "UPDATE tenant_memberships SET role=$3
                 WHERE tenant_id=$1 AND user_id=$2 AND role<>'owner'",
            )
            .bind(tenant_id.to_string())
            .bind(user_id)
            .bind(role.as_str())
            .execute(&self.pool)
            .await
            .map_err(map_error)?,
        )
    }

    async fn remove_member(&self, tenant_id: Uuid, user_id: Uuid) -> Result<(), WebAuthError> {
        let mut transaction = self.pool.begin().await.map_err(map_error)?;
        let role = sqlx::query_scalar::<_, String>(
            "SELECT role FROM tenant_memberships
             WHERE tenant_id=$1 AND user_id=$2 FOR UPDATE",
        )
        .bind(tenant_id.to_string())
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_error)?
        .ok_or(WebAuthError::NotFound)?;
        if role == "owner" {
            let owners: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM tenant_memberships WHERE tenant_id=$1 AND role='owner'",
            )
            .bind(tenant_id.to_string())
            .fetch_one(&mut *transaction)
            .await
            .map_err(map_error)?;
            if owners <= 1 {
                return Err(WebAuthError::Conflict);
            }
        }
        sqlx::query("DELETE FROM tenant_memberships WHERE tenant_id=$1 AND user_id=$2")
            .bind(tenant_id.to_string())
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)
    }

    async fn onboarding_facts(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        project_id: Option<&str>,
    ) -> Result<OnboardingFacts, WebAuthError> {
        if project_id.is_some_and(|id| id.is_empty() || id.len() > 255) {
            return Err(WebAuthError::Invalid);
        }
        let row = sqlx::query(
            "WITH selected_project AS (
                SELECT r.id
                FROM admin_resources r
                WHERE r.kind='projects' AND r.tenant_id=$1
                  AND r.enabled AND r.retired_at IS NULL
                ORDER BY CASE WHEN r.id=$3 THEN 0 ELSE 1 END,
                         r.created_at DESC,r.id DESC
                LIMIT 1
             ), ready_providers AS (
                SELECT p.id
                FROM admin_resources p
                JOIN provider_health h ON h.provider_id=p.id AND h.status='healthy'
                WHERE p.kind='providers' AND (p.tenant_id=$1 OR p.tenant_id IS NULL)
                  AND p.enabled AND p.retired_at IS NULL
                  AND ((jsonb_typeof(p.body->'available_models')='array'
                        AND jsonb_array_length(p.body->'available_models')>0)
                       OR (p.tenant_id IS NULL
                           AND COALESCE((p.body->>'environment_credential')::boolean,false)))
             )
             SELECT t.onboarding_auto_open,m.onboarding_seen_at,
                    m.onboarding_collapsed_at,
                    (SELECT id FROM selected_project) AS project_id,
                    EXISTS(SELECT 1 FROM selected_project) AS project_ready,
                    EXISTS(SELECT 1 FROM ready_providers) AS provider_ready,
                    EXISTS(
                        SELECT 1 FROM admin_resources r
                        JOIN selected_project s
                          ON r.body->>'project_id'=s.id OR r.body->>'project_id' IS NULL
                        JOIN ready_providers p ON p.id=r.body->>'provider'
                        JOIN admin_resources provider
                          ON provider.kind='providers' AND provider.id=p.id
                        WHERE r.kind='model_routes' AND (r.tenant_id=$1 OR r.tenant_id IS NULL)
                          AND r.enabled AND r.retired_at IS NULL
                          AND (provider.body->'available_models' ? (r.body->>'upstream_model')
                               OR (provider.tenant_id IS NULL
                                   AND COALESCE((provider.body->>'environment_credential')::boolean,false)))
                    ) AS route_ready,
                    EXISTS(
                        SELECT 1 FROM virtual_keys k
                        JOIN selected_project s ON k.project_id=s.id
                        WHERE k.tenant_id=$1 AND k.revoked_at IS NULL
                          AND (k.expires_at IS NULL OR k.expires_at>now())
                          AND NOT EXISTS(
                            SELECT 1 FROM plan_resource_suspensions suspension
                            WHERE suspension.tenant_id=k.tenant_id
                              AND suspension.resource_kind='active_api_keys'
                              AND suspension.resource_id=k.id::text
                              AND suspension.restored_at IS NULL)
                    ) AS api_key_ready,
                    EXISTS(
                        SELECT 1 FROM usage_events u
                        JOIN selected_project s ON u.project_id=s.id
                        WHERE u.tenant_id=$1 AND u.status='succeeded'
                    ) AS first_request_ready
             FROM tenants t
             JOIN tenant_memberships m ON m.tenant_id=t.id AND m.user_id=$2
             WHERE t.id=$1",
        )
        .bind(tenant_id.to_string())
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_error)?
        .ok_or(WebAuthError::Forbidden)?;
        Ok(OnboardingFacts {
            auto_open: row
                .try_get("onboarding_auto_open")
                .map_err(|_| WebAuthError::Unavailable)?,
            seen_at: row
                .try_get("onboarding_seen_at")
                .map_err(|_| WebAuthError::Unavailable)?,
            collapsed_at: row
                .try_get("onboarding_collapsed_at")
                .map_err(|_| WebAuthError::Unavailable)?,
            project_id: row
                .try_get("project_id")
                .map_err(|_| WebAuthError::Unavailable)?,
            project_ready: row
                .try_get("project_ready")
                .map_err(|_| WebAuthError::Unavailable)?,
            provider_ready: row
                .try_get("provider_ready")
                .map_err(|_| WebAuthError::Unavailable)?,
            route_ready: row
                .try_get("route_ready")
                .map_err(|_| WebAuthError::Unavailable)?,
            api_key_ready: row
                .try_get("api_key_ready")
                .map_err(|_| WebAuthError::Unavailable)?,
            first_request_ready: row
                .try_get("first_request_ready")
                .map_err(|_| WebAuthError::Unavailable)?,
        })
    }

    async fn update_onboarding_display(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        display: OnboardingDisplay,
    ) -> Result<(), WebAuthError> {
        let collapsed = display == OnboardingDisplay::Collapsed;
        affected(
            sqlx::query(
                "UPDATE tenant_memberships
                 SET onboarding_seen_at=COALESCE(onboarding_seen_at,now()),
                     onboarding_collapsed_at=CASE WHEN $3 THEN now() ELSE NULL END
                 WHERE tenant_id=$1 AND user_id=$2",
            )
            .bind(tenant_id.to_string())
            .bind(user_id)
            .bind(collapsed)
            .execute(&self.pool)
            .await
            .map_err(map_error)?,
        )
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

fn organization_from_row(row: sqlx::postgres::PgRow) -> Result<Organization, WebAuthError> {
    let id: String = row.try_get("id").map_err(|_| WebAuthError::Unavailable)?;
    let version: i64 = row
        .try_get("version")
        .map_err(|_| WebAuthError::Unavailable)?;
    Ok(Organization {
        id: Uuid::parse_str(&id).map_err(|_| WebAuthError::Unavailable)?,
        name: row.try_get("name").map_err(|_| WebAuthError::Unavailable)?,
        slug: row.try_get("slug").map_err(|_| WebAuthError::Unavailable)?,
        default_environment: row
            .try_get("default_environment")
            .map_err(|_| WebAuthError::Unavailable)?,
        region: row
            .try_get("region")
            .map_err(|_| WebAuthError::Unavailable)?,
        default_member_role: parse_role(
            &row.try_get::<String, _>("default_member_role")
                .map_err(|_| WebAuthError::Unavailable)?,
        )?,
        default_provider_id: row
            .try_get("default_provider_id")
            .map_err(|_| WebAuthError::Unavailable)?,
        version: u64::try_from(version).map_err(|_| WebAuthError::Unavailable)?,
    })
}

fn affected(result: sqlx::postgres::PgQueryResult) -> Result<(), WebAuthError> {
    (result.rows_affected() == 1)
        .then_some(())
        .ok_or(WebAuthError::NotFound)
}

fn slug(name: &str, tenant_id: Uuid) -> String {
    let value = name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim_matches('-').replace("--", "-");
    if value.len() >= 2 {
        format!(
            "{}-{}",
            value.chars().take(48).collect::<String>(),
            &tenant_id.to_string()[..8]
        )
    } else {
        tenant_id.to_string()
    }
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
