//! Immutable usage recording and v0.1 cost calculation.

use std::sync::Arc;

use chrono::Utc;
use gateway_pricing::PricingCatalog;
use gateway_store::GatewayStore;
use gateway_types::{QuotaReservation, UsageEvent};
use rust_decimal::Decimal;
use thiserror::Error;

/// USD-per-million pricing for the sole configured route.
#[derive(Clone, Copy, Debug)]
pub struct Pricing {
    /// Input-token rate.
    pub input_per_million: Decimal,
    /// Output-token rate.
    pub output_per_million: Decimal,
}

impl Pricing {
    /// Calculate estimated request cost rounded to eight decimal places.
    pub fn cost(&self, prompt_tokens: u64, completion_tokens: u64) -> Decimal {
        let million = Decimal::from(1_000_000_u64);
        ((Decimal::from(prompt_tokens) * self.input_per_million
            + Decimal::from(completion_tokens) * self.output_per_million)
            / million)
            .round_dp(8)
    }
}

/// Usage ledger service.
#[derive(Clone)]
pub struct MeteringService {
    store: Arc<dyn GatewayStore>,
    pricing: Pricing,
    catalog: Option<Arc<dyn PricingCatalog>>,
}

impl MeteringService {
    /// Construct a metering service.
    pub fn new(store: Arc<dyn GatewayStore>, pricing: Pricing) -> Self {
        Self {
            store,
            pricing,
            catalog: None,
        }
    }

    pub fn with_catalog(mut self, catalog: Arc<dyn PricingCatalog>) -> Self {
        self.catalog = Some(catalog);
        self
    }

    /// Calculate configured route cost.
    pub fn cost(&self, prompt_tokens: u64, completion_tokens: u64) -> Decimal {
        self.pricing.cost(prompt_tokens, completion_tokens)
    }

    pub async fn cost_for(
        &self,
        provider: &str,
        model: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> Decimal {
        if let Some(catalog) = &self.catalog {
            if let Ok(Some(price)) = catalog.active_price(provider, model, Utc::now()).await {
                return price
                    .cost(prompt_tokens, completion_tokens, 0, Decimal::ZERO)
                    .round_dp(8);
            }
        }
        self.cost(prompt_tokens, completion_tokens)
    }

    /// Persist usage and finalize quota atomically.
    pub async fn finalize(
        &self,
        reservation: &QuotaReservation,
        event: UsageEvent,
    ) -> Result<(), MeteringError> {
        self.store
            .finalize_usage(reservation.reservation_id, event)
            .await
            .map_err(|_| MeteringError::Unavailable)
    }

    /// Release quota when the provider was never reached.
    pub async fn release(&self, reservation_id: uuid::Uuid) -> Result<(), MeteringError> {
        self.store
            .release_reservation(reservation_id)
            .await
            .map_err(|_| MeteringError::Unavailable)
    }
}

/// Metering persistence failure.
#[derive(Clone, Debug, Error)]
pub enum MeteringError {
    /// Ledger storage is unavailable.
    #[error("usage metering unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::Pricing;

    #[test]
    fn computes_decimal_cost() {
        let pricing = Pricing {
            input_per_million: Decimal::new(10, 0),
            output_per_million: Decimal::new(20, 0),
        };
        assert_eq!(pricing.cost(1_000, 500), Decimal::new(2, 2));
    }
}
