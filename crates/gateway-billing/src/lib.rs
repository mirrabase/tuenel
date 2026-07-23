//! Billing-neutral usage delivery.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use gateway_secrets::SecretService;
use gateway_types::SecretRef;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Sign a webhook body using the documented timestamped HMAC payload.
pub fn signature(secret: &[u8], timestamp: i64, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts every key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    BASE64.encode(mac.finalize().into_bytes())
}

/// Constant-time verification of a webhook signature.
pub fn verify(secret: &[u8], timestamp: i64, body: &[u8], encoded: &str) -> bool {
    let Ok(expected) = BASE64.decode(encoded) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts every key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    mac.verify_slice(&expected).is_ok()
}

#[derive(Clone, Debug)]
pub struct BillingDelivery {
    pub event_id: Uuid,
    pub tenant_id: String,
    pub url: String,
    pub secret_ref: SecretRef,
    pub payload: serde_json::Value,
    pub attempt_count: u32,
    pub maximum_attempts: u32,
}

#[async_trait]
pub trait BillingRepository: Send + Sync {
    async fn claim_due(&self, now: DateTime<Utc>) -> Result<Option<BillingDelivery>, BillingError>;
    async fn finish_delivery(
        &self,
        event_id: Uuid,
        status_code: Option<u16>,
        error: Option<&str>,
        delivered: bool,
    ) -> Result<(), BillingError>;
}

#[derive(Clone)]
pub struct BillingWorker {
    repository: Arc<dyn BillingRepository>,
    secrets: SecretService,
    client: reqwest::Client,
}

impl BillingWorker {
    pub fn new(
        repository: Arc<dyn BillingRepository>,
        secrets: SecretService,
        timeout: Duration,
    ) -> Result<Self, BillingError> {
        Ok(Self {
            repository,
            secrets,
            client: reqwest::Client::builder()
                .timeout(timeout)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| BillingError::Unavailable)?,
        })
    }

    pub async fn deliver_once(&self) -> Result<bool, BillingError> {
        let Some(delivery) = self.repository.claim_due(Utc::now()).await? else {
            return Ok(false);
        };
        let secret = self
            .secrets
            .expose(&delivery.tenant_id, &delivery.secret_ref)
            .await
            .map_err(|_| BillingError::Unavailable)?;
        let body = serde_json::to_vec(&delivery.payload).map_err(|_| BillingError::Unavailable)?;
        let timestamp = Utc::now().timestamp();
        let response = self
            .client
            .post(&delivery.url)
            .header("content-type", "application/json")
            .header("x-gateway-timestamp", timestamp)
            .header(
                "x-gateway-signature",
                signature(secret.expose(), timestamp, &body),
            )
            .body(body)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                self.repository
                    .finish_delivery(
                        delivery.event_id,
                        Some(response.status().as_u16()),
                        None,
                        true,
                    )
                    .await?
            }
            Ok(response) => {
                self.repository
                    .finish_delivery(
                        delivery.event_id,
                        Some(response.status().as_u16()),
                        Some("webhook returned non-success status"),
                        false,
                    )
                    .await?
            }
            Err(_) => {
                self.repository
                    .finish_delivery(
                        delivery.event_id,
                        None,
                        Some("webhook transport failed"),
                        false,
                    )
                    .await?
            }
        }
        Ok(true)
    }

    pub async fn run(self) {
        loop {
            match self.deliver_once().await {
                Ok(true) => {}
                Ok(false) => tokio::time::sleep(Duration::from_secs(1)).await,
                Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
            }
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum BillingError {
    #[error("billing delivery unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::{signature, verify};

    #[test]
    fn signatures_are_verifiable_and_tamper_evident() {
        let signature = signature(b"secret", 123, br#"{"event_id":"1"}"#);
        assert!(verify(b"secret", 123, br#"{"event_id":"1"}"#, &signature));
        assert!(!verify(b"secret", 123, br#"{"event_id":"2"}"#, &signature));
    }
}
