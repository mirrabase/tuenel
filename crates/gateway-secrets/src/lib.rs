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
    cipher: SecretCipher,
}

#[derive(Clone)]
pub struct SecretCipher(Aes256Gcm);

pub struct EncryptedValue {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl SecretCipher {
    pub fn new(base64_key: &str) -> Result<Self, SecretError> {
        let key = STANDARD
            .decode(base64_key)
            .map_err(|_| SecretError::InvalidKey)?;
        if key.len() != 32 {
            return Err(SecretError::InvalidKey);
        }
        Ok(Self(
            Aes256Gcm::new_from_slice(&key).map_err(|_| SecretError::InvalidKey)?,
        ))
    }

    pub fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<EncryptedValue, SecretError> {
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .0
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| SecretError::Crypto)?;
        Ok(EncryptedValue {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    pub fn open(
        &self,
        aad: &[u8],
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, SecretError> {
        let nonce: [u8; 12] = nonce.try_into().map_err(|_| SecretError::Crypto)?;
        self.0
            .decrypt(
                (&nonce).into(),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| SecretError::Crypto)
    }
}

impl SecretService {
    pub fn new(
        repository: Arc<dyn SecretRepository>,
        base64_key: &str,
    ) -> Result<Self, SecretError> {
        Ok(Self {
            repository,
            cipher: SecretCipher::new(base64_key)?,
        })
    }

    pub async fn store(
        &self,
        tenant_id: &str,
        purpose: &str,
        plaintext: &[u8],
    ) -> Result<SecretRef, SecretError> {
        let secret_ref = SecretRef(format!("sec_{}", uuid::Uuid::now_v7().simple()));
        let aad = format!("{tenant_id}:{purpose}:{}", secret_ref.0);
        let encrypted = self.cipher.seal(aad.as_bytes(), plaintext)?;
        self.repository
            .insert_secret(SecretMaterialRecord {
                secret_ref: secret_ref.clone(),
                tenant_id: tenant_id.into(),
                purpose: purpose.into(),
                nonce: encrypted.nonce,
                ciphertext: encrypted.ciphertext,
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
        let aad = format!(
            "{}:{}:{}",
            record.tenant_id, record.purpose, record.secret_ref.0
        );
        let plaintext = self
            .cipher
            .open(aad.as_bytes(), &record.nonce, &record.ciphertext)?;
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

#[cfg(test)]
mod tests {
    use super::SecretCipher;

    #[test]
    fn encrypted_values_require_the_same_aad_and_never_contain_plaintext() {
        let cipher = SecretCipher::new("MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=").unwrap();
        let encrypted = cipher.seal(b"auth:event", b"wv_secret-token").unwrap();
        assert!(
            !encrypted
                .ciphertext
                .windows(b"wv_secret-token".len())
                .any(|window| window == b"wv_secret-token")
        );
        assert_eq!(
            cipher
                .open(b"auth:event", &encrypted.nonce, &encrypted.ciphertext)
                .unwrap(),
            b"wv_secret-token"
        );
        assert!(
            cipher
                .open(b"auth:other", &encrypted.nonce, &encrypted.ciphertext)
                .is_err()
        );
    }
}
