use std::{fmt, sync::Arc, time::Duration};

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use gateway_types::{AuthenticationMethod, Principal};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const SESSION_PREFIX: &str = "ws_";
const INVITATION_PREFIX: &str = "wi_";
const VERIFICATION_PREFIX: &str = "wv_";
const BOOTSTRAP_PREFIX: &str = "tb_";
const VERIFICATION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

pub const TERMS_OF_SERVICE_URL: &str = "https://mirrabase.com/terms";
pub const PRIVACY_POLICY_URL: &str = "https://mirrabase.com/privacy";
pub const TERMS_OF_SERVICE_VERSION: &str = "terms-v1";
pub const PRIVACY_POLICY_VERSION: &str = "privacy-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationMode {
    Public,
    InviteOnly,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvitationDelivery {
    Manual,
    Email,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthCapabilities {
    pub deployment_mode: String,
    pub registration_mode: RegistrationMode,
    pub bootstrap_required: bool,
    pub email_verification_required: bool,
}

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingDisplay {
    Expanded,
    Collapsed,
}

#[derive(Clone, Debug)]
pub struct OnboardingFacts {
    pub auto_open: bool,
    pub seen_at: Option<DateTime<Utc>>,
    pub collapsed_at: Option<DateTime<Utc>>,
    pub project_id: Option<String>,
    pub project_ready: bool,
    pub provider_ready: bool,
    pub route_ready: bool,
    pub api_key_ready: bool,
    pub first_request_ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStepStatus {
    Complete,
    Current,
    Pending,
    Blocked,
    NeedsAdmin,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingStep {
    pub id: &'static str,
    pub status: OnboardingStepStatus,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingProgress {
    pub version: u8,
    pub auto_open: bool,
    pub seen: bool,
    pub display: OnboardingDisplay,
    pub can_configure: bool,
    pub can_test: bool,
    pub project_id: Option<String>,
    pub completed_steps: usize,
    pub total_steps: usize,
    pub complete: bool,
    pub steps: Vec<OnboardingStep>,
}

#[derive(Clone, Debug)]
pub struct Signup {
    pub email: String,
    pub password: String,
    pub tenant_name: String,
    pub terms_accepted: bool,
    pub privacy_acknowledged: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SignupResult {
    pub email: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerificationResult {
    pub email: String,
}

#[derive(Clone, Debug)]
pub struct Bootstrap {
    pub token: String,
    pub email: String,
    pub password: String,
    pub tenant_name: String,
}

#[async_trait]
pub trait VerificationEmailSender: Send + Sync {
    async fn send_verification(
        &self,
        event_id: Uuid,
        email: &str,
        token: &str,
    ) -> Result<(), WebAuthError>;
    async fn send_invitation(
        &self,
        event_id: Uuid,
        email: &str,
        token: &str,
    ) -> Result<(), WebAuthError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthEmailKind {
    Verification,
    Invitation,
}

impl AuthEmailKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verification => "verification",
            Self::Invitation => "invitation",
        }
    }
}

#[async_trait]
pub trait AuthEmailQueue: Send + Sync {
    async fn enqueue(
        &self,
        kind: AuthEmailKind,
        email: &str,
        token: &str,
    ) -> Result<(), WebAuthError>;
}

#[async_trait]
pub trait AuthAttemptLimiter: Send + Sync {
    async fn check(
        &self,
        action: &str,
        subject: &str,
        maximum: u64,
        window: Duration,
    ) -> Result<(), WebAuthError>;
}

#[derive(Clone)]
pub struct LoginResult {
    pub user_id: Uuid,
    pub credential: SessionCredential,
    pub expires_at: DateTime<Utc>,
    pub memberships: Vec<Membership>,
}

/// Rust-only session minting boundary for identity adapters that have already
/// validated an external browser login. This trait is intentionally not wired
/// to an unauthenticated HTTP endpoint.
#[async_trait]
pub trait TrustedSessionIssuer: Send + Sync {
    async fn issue_trusted_session(&self, user_id: Uuid) -> Result<LoginResult, WebAuthError>;
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
    pub token: Option<SessionCredential>,
    pub expires_at: DateTime<Utc>,
    pub delivery: InvitationDelivery,
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
    async fn installation_initialized(&self) -> Result<bool, WebAuthError>;
    async fn bootstrap_account(
        &self,
        user: &WebUser,
        tenant_id: Uuid,
        tenant_name: &str,
        session_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<Vec<Membership>, WebAuthError>;
    async fn create_tenant(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        tenant_name: &str,
    ) -> Result<Membership, WebAuthError>;
    #[allow(clippy::too_many_arguments)]
    async fn create_pending_registration(
        &self,
        email: &str,
        password_hash: &str,
        tenant_name: &str,
        terms_accepted_at: DateTime<Utc>,
        privacy_acknowledged_at: DateTime<Utc>,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), WebAuthError>;
    async fn refresh_pending_registration(
        &self,
        email: &str,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<bool, WebAuthError>;
    async fn verify_pending_registration(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<VerificationResult, WebAuthError>;
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
    async fn register_invitation(
        &self,
        token_hash: &[u8],
        user: &WebUser,
        session_hash: &[u8],
        session_expires_at: DateTime<Utc>,
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
    async fn onboarding_facts(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        project_id: Option<&str>,
    ) -> Result<OnboardingFacts, WebAuthError>;
    async fn update_onboarding_display(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        display: OnboardingDisplay,
    ) -> Result<(), WebAuthError>;
}

#[derive(Clone)]
pub struct WebAuthService {
    repository: Arc<dyn IdentityRepository>,
    session_ttl: Duration,
    email_queue: Option<Arc<dyn AuthEmailQueue>>,
    registration_mode: RegistrationMode,
    deployment_mode: String,
    bootstrap_hash: Option<Vec<u8>>,
    invitation_delivery: InvitationDelivery,
    limiter: Option<Arc<dyn AuthAttemptLimiter>>,
    limiter_key: Option<Vec<u8>>,
}

impl WebAuthService {
    pub fn new(repository: Arc<dyn IdentityRepository>, session_ttl: Duration) -> Self {
        Self {
            repository,
            session_ttl,
            email_queue: None,
            registration_mode: RegistrationMode::InviteOnly,
            deployment_mode: "standalone".into(),
            bootstrap_hash: None,
            invitation_delivery: InvitationDelivery::Manual,
            limiter: None,
            limiter_key: None,
        }
    }

    pub fn with_email_queue(mut self, queue: Arc<dyn AuthEmailQueue>) -> Self {
        self.email_queue = Some(queue);
        self
    }

    pub fn with_attempt_limiter(
        mut self,
        limiter: Arc<dyn AuthAttemptLimiter>,
        key: Vec<u8>,
    ) -> Self {
        self.limiter = Some(limiter);
        self.limiter_key = Some(key);
        self
    }

    async fn limit(&self, action: &str, subject: &str, maximum: u64) -> Result<(), WebAuthError> {
        let (Some(limiter), Some(key)) = (&self.limiter, &self.limiter_key) else {
            return Ok(());
        };
        let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|_| WebAuthError::Unavailable)?;
        mac.update(subject.trim().to_ascii_lowercase().as_bytes());
        let digest = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        limiter
            .check(action, &digest, maximum, Duration::from_secs(15 * 60))
            .await
    }

    pub fn with_registration(
        mut self,
        deployment_mode: impl Into<String>,
        registration_mode: RegistrationMode,
        bootstrap_hash: Option<Vec<u8>>,
        invitation_delivery: InvitationDelivery,
    ) -> Self {
        self.deployment_mode = deployment_mode.into();
        self.registration_mode = registration_mode;
        self.bootstrap_hash = bootstrap_hash;
        self.invitation_delivery = invitation_delivery;
        self
    }

    pub async fn capabilities(&self) -> Result<AuthCapabilities, WebAuthError> {
        Ok(AuthCapabilities {
            deployment_mode: self.deployment_mode.clone(),
            registration_mode: self.registration_mode,
            bootstrap_required: !self.repository.installation_initialized().await?,
            email_verification_required: self.registration_mode == RegistrationMode::Public,
        })
    }

    pub async fn bootstrap(&self, input: Bootstrap) -> Result<LoginResult, WebAuthError> {
        self.limit("bootstrap", &input.token, 10).await?;
        if self.repository.installation_initialized().await? {
            return Err(WebAuthError::BootstrapConsumed);
        }
        let expected = self
            .bootstrap_hash
            .as_deref()
            .ok_or(WebAuthError::Unavailable)?;
        if !valid_bootstrap_token(expected, &input.token) {
            return Err(WebAuthError::InvalidBootstrapToken);
        }
        let email = normalize_email(&input.email)?;
        validate_password(&input.password)?;
        let tenant_name = validate_tenant_name(&input.tenant_name)?;
        let user = WebUser {
            id: Uuid::now_v7(),
            email,
            password_hash: hash_password(&input.password)?,
            gateway_admin: true,
        };
        let (credential, session_hash) = new_session();
        let expires_at = expires_at(self.session_ttl);
        let memberships = self
            .repository
            .bootstrap_account(
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

    pub async fn signup(&self, input: Signup) -> Result<SignupResult, WebAuthError> {
        if self.registration_mode != RegistrationMode::Public {
            return Err(WebAuthError::RegistrationClosed);
        }
        validate_signup_consent(&input)?;
        self.limit("signup", &input.email, 5).await?;
        let email = normalize_email(&input.email)?;
        validate_password(&input.password)?;
        if self.repository.user_by_email(&email).await?.is_some() {
            return Ok(SignupResult { email });
        }
        let tenant_name = validate_tenant_name(&input.tenant_name)?;
        let password_hash = hash_password(&input.password)?;
        let (token, token_hash) = new_token(VERIFICATION_PREFIX);
        let consented_at = Utc::now();
        self.repository
            .create_pending_registration(
                &email,
                &password_hash,
                tenant_name,
                consented_at,
                consented_at,
                &token_hash,
                Utc::now() + chrono::Duration::from_std(VERIFICATION_TTL).unwrap(),
            )
            .await?;
        self.email_queue
            .as_ref()
            .ok_or(WebAuthError::Unavailable)?
            .enqueue(AuthEmailKind::Verification, &email, token.expose())
            .await?;
        Ok(SignupResult { email })
    }

    pub async fn verify(&self, token: &str) -> Result<VerificationResult, WebAuthError> {
        let secret = token
            .strip_prefix(VERIFICATION_PREFIX)
            .filter(|value| value.len() >= 32)
            .ok_or(WebAuthError::Invalid)?;
        self.repository
            .verify_pending_registration(&token_hash(secret), Utc::now())
            .await
    }

    pub async fn resend_verification(&self, email: &str) -> Result<(), WebAuthError> {
        self.limit("verification_resend", email, 5).await?;
        let email = normalize_email(email)?;
        let (token, token_hash) = new_token(VERIFICATION_PREFIX);
        if self
            .repository
            .refresh_pending_registration(
                &email,
                &token_hash,
                Utc::now() + chrono::Duration::from_std(VERIFICATION_TTL).unwrap(),
                Utc::now(),
            )
            .await?
        {
            self.email_queue
                .as_ref()
                .ok_or(WebAuthError::Unavailable)?
                .enqueue(AuthEmailKind::Verification, &email, token.expose())
                .await?;
        }
        Ok(())
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResult, WebAuthError> {
        self.limit("login", email, 10).await?;
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

    pub async fn onboarding(
        &self,
        credential: &str,
        project_id: Option<&str>,
    ) -> Result<OnboardingProgress, WebAuthError> {
        let principal = self.authenticate(credential).await?;
        let tenant_id = parse_tenant(&principal.tenant_id)?;
        let user_id = principal
            .user_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or(WebAuthError::Forbidden)?;
        let facts = self
            .repository
            .onboarding_facts(tenant_id, user_id, project_id)
            .await?;
        let can_configure = principal
            .roles
            .iter()
            .any(|role| matches!(role.as_str(), "owner" | "admin" | "gateway_admin"));
        let can_test = principal.roles.iter().any(|role| {
            matches!(
                role.as_str(),
                "owner" | "admin" | "engineer" | "gateway_admin"
            )
        });
        Ok(onboarding_progress(facts, can_configure, can_test))
    }

    pub async fn update_onboarding_display(
        &self,
        credential: &str,
        display: OnboardingDisplay,
    ) -> Result<OnboardingProgress, WebAuthError> {
        let principal = self.authenticate(credential).await?;
        let tenant_id = parse_tenant(&principal.tenant_id)?;
        let user_id = principal
            .user_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or(WebAuthError::Forbidden)?;
        self.repository
            .update_onboarding_display(tenant_id, user_id, display)
            .await?;
        self.onboarding(credential, None).await
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
            token: (self.invitation_delivery == InvitationDelivery::Manual).then(|| token.clone()),
            expires_at: Utc::now() + chrono::Duration::days(7),
            delivery: self.invitation_delivery,
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
        if self.invitation_delivery == InvitationDelivery::Email {
            self.email_queue
                .as_ref()
                .ok_or(WebAuthError::Unavailable)?
                .enqueue(AuthEmailKind::Invitation, &email, token.expose())
                .await?;
        }
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

    pub async fn register_invitation(
        &self,
        token: &str,
        password: &str,
    ) -> Result<LoginResult, WebAuthError> {
        self.limit("invitation_register", token, 10).await?;
        if self.registration_mode == RegistrationMode::Closed {
            return Err(WebAuthError::RegistrationClosed);
        }
        validate_password(password)?;
        let secret = token
            .strip_prefix(INVITATION_PREFIX)
            .filter(|value| value.len() >= 32)
            .ok_or(WebAuthError::Invalid)?;
        let (credential, session_hash) = new_session();
        let expires_at = expires_at(self.session_ttl);
        let user = WebUser {
            id: Uuid::now_v7(),
            email: String::new(),
            password_hash: hash_password(password)?,
            gateway_admin: false,
        };
        let membership = self
            .repository
            .register_invitation(
                &token_hash(secret),
                &user,
                &session_hash,
                expires_at,
                Utc::now(),
            )
            .await?;
        Ok(LoginResult {
            user_id: user.id,
            credential,
            expires_at,
            memberships: vec![membership],
        })
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

#[async_trait]
impl TrustedSessionIssuer for WebAuthService {
    async fn issue_trusted_session(&self, user_id: Uuid) -> Result<LoginResult, WebAuthError> {
        let (credential, session_hash) = new_session();
        let expires_at = expires_at(self.session_ttl);
        self.repository
            .create_session(user_id, &session_hash, expires_at)
            .await?;
        Ok(LoginResult {
            user_id,
            credential,
            expires_at,
            memberships: self.repository.memberships(user_id).await?,
        })
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

fn validate_tenant_name(value: &str) -> Result<&str, WebAuthError> {
    let tenant_name = value.trim();
    if !(1..=100).contains(&tenant_name.len()) {
        return Err(WebAuthError::Invalid);
    }
    Ok(tenant_name)
}

fn validate_signup_consent(input: &Signup) -> Result<(), WebAuthError> {
    if input.terms_accepted && input.privacy_acknowledged {
        Ok(())
    } else {
        Err(WebAuthError::Invalid)
    }
}

fn valid_bootstrap_token(expected: &[u8], token: &str) -> bool {
    if token
        .strip_prefix(BOOTSTRAP_PREFIX)
        .is_none_or(|value| value.len() < 32)
    {
        return false;
    }
    let presented = Sha256::digest(token.as_bytes());
    expected.len() == presented.len()
        && aws_lc_rs::constant_time::verify_slices_are_equal(expected, &presented).is_ok()
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

fn onboarding_progress(
    facts: OnboardingFacts,
    can_configure: bool,
    can_test: bool,
) -> OnboardingProgress {
    const IDS: [&str; 5] = [
        "create_project",
        "connect_provider",
        "create_route",
        "create_api_key",
        "send_first_request",
    ];
    let completed = [
        facts.project_ready,
        facts.provider_ready,
        facts.route_ready,
        facts.api_key_ready,
        facts.first_request_ready,
    ];
    let all_complete = completed.into_iter().all(|ready| ready);
    let current = completed.iter().position(|ready| !ready);
    let mut steps = Vec::with_capacity(IDS.len());
    for (index, id) in IDS.iter().enumerate() {
        let status = if completed[index] {
            OnboardingStepStatus::Complete
        } else if current == Some(index) {
            if can_configure || (index == 4 && can_test) {
                OnboardingStepStatus::Current
            } else {
                OnboardingStepStatus::NeedsAdmin
            }
        } else if current.is_some_and(|current| index > current) {
            OnboardingStepStatus::Blocked
        } else {
            OnboardingStepStatus::Pending
        };
        steps.push(OnboardingStep { id, status });
    }
    OnboardingProgress {
        version: 1,
        auto_open: facts.auto_open && facts.seen_at.is_none() && !all_complete,
        seen: facts.seen_at.is_some(),
        display: if facts.collapsed_at.is_some() || (!facts.auto_open && facts.seen_at.is_none()) {
            OnboardingDisplay::Collapsed
        } else {
            OnboardingDisplay::Expanded
        },
        can_configure,
        can_test,
        project_id: facts.project_id,
        completed_steps: completed.into_iter().filter(|ready| *ready).count(),
        total_steps: completed.len(),
        complete: all_complete,
        steps,
    }
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
    #[error("authentication rate limit exceeded")]
    RateLimited,
    #[error("registration is closed")]
    RegistrationClosed,
    #[error("bootstrap has already been consumed")]
    BootstrapConsumed,
    #[error("invalid bootstrap token")]
    InvalidBootstrapToken,
}

#[cfg(test)]
mod tests {
    use sha2::Digest;
    use uuid::Uuid;

    use super::{
        OnboardingDisplay, OnboardingFacts, OnboardingStepStatus, Signup, new_session,
        normalize_email, onboarding_progress, parse_credential, token_hash, valid_bootstrap_token,
        validate_signup_consent, validate_tenant_name,
    };

    #[test]
    fn validates_boundary_values() {
        assert_eq!(
            normalize_email(" User@Example.COM ").unwrap(),
            "user@example.com"
        );
        assert!(normalize_email("not-an-email").is_err());
        assert!(parse_credential("ws_short.00000000-0000-0000-0000-000000000000", true).is_err());
        assert_eq!(validate_tenant_name(" Acme ").unwrap(), "Acme");
    }

    #[test]
    fn requires_both_signup_legal_consents() {
        let base = Signup {
            email: "person@example.com".to_owned(),
            password: "a-secure-password".to_owned(),
            tenant_name: "Acme".to_owned(),
            terms_accepted: true,
            privacy_acknowledged: true,
        };
        assert!(validate_signup_consent(&base).is_ok());
        assert!(
            validate_signup_consent(&Signup {
                terms_accepted: false,
                ..base.clone()
            })
            .is_err()
        );
        assert!(
            validate_signup_consent(&Signup {
                privacy_acknowledged: false,
                ..base
            })
            .is_err()
        );
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

    #[test]
    fn bootstrap_token_requires_prefix_length_and_matching_digest() {
        let token = "tb_0123456789abcdef0123456789abcdef";
        let expected = sha2::Sha256::digest(token.as_bytes());
        assert!(valid_bootstrap_token(&expected, token));
        assert!(!valid_bootstrap_token(&expected, "tb_short"));
        assert!(!valid_bootstrap_token(
            &expected,
            "tb_ffffffffffffffffffffffffffffffff"
        ));
    }

    #[test]
    fn onboarding_is_ordered_and_read_only_users_need_an_admin() {
        let progress = onboarding_progress(
            OnboardingFacts {
                auto_open: true,
                seen_at: None,
                collapsed_at: None,
                project_id: Some("project".into()),
                project_ready: true,
                provider_ready: false,
                route_ready: false,
                api_key_ready: false,
                first_request_ready: false,
            },
            false,
            false,
        );
        assert!(progress.auto_open);
        assert_eq!(progress.display, OnboardingDisplay::Expanded);
        assert_eq!(progress.completed_steps, 1);
        assert_eq!(progress.steps[0].status, OnboardingStepStatus::Complete);
        assert_eq!(progress.steps[1].status, OnboardingStepStatus::NeedsAdmin);
        assert_eq!(progress.steps[2].status, OnboardingStepStatus::Blocked);
    }
}
