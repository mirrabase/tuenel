use std::{fmt, sync::Arc, time::Duration};

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use gateway_types::{AuthenticationMethod, Principal};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const SESSION_PREFIX: &str = "ws_";
const INVITATION_PREFIX: &str = "wi_";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantRole {
    Owner,
    Admin,
    Engineer,
    Viewer,
}

impl TenantRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Engineer => "engineer",
            Self::Viewer => "viewer",
        }
    }
}

#[derive(Clone, Debug)]
pub struct WebUser {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub gateway_admin: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Membership {
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub role: TenantRole,
}

#[derive(Clone, Debug)]
pub struct Signup {
    pub email: String,
    pub password: String,
    pub tenant_name: String,
}

#[derive(Clone)]
pub struct LoginResult {
    pub user_id: Uuid,
    pub credential: SessionCredential,
    pub expires_at: DateTime<Utc>,
    pub memberships: Vec<Membership>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionInfo {
    pub user_id: Uuid,
    pub email: String,
    pub gateway_admin: bool,
    pub expires_at: DateTime<Utc>,
    pub memberships: Vec<Membership>,
}

#[derive(Clone, Debug)]
pub struct InvitationResult {
    pub id: Uuid,
    pub token: SessionCredential,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub default_environment: String,
    pub region: String,
    pub default_member_role: TenantRole,
    pub default_provider_id: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OrganizationUpdate {
    pub name: String,
    pub slug: String,
    pub default_environment: String,
    pub region: String,
    pub default_member_role: TenantRole,
    pub default_provider_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PendingInvitation {
    pub id: Uuid,
    pub email: String,
    pub role: TenantRole,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct SessionCredential(String);

impl SessionCredential {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionCredential([REDACTED])")
    }
}

#[async_trait]
pub trait IdentityRepository: Send + Sync {
    async fn create_tenant(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        tenant_name: &str,
    ) -> Result<Membership, WebAuthError>;
    async fn create_account(
        &self,
        user: &WebUser,
        tenant_id: Uuid,
        tenant_name: &str,
        session_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<Vec<Membership>, WebAuthError>;
    async fn user_by_email(&self, email: &str) -> Result<Option<WebUser>, WebAuthError>;
    async fn create_session(
        &self,
        user_id: Uuid,
        session_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), WebAuthError>;
    async fn revoke_session(&self, session_hash: &[u8]) -> Result<(), WebAuthError>;
    async fn memberships(&self, user_id: Uuid) -> Result<Vec<Membership>, WebAuthError>;
    async fn session_principal(
        &self,
        session_hash: &[u8],
        tenant_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Option<(WebUser, TenantRole)>, WebAuthError>;
    async fn session_user(
        &self,
        session_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<(WebUser, DateTime<Utc>)>, WebAuthError>;
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
    ) -> Result<(), WebAuthError>;
    async fn accept_invitation(
        &self,
        token_hash: &[u8],
        user_id: Uuid,
        email: &str,
        now: DateTime<Utc>,
    ) -> Result<Membership, WebAuthError>;
    async fn organization(&self, tenant_id: Uuid) -> Result<Option<Organization>, WebAuthError>;
    async fn update_organization(
        &self,
        tenant_id: Uuid,
        expected_version: u64,
        input: &OrganizationUpdate,
    ) -> Result<Organization, WebAuthError>;
    async fn delete_organization(&self, tenant_id: Uuid) -> Result<(), WebAuthError>;
    async fn pending_invitations(
        &self,
        tenant_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Vec<PendingInvitation>, WebAuthError>;
    async fn revoke_invitation(
        &self,
        tenant_id: Uuid,
        invitation_id: Uuid,
    ) -> Result<(), WebAuthError>;
    async fn update_member(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: TenantRole,
    ) -> Result<(), WebAuthError>;
    async fn remove_member(&self, tenant_id: Uuid, user_id: Uuid) -> Result<(), WebAuthError>;
}

#[derive(Clone)]
pub struct WebAuthService {
    repository: Arc<dyn IdentityRepository>,
    session_ttl: Duration,
}

impl WebAuthService {
    pub fn new(repository: Arc<dyn IdentityRepository>, session_ttl: Duration) -> Self {
        Self {
            repository,
            session_ttl,
        }
    }

    pub async fn signup(&self, input: Signup) -> Result<LoginResult, WebAuthError> {
        let email = normalize_email(&input.email)?;
        validate_password(&input.password)?;
        let tenant_name = input.tenant_name.trim();
        if tenant_name.is_empty() || tenant_name.len() > 100 {
            return Err(WebAuthError::Invalid);
        }
        let password_hash = hash_password(&input.password)?;
        let (credential, session_hash) = new_session();
        let expires_at = expires_at(self.session_ttl);
        let user = WebUser {
            id: Uuid::now_v7(),
            email,
            password_hash,
            gateway_admin: false,
        };
        let memberships = self
            .repository
            .create_account(
                &user,
                Uuid::now_v7(),
                tenant_name,
                &session_hash,
                expires_at,
            )
            .await?;
        Ok(LoginResult {
            user_id: user.id,
            credential,
            expires_at,
            memberships,
        })
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResult, WebAuthError> {
        let email = normalize_email(email)?;
        let user = self
            .repository
            .user_by_email(&email)
            .await?
            .ok_or(WebAuthError::InvalidCredentials)?;
        let hash =
            PasswordHash::new(&user.password_hash).map_err(|_| WebAuthError::InvalidCredentials)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .map_err(|_| WebAuthError::InvalidCredentials)?;
        let (credential, session_hash) = new_session();
        let expires_at = expires_at(self.session_ttl);
        self.repository
            .create_session(user.id, &session_hash, expires_at)
            .await?;
        Ok(LoginResult {
            user_id: user.id,
            credential,
            expires_at,
            memberships: self.repository.memberships(user.id).await?,
        })
    }

    pub async fn logout(&self, credential: &str) -> Result<(), WebAuthError> {
        let (secret, _) = parse_credential(credential, false)?;
        self.repository.revoke_session(&token_hash(secret)).await
    }

    pub async fn session(&self, credential: &str) -> Result<SessionInfo, WebAuthError> {
        let (secret, _) = parse_credential(credential, false)?;
        let (user, expires_at) = self
            .repository
            .session_user(&token_hash(secret), Utc::now())
            .await?
            .ok_or(WebAuthError::InvalidCredentials)?;
        Ok(SessionInfo {
            user_id: user.id,
            email: user.email,
            gateway_admin: user.gateway_admin,
            expires_at,
            memberships: self.repository.memberships(user.id).await?,
        })
    }

    pub async fn authenticate(&self, credential: &str) -> Result<Principal, WebAuthError> {
        let (secret, tenant_id) = parse_credential(credential, true)?;
        let tenant_id = tenant_id.ok_or(WebAuthError::InvalidCredentials)?;
        let (user, role) = self
            .repository
            .session_principal(&token_hash(secret), tenant_id, Utc::now())
            .await?
            .ok_or(WebAuthError::InvalidCredentials)?;
        let mut roles = vec![role.as_str().to_owned()];
        if user.gateway_admin {
            roles.push("gateway_admin".to_owned());
        }
        Ok(Principal {
            principal_id: format!("user:{}", user.id),
            tenant_id: tenant_id.to_string(),
            project_id: None,
            user_id: Some(user.id.to_string()),
            roles,
            scopes: Vec::new(),
            authentication_method: AuthenticationMethod::WebSession,
            virtual_key_id: None,
        })
    }

    pub async fn create_tenant(
        &self,
        credential: &str,
        tenant_name: &str,
    ) -> Result<Membership, WebAuthError> {
        let session = self.session(credential).await?;
        let tenant_name = tenant_name.trim();
        if tenant_name.is_empty() || tenant_name.len() > 100 {
            return Err(WebAuthError::Invalid);
        }
        self.repository
            .create_tenant(session.user_id, Uuid::now_v7(), tenant_name)
            .await
    }

    pub async fn invite(
        &self,
        credential: &str,
        email: &str,
        role: TenantRole,
    ) -> Result<InvitationResult, WebAuthError> {
        if role == TenantRole::Owner {
            return Err(WebAuthError::Forbidden);
        }
        let principal = self.authenticate(credential).await?;
        if !principal
            .roles
            .iter()
            .any(|role| matches!(role.as_str(), "owner" | "admin"))
        {
            return Err(WebAuthError::Forbidden);
        }
        let tenant_id = Uuid::parse_str(&principal.tenant_id).map_err(|_| WebAuthError::Invalid)?;
        let invited_by = principal
            .user_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or(WebAuthError::Forbidden)?;
        let email = normalize_email(email)?;
        let (token, token_hash) = new_token(INVITATION_PREFIX);
        let result = InvitationResult {
            id: Uuid::now_v7(),
            token,
            expires_at: Utc::now() + chrono::Duration::days(7),
        };
        self.repository
            .create_invitation(
                result.id,
                tenant_id,
                &email,
                role,
                &token_hash,
                invited_by,
                result.expires_at,
            )
            .await?;
        Ok(result)
    }

    pub async fn accept_invitation(
        &self,
        credential: &str,
        token: &str,
    ) -> Result<Membership, WebAuthError> {
        let session = self.session(credential).await?;
        let secret = token
            .strip_prefix(INVITATION_PREFIX)
            .filter(|value| value.len() >= 32)
            .ok_or(WebAuthError::Invalid)?;
        self.repository
            .accept_invitation(
                &token_hash(secret),
                session.user_id,
                &session.email,
                Utc::now(),
            )
            .await
    }

    pub async fn organization(&self, credential: &str) -> Result<Organization, WebAuthError> {
        let principal = self.authenticate(credential).await?;
        self.repository
            .organization(parse_tenant(&principal.tenant_id)?)
            .await?
            .ok_or(WebAuthError::NotFound)
    }

    pub async fn update_organization(
        &self,
        credential: &str,
        expected_version: u64,
        input: OrganizationUpdate,
    ) -> Result<Organization, WebAuthError> {
        let principal = self.authenticate(credential).await?;
        require_manager(&principal.roles)?;
        validate_organization(&input)?;
        self.repository
            .update_organization(
                parse_tenant(&principal.tenant_id)?,
                expected_version,
                &input,
            )
            .await
    }

    pub async fn delete_organization(
        &self,
        credential: &str,
        confirmation: &str,
    ) -> Result<(), WebAuthError> {
        let principal = self.authenticate(credential).await?;
        if !principal.roles.iter().any(|role| role == "owner") {
            return Err(WebAuthError::Forbidden);
        }
        let tenant_id = parse_tenant(&principal.tenant_id)?;
        let organization = self
            .repository
            .organization(tenant_id)
            .await?
            .ok_or(WebAuthError::NotFound)?;
        if confirmation != organization.slug {
            return Err(WebAuthError::Invalid);
        }
        self.repository.delete_organization(tenant_id).await
    }

    pub async fn pending_invitations(
        &self,
        credential: &str,
    ) -> Result<Vec<PendingInvitation>, WebAuthError> {
        let principal = self.authenticate(credential).await?;
        require_manager(&principal.roles)?;
        self.repository
            .pending_invitations(parse_tenant(&principal.tenant_id)?, Utc::now())
            .await
    }

    pub async fn revoke_invitation(
        &self,
        credential: &str,
        invitation_id: Uuid,
    ) -> Result<(), WebAuthError> {
        let principal = self.authenticate(credential).await?;
        require_manager(&principal.roles)?;
        self.repository
            .revoke_invitation(parse_tenant(&principal.tenant_id)?, invitation_id)
            .await
    }

    pub async fn update_member(
        &self,
        credential: &str,
        user_id: Uuid,
        role: TenantRole,
    ) -> Result<(), WebAuthError> {
        let principal = self.authenticate(credential).await?;
        require_manager(&principal.roles)?;
        if role == TenantRole::Owner {
            return Err(WebAuthError::Forbidden);
        }
        self.repository
            .update_member(parse_tenant(&principal.tenant_id)?, user_id, role)
            .await
    }

    pub async fn remove_member(&self, credential: &str, user_id: Uuid) -> Result<(), WebAuthError> {
        let principal = self.authenticate(credential).await?;
        let actor = principal
            .user_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(WebAuthError::Forbidden)?;
        if actor != user_id {
            require_manager(&principal.roles)?;
        }
        self.repository
            .remove_member(parse_tenant(&principal.tenant_id)?, user_id)
            .await
    }
}

fn require_manager(roles: &[String]) -> Result<(), WebAuthError> {
    roles
        .iter()
        .any(|role| matches!(role.as_str(), "owner" | "admin"))
        .then_some(())
        .ok_or(WebAuthError::Forbidden)
}

fn parse_tenant(value: &str) -> Result<Uuid, WebAuthError> {
    Uuid::parse_str(value).map_err(|_| WebAuthError::Invalid)
}

fn validate_organization(input: &OrganizationUpdate) -> Result<(), WebAuthError> {
    let valid_slug = (2..=63).contains(&input.slug.len())
        && input
            .slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !input.slug.starts_with('-')
        && !input.slug.ends_with('-')
        && !input.slug.contains("--");
    if !(2..=100).contains(&input.name.trim().len())
        || !valid_slug
        || !matches!(
            input.default_environment.as_str(),
            "production" | "staging" | "development"
        )
        || !matches!(input.region.as_str(), "global" | "us" | "eu" | "apac")
        || !matches!(
            input.default_member_role,
            TenantRole::Engineer | TenantRole::Viewer
        )
    {
        return Err(WebAuthError::Invalid);
    }
    Ok(())
}

fn normalize_email(value: &str) -> Result<String, WebAuthError> {
    let value = value.trim().to_ascii_lowercase();
    let (local, domain) = value.split_once('@').ok_or(WebAuthError::Invalid)?;
    if value.len() > 254 || local.is_empty() || domain.is_empty() || domain.contains('@') {
        return Err(WebAuthError::Invalid);
    }
    Ok(value)
}

fn validate_password(value: &str) -> Result<(), WebAuthError> {
    if (12..=128).contains(&value.len()) {
        Ok(())
    } else {
        Err(WebAuthError::Invalid)
    }
}

fn hash_password(value: &str) -> Result<String, WebAuthError> {
    Argon2::default()
        .hash_password(value.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|hash| hash.to_string())
        .map_err(|_| WebAuthError::Hashing)
}

fn new_session() -> (SessionCredential, Vec<u8>) {
    new_token(SESSION_PREFIX)
}

fn new_token(prefix: &str) -> (SessionCredential, Vec<u8>) {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let secret = URL_SAFE_NO_PAD.encode(bytes);
    (
        SessionCredential(format!("{prefix}{secret}")),
        token_hash(&secret),
    )
}

fn token_hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

fn parse_credential(
    value: &str,
    tenant_required: bool,
) -> Result<(&str, Option<Uuid>), WebAuthError> {
    let value = value
        .strip_prefix(SESSION_PREFIX)
        .ok_or(WebAuthError::InvalidCredentials)?;
    let (secret, tenant) = value
        .split_once('.')
        .map_or((value, None), |(secret, tenant)| {
            (secret, Uuid::parse_str(tenant).ok())
        });
    if secret.len() < 32 || (tenant_required && tenant.is_none()) {
        return Err(WebAuthError::InvalidCredentials);
    }
    Ok((secret, tenant))
}

fn expires_at(ttl: Duration) -> DateTime<Utc> {
    Utc::now() + chrono::Duration::from_std(ttl).expect("session TTL fits chrono")
}

#[derive(Clone, Debug, Error)]
pub enum WebAuthError {
    #[error("invalid authentication input")]
    Invalid,
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("identity already exists")]
    Conflict,
    #[error("operation is not permitted")]
    Forbidden,
    #[error("identity record not found")]
    NotFound,
    #[error("credential hashing failed")]
    Hashing,
    #[error("identity service unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{new_session, normalize_email, parse_credential, token_hash};

    #[test]
    fn validates_boundary_values() {
        assert_eq!(
            normalize_email(" User@Example.COM ").unwrap(),
            "user@example.com"
        );
        assert!(normalize_email("not-an-email").is_err());
        assert!(parse_credential("ws_short.00000000-0000-0000-0000-000000000000", true).is_err());
    }

    #[test]
    fn session_secret_is_redacted_and_tenant_bound_per_request() {
        let (credential, hash) = new_session();
        assert_eq!(format!("{credential:?}"), "SessionCredential([REDACTED])");
        assert!(parse_credential(credential.expose(), true).is_err());
        let tenant_id = Uuid::now_v7();
        let presented = format!("{}.{tenant_id}", credential.expose());
        let (secret, parsed_tenant) = parse_credential(&presented, true).unwrap();
        assert_eq!(parsed_tenant, Some(tenant_id));
        assert_eq!(token_hash(secret), hash);
    }
}
