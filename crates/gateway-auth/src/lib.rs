//! JWT/OIDC and Virtual Key authentication with Principal normalization.

mod web;

pub use web::{
    AuthAttemptLimiter, AuthCapabilities, AuthEmailKind, AuthEmailQueue, Bootstrap,
    IdentityRepository, InvitationDelivery, InvitationResult, LoginResult, Membership,
    Organization, OrganizationUpdate, PendingInvitation, RegistrationMode, SessionInfo, Signup,
    SignupResult, TenantRole, TrustedSessionIssuer, VerificationEmailSender, VerificationResult,
    WebAuthError, WebAuthService, WebUser,
};

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use gateway_keys::VirtualKeyService;
use gateway_store::GatewayStore;
use gateway_types::{AuthenticationMethod, Principal};
use jsonwebtoken::{
    Algorithm, DecodingKey, Validation, decode, decode_header,
    jwk::{Jwk, JwkSet},
};
use serde::Deserialize;
use thiserror::Error;
use tokio::{sync::RwLock, time::Instant};

/// Authentication boundary consumed by the HTTP transport.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Authenticate a bearer credential without logging it.
    async fn authenticate(&self, credential: &str) -> Result<Principal, AuthError>;
}

/// Combined JWT and Virtual Key authenticator.
pub struct AuthService {
    jwt: Option<JwtAuthenticator>,
    keys: VirtualKeyService,
    web: Option<WebAuthService>,
}

impl AuthService {
    /// Construct an authentication chain.
    pub fn new(jwt: Option<JwtAuthenticator>, keys: VirtualKeyService) -> Self {
        Self {
            jwt,
            keys,
            web: None,
        }
    }

    /// Enable gateway-owned browser sessions.
    pub fn with_web(mut self, web: WebAuthService) -> Self {
        self.web = Some(web);
        self
    }
}

#[async_trait]
impl Authenticator for AuthService {
    async fn authenticate(&self, credential: &str) -> Result<Principal, AuthError> {
        if credential.starts_with("ws_") {
            self.web
                .as_ref()
                .ok_or(AuthError::Invalid)?
                .authenticate(credential)
                .await
                .map_err(|error| match error {
                    WebAuthError::Unavailable => AuthError::Unavailable,
                    _ => AuthError::Invalid,
                })
        } else if credential.starts_with("vk_live_") {
            self.keys
                .authenticate(credential)
                .await
                .map_err(|_| AuthError::Invalid)
        } else {
            match &self.jwt {
                Some(jwt) => jwt.authenticate(credential).await,
                None => Err(AuthError::Invalid),
            }
        }
    }
}

/// One-issuer JWT validator backed by a bounded JWKS cache.
pub struct JwtAuthenticator {
    issuer: String,
    audience: String,
    jwks_url: String,
    client: reqwest::Client,
    store: Arc<dyn GatewayStore>,
    cache: RwLock<JwksCache>,
    cache_ttl: Duration,
}

impl JwtAuthenticator {
    /// Build a validator and fetch the initial JWKS document.
    pub async fn new(
        issuer: String,
        audience: String,
        jwks_url: String,
        store: Arc<dyn GatewayStore>,
    ) -> Result<Self, AuthError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| AuthError::Unavailable)?;
        let keys = fetch_jwks(&client, &jwks_url).await?;
        Ok(Self {
            issuer,
            audience,
            jwks_url,
            client,
            store,
            cache: RwLock::new(JwksCache {
                keys,
                expires_at: Instant::now() + Duration::from_secs(900),
            }),
            cache_ttl: Duration::from_secs(900),
        })
    }

    /// Validate a JWT and reject tenants not explicitly provisioned.
    pub async fn authenticate(&self, token: &str) -> Result<Principal, AuthError> {
        let header = decode_header(token).map_err(|_| AuthError::Invalid)?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::Invalid);
        }
        let kid = header.kid.ok_or(AuthError::Invalid)?;
        let mut jwk = self.cached_key(&kid).await;
        if jwk.is_none() {
            self.refresh().await?;
            jwk = self.cached_key(&kid).await;
        }
        let key = DecodingKey::from_jwk(jwk.as_ref().ok_or(AuthError::Invalid)?)
            .map_err(|_| AuthError::Invalid)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&[self.audience.as_str()]);
        validation.leeway = 30;
        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(|_| AuthError::Invalid)?
            .claims;
        if self
            .store
            .find_tenant(&claims.tenant_id)
            .await
            .map_err(|_| AuthError::Unavailable)?
            .is_none()
        {
            return Err(AuthError::UnknownTenant);
        }
        Ok(Principal {
            principal_id: format!("jwt:{}:{}", self.issuer, claims.sub),
            tenant_id: claims.tenant_id,
            project_id: None,
            user_id: Some(claims.sub),
            roles: claims.roles,
            scopes: Vec::new(),
            authentication_method: AuthenticationMethod::Jwt,
            virtual_key_id: None,
        })
    }

    async fn cached_key(&self, kid: &str) -> Option<Jwk> {
        let cache = self.cache.read().await;
        if cache.expires_at <= Instant::now() {
            return None;
        }
        cache
            .keys
            .keys
            .iter()
            .find(|key| key.common.key_id.as_deref() == Some(kid))
            .cloned()
    }

    async fn refresh(&self) -> Result<(), AuthError> {
        let keys = fetch_jwks(&self.client, &self.jwks_url).await?;
        *self.cache.write().await = JwksCache {
            keys,
            expires_at: Instant::now() + self.cache_ttl,
        };
        Ok(())
    }
}

#[derive(Debug)]
struct JwksCache {
    keys: JwkSet,
    expires_at: Instant,
}

#[derive(Clone, Debug, Deserialize)]
struct Claims {
    sub: String,
    tenant_id: String,
    #[serde(default)]
    roles: Vec<String>,
}

async fn fetch_jwks(client: &reqwest::Client, url: &str) -> Result<JwkSet, AuthError> {
    client
        .get(url)
        .send()
        .await
        .map_err(|_| AuthError::Unavailable)?
        .error_for_status()
        .map_err(|_| AuthError::Unavailable)?
        .json()
        .await
        .map_err(|_| AuthError::Unavailable)
}

/// Authentication failure safe to map at a transport boundary.
#[derive(Clone, Debug, Error)]
pub enum AuthError {
    /// Credential was invalid, expired, or unsupported.
    #[error("invalid credential")]
    Invalid,
    /// JWT referred to a tenant that was not provisioned.
    #[error("tenant is not provisioned")]
    UnknownTenant,
    /// Authentication dependency is unavailable.
    #[error("authentication unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{Json, Router, routing::get};
    use base64::{Engine, engine::general_purpose::STANDARD};
    use gateway_keys::VirtualKeyService;
    use gateway_store::{GatewayStore, TenantRecord};
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde::Serialize;
    use serde_json::json;
    use store_memory::MemoryStore;

    use super::{AuthError, AuthService, Authenticator, JwtAuthenticator};

    const PRIVATE_KEY_DER: &str = "MIIEowIBAAKCAQEA7N8zu98SzxpVwjFjoELTaBf2RjS1yjl2IETux15nvtJNoDea+WY5iMBoWc0AcNGKl66fQ51pRbASe4SFJbxAFd6/PwuX0aMRK562orw+emHTqJA1w2FnAB+5sHr6f2FdX31RJ1sLrBrPoDDHPMrfatTsDkMxLlL3OglZ1kFkwxYDx/3+AjxQ4IK+rgPQWQ2SLDKSu35ZszlG42gi98d0ieY2x10vwQxgYJ56TQnLJXxF1JChHEnFSJ38r68PrJJDS3KmtdJ6AGkE6YjyAIefJGytfkHYEj8U1RLCh/Rrjj/HLe3TY0cZb7ZmnkqPQbRHik1nzULGjjwFPU5tqcV2DQIDAQABAoIBAGm3lm3PeiQPk13X0CiLGrJXG3Wq+cWXDrcJPO6jHjdmwflzR8nui1gS05/cpEk15B6dc3xoaT8Ofbk95HT6hzmbkAxxvqD0H+oxbD7GODZDqgUN08jvFFrUTfpLcLhgOp2vHwSrGFMIJklO6+UggEU8YVxeNbmAktGGsd8zkWaVdVKcCoOjEbNUveu8Br8XjA9VNheG5osdGiifMRtSvliIQWJbDkAqebbSvq8O3neudC5xskUFu3mxN4TKvyLoL6PB7kZOxcSVAXehrpba12uRswUnqNbmzctFfNzVYhjgXmvAzxjXbZ4eOKKF9HePwy9V4ot/eCytEgxuULZ9mssCgYEA/dVU3P2zK2Z23wHW/DyuD6U6w0F7+CZTk2+7Xh311xmxV786D2qmOL7xkosXlxoaX6fVHfwLSG8wdOuEjSl8VuhlUFk2/MD0EipYmbj+p3T2igfF8UV4tIN6STUA8S6l0Fy9AB9vjW/1OoalMXgwUs3Smljjs53W2GY68scURo8CgYEA7uTOlnIR4oFPsEiUNNOyJunpBQ5Wt6w5jMTn+eX5ANBQgvzF3GwUssXU2uS6K8KOKj3+OGNlX8ZfWGzR/oNaqLgX06GSE1Q4H7BJafD0Iep8YMzVHRk6sCOQmrzJNIBtWwIEiR5i589kDw8TodKrGBTdMmHxfU3nCiRFv113Z6MCgYAmSTAkqQuGR000s9VWdFyYtYZYfx8Qvc8rVNYBCynSiOiL4KcEPkTWGE7dmKc1PlWuCeWGQUb+ZO79I6z5kcFUZncpmFtH4l6uAr8caJ/YaDbreOKtUpozOAWQ1zLOLggKloJXa2ZrAfEOI9L01DkNtEfIyhGGPQ9z0m+fwNFZFwKBgQDtJ6/+olcm6QBXHHYky1O8VdHB9y4XQJ4RJRi1eJvtNt/2aUFzRMh3gPWCKDa5YncHcGuDRwlIPwJAIieF5piFjdv5eBgvoBfnPXZj+ZQiZ0n6Pt4B+R3N5kCTnH6R5DyrcCFYjhXZ0oSefnUa3KyFR5EfhyPZJRELfF7RTtROyQKBgA7UKRo0KS0TlOsOQ2UpYOGBrGFtvk23MPuQs/FZ4tXrv/Sdvu3e+GaL8cln/LCotCvXFamDj8F0UAVOZrw5dcQUQXVfhyuWdUFm5H8wKEC2sBlfNcuiWaRO6NRYc1ykmrmgwGU/jlgL4C2Qe4y1MGoOphX4GvoVKnfg2+nM9xlO";
    const PUBLIC_N: &str = "7N8zu98SzxpVwjFjoELTaBf2RjS1yjl2IETux15nvtJNoDea-WY5iMBoWc0AcNGKl66fQ51pRbASe4SFJbxAFd6_PwuX0aMRK562orw-emHTqJA1w2FnAB-5sHr6f2FdX31RJ1sLrBrPoDDHPMrfatTsDkMxLlL3OglZ1kFkwxYDx_3-AjxQ4IK-rgPQWQ2SLDKSu35ZszlG42gi98d0ieY2x10vwQxgYJ56TQnLJXxF1JChHEnFSJ38r68PrJJDS3KmtdJ6AGkE6YjyAIefJGytfkHYEj8U1RLCh_Rrjj_HLe3TY0cZb7ZmnkqPQbRHik1nzULGjjwFPU5tqcV2DQ";

    #[derive(Serialize)]
    struct TestClaims<'a> {
        sub: &'a str,
        tenant_id: &'a str,
        roles: Vec<&'a str>,
        iss: &'a str,
        aud: &'a str,
        exp: usize,
    }

    #[tokio::test]
    async fn validates_signature_issuer_audience_expiry_and_tenant() {
        let jwks = json!({"keys":[{
            "kty":"RSA",
            "kid":"test-key",
            "alg":"RS256",
            "use":"sig",
            "n":PUBLIC_N,
            "e":"AQAB"
        }]});
        let app = Router::new().route("/jwks", get(move || async move { Json(jwks) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app).into_future());
        let issuer = format!("http://{address}");
        let store = Arc::new(MemoryStore::new());
        store
            .insert_tenant(TenantRecord {
                id: "tenant-a".into(),
                daily_token_limit: 1_000,
            })
            .await
            .unwrap();
        let validator = JwtAuthenticator::new(
            issuer.clone(),
            "gateway".into(),
            format!("{issuer}/jwks"),
            store,
        )
        .await
        .unwrap();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key".into());
        let private_der = STANDARD.decode(PRIVATE_KEY_DER).unwrap();
        let token = encode(
            &header,
            &TestClaims {
                sub: "user-a",
                tenant_id: "tenant-a",
                roles: vec!["gateway_admin"],
                iss: &issuer,
                aud: "gateway",
                exp: (chrono::Utc::now().timestamp() + 300) as usize,
            },
            &EncodingKey::from_rsa_der(&private_der),
        )
        .unwrap();
        let principal = validator.authenticate(&token).await.unwrap();
        assert_eq!(principal.tenant_id, "tenant-a");
        assert_eq!(principal.user_id.as_deref(), Some("user-a"));
    }

    #[tokio::test]
    async fn rejects_jwt_credentials_when_oidc_is_disabled() {
        let store: Arc<dyn GatewayStore> = Arc::new(MemoryStore::new());
        let authenticator = AuthService::new(None, VirtualKeyService::new(store));
        assert!(matches!(
            authenticator.authenticate("not-a-local-key").await,
            Err(AuthError::Invalid)
        ));
    }
}
