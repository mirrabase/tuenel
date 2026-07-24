//! Virtual Key issuance, hashing, verification, and revocation.

use std::sync::Arc;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use gateway_store::{GatewayStore, StoreError};
use gateway_types::{
    AuthenticationMethod, IssuedVirtualKey, NewVirtualKey, PlaintextVirtualKey, Principal,
    VirtualKeyRecord,
};
use rand::{RngCore, rngs::OsRng};
use thiserror::Error;
use uuid::Uuid;

/// Stateful Virtual Key service backed by a gateway store.
#[derive(Clone)]
pub struct VirtualKeyService {
    store: Arc<dyn GatewayStore>,
}

impl VirtualKeyService {
    /// Construct a service.
    pub fn new(store: Arc<dyn GatewayStore>) -> Self {
        Self { store }
    }

    /// Generate a new key and persist only its Argon2id hash.
    pub async fn issue(&self, input: NewVirtualKey) -> Result<IssuedVirtualKey, KeyError> {
        let mut prefix_bytes = [0_u8; 9];
        let mut secret_bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut prefix_bytes);
        OsRng.fill_bytes(&mut secret_bytes);
        let lookup_prefix = URL_SAFE_NO_PAD.encode(prefix_bytes);
        let secret = URL_SAFE_NO_PAD.encode(secret_bytes);
        let plaintext = format!("vk_live_{lookup_prefix}_{secret}");
        let salt = SaltString::generate(&mut OsRng);
        let secret_hash = Argon2::default()
            .hash_password(secret.as_bytes(), &salt)
            .map_err(|_| KeyError::Hashing)?
            .to_string();
        let record = VirtualKeyRecord {
            id: Uuid::now_v7(),
            display_name: input.display_name,
            lookup_prefix,
            secret_hash,
            tenant_id: input.tenant_id,
            project_id: input.project_id,
            user_id: input.user_id,
            scopes: input.scopes,
            expires_at: input.expires_at,
            revoked_at: None,
            daily_token_limit: input.daily_token_limit,
            allowed_models: input.allowed_models,
            daily_request_limit: input.daily_request_limit,
            monthly_budget: input.monthly_budget,
        };
        self.store.insert_virtual_key(record.clone()).await?;
        Ok(IssuedVirtualKey {
            record,
            plaintext: PlaintextVirtualKey::new(plaintext),
        })
    }

    /// Validate a key and normalize it to a Principal.
    pub async fn authenticate(&self, presented: &str) -> Result<Principal, KeyError> {
        let (lookup_prefix, secret) = parse_key(presented)?;
        let record = self
            .store
            .find_virtual_key_by_prefix(lookup_prefix)
            .await?
            .ok_or(KeyError::Invalid)?;
        if record.revoked_at.is_some() || record.expires_at.is_some_and(|time| time <= Utc::now()) {
            return Err(KeyError::Invalid);
        }
        let hash = PasswordHash::new(&record.secret_hash).map_err(|_| KeyError::Invalid)?;
        Argon2::default()
            .verify_password(secret.as_bytes(), &hash)
            .map_err(|_| KeyError::Invalid)?;
        self.store.touch_virtual_key(record.id).await?;
        let mut scopes = record.scopes;
        scopes.extend(
            record
                .allowed_models
                .iter()
                .map(|model| format!("model:{model}")),
        );
        Ok(Principal {
            principal_id: format!("virtual-key:{}", record.id),
            tenant_id: record.tenant_id,
            project_id: record.project_id,
            user_id: record.user_id,
            roles: Vec::new(),
            scopes,
            authentication_method: AuthenticationMethod::VirtualKey,
            virtual_key_id: Some(record.id),
        })
    }

    /// Revoke a tenant-owned key without revealing cross-tenant existence.
    pub async fn revoke(
        &self,
        tenant_id: &str,
        project_id: Option<&str>,
        key_id: Uuid,
    ) -> Result<bool, KeyError> {
        self.store
            .revoke_virtual_key(tenant_id, project_id, key_id)
            .await
            .map_err(Into::into)
    }
}

/// Virtual Key failure.
#[derive(Clone, Debug, Error)]
pub enum KeyError {
    /// The presented key is invalid, expired, or revoked.
    #[error("invalid virtual key")]
    Invalid,
    /// Key hashing failed.
    #[error("virtual key hashing failed")]
    Hashing,
    /// Persistence failed.
    #[error("virtual key persistence failed")]
    Store,
}

impl From<StoreError> for KeyError {
    fn from(_: StoreError) -> Self {
        Self::Store
    }
}

fn parse_key(key: &str) -> Result<(&str, &str), KeyError> {
    let remainder = key.strip_prefix("vk_live_").ok_or(KeyError::Invalid)?;
    let prefix = remainder.get(..12).ok_or(KeyError::Invalid)?;
    if remainder.as_bytes().get(12) != Some(&b'_') {
        return Err(KeyError::Invalid);
    }
    let secret = remainder
        .get(13..)
        .filter(|secret| secret.len() >= 32)
        .ok_or(KeyError::Invalid)?;
    Ok((prefix, secret))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gateway_store::{GatewayStore, TenantRecord};
    use gateway_types::NewVirtualKey;
    use store_memory::MemoryStore;

    use super::{VirtualKeyService, parse_key};

    #[test]
    fn rejects_malformed_keys() {
        assert!(parse_key("secret").is_err());
        assert!(parse_key("vk_live_short_secret").is_err());
        assert_eq!(
            parse_key("vk_live_abc_defghijk_abcdefghijklmnopqrstuvwxyz123456")
                .unwrap()
                .0,
            "abc_defghijk"
        );
    }

    #[tokio::test]
    async fn plaintext_is_one_time_and_revocation_is_enforced() {
        let store = Arc::new(MemoryStore::new());
        store
            .insert_tenant(TenantRecord {
                id: "tenant-a".into(),
                daily_token_limit: 10_000,
            })
            .await
            .unwrap();
        let service = VirtualKeyService::new(store);
        let issued = service
            .issue(NewVirtualKey {
                tenant_id: "tenant-a".into(),
                display_name: Some("test".into()),
                project_id: None,
                user_id: Some("user-a".into()),
                scopes: vec!["chat".into()],
                expires_at: None,
                daily_token_limit: 1_000,
                allowed_models: vec!["gateway-default".into()],
                daily_request_limit: Some(100),
                monthly_budget: None,
            })
            .await
            .unwrap();
        assert!(!format!("{issued:?}").contains(issued.plaintext.expose()));
        let principal = service
            .authenticate(issued.plaintext.expose())
            .await
            .unwrap();
        assert_eq!(principal.tenant_id, "tenant-a");
        assert!(
            principal
                .scopes
                .contains(&"model:gateway-default".to_owned())
        );
        assert!(
            service
                .revoke("tenant-a", None, issued.record.id)
                .await
                .unwrap()
        );
        assert!(
            service
                .authenticate(issued.plaintext.expose())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn project_revocation_cannot_cross_project_boundary() {
        let store = Arc::new(MemoryStore::new());
        store
            .insert_tenant(TenantRecord {
                id: "tenant-a".into(),
                daily_token_limit: 10_000,
            })
            .await
            .unwrap();
        let service = VirtualKeyService::new(store);
        let issued = service
            .issue(NewVirtualKey {
                tenant_id: "tenant-a".into(),
                display_name: Some("project-a".into()),
                project_id: Some("project-a".into()),
                user_id: None,
                scopes: vec!["inference".into()],
                expires_at: None,
                daily_token_limit: 1_000,
                allowed_models: Vec::new(),
                daily_request_limit: None,
                monthly_budget: None,
            })
            .await
            .unwrap();
        assert!(
            !service
                .revoke("tenant-a", Some("project-b"), issued.record.id)
                .await
                .unwrap()
        );
        assert!(
            service
                .authenticate(issued.plaintext.expose())
                .await
                .is_ok()
        );
        assert!(
            service
                .revoke("tenant-a", Some("project-a"), issued.record.id)
                .await
                .unwrap()
        );
    }
}
