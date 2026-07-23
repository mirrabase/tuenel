//! Encrypted-at-rest secret storage with opaque references.

use std::sync::Arc;

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload},
};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gateway_types::SecretRef;
use rand::{RngCore, rngs::OsRng};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct SecretMaterialRecord {
    pub secret_ref: SecretRef,
    pub tenant_id: String,
    pub purpose: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[async_trait]
pub trait SecretRepository: Send + Sync {
    async fn insert_secret(&self, record: SecretMaterialRecord) -> Result<(), SecretError>;
    async fn secret(
        &self,
        tenant_id: &str,
        secret_ref: &SecretRef,
    ) -> Result<Option<SecretMaterialRecord>, SecretError>;
    async fn delete_secret(
        &self,
        tenant_id: &str,
        secret_ref: &SecretRef,
    ) -> Result<(), SecretError>;
}

#[derive(Clone)]
pub struct SecretService {
    repository: Arc<dyn SecretRepository>,
    cipher: Aes256Gcm,
}

impl SecretService {
    pub fn new(
        repository: Arc<dyn SecretRepository>,
        base64_key: &str,
    ) -> Result<Self, SecretError> {
        let key = STANDARD
            .decode(base64_key)
            .map_err(|_| SecretError::InvalidKey)?;
        if key.len() != 32 {
            return Err(SecretError::InvalidKey);
        }
        Ok(Self {
            repository,
            cipher: Aes256Gcm::new_from_slice(&key).map_err(|_| SecretError::InvalidKey)?,
        })
    }

    pub async fn store(
        &self,
        tenant_id: &str,
        purpose: &str,
        plaintext: &[u8],
    ) -> Result<SecretRef, SecretError> {
        let secret_ref = SecretRef(format!("sec_{}", uuid::Uuid::now_v7().simple()));
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let aad = format!("{tenant_id}:{purpose}:{}", secret_ref.0);
        let ciphertext = self
            .cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| SecretError::Crypto)?;
        self.repository
            .insert_secret(SecretMaterialRecord {
                secret_ref: secret_ref.clone(),
                tenant_id: tenant_id.into(),
                purpose: purpose.into(),
                nonce: nonce.to_vec(),
                ciphertext,
            })
            .await?;
        Ok(secret_ref)
    }

    pub async fn expose(
        &self,
        tenant_id: &str,
        secret_ref: &SecretRef,
    ) -> Result<SecretValue, SecretError> {
        let record = self
            .repository
            .secret(tenant_id, secret_ref)
            .await?
            .ok_or(SecretError::NotFound)?;
        let nonce: [u8; 12] = record.nonce.try_into().map_err(|_| SecretError::Crypto)?;
        let aad = format!(
            "{}:{}:{}",
            record.tenant_id, record.purpose, record.secret_ref.0
        );
        let plaintext = self
            .cipher
            .decrypt(
                (&nonce).into(),
                Payload {
                    msg: &record.ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| SecretError::Crypto)?;
        Ok(SecretValue(plaintext))
    }
    pub async fn delete(&self, tenant_id: &str, secret_ref: &SecretRef) -> Result<(), SecretError> {
        self.repository.delete_secret(tenant_id, secret_ref).await
    }
}

pub struct SecretValue(Vec<u8>);
impl SecretValue {
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl std::fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SecretError {
    #[error("credential encryption key is invalid")]
    InvalidKey,
    #[error("credential encryption failed")]
    Crypto,
    #[error("credential not found")]
    NotFound,
    #[error("credential persistence unavailable")]
    Unavailable,
}
