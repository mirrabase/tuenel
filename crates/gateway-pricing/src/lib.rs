//! Versioned model pricing and cost calculation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct ModelPrice {
    pub price_id: Uuid,
    pub provider_id: String,
    pub upstream_model: String,
    pub input_cost_per_million: Decimal,
    pub output_cost_per_million: Decimal,
    pub cached_input_cost_per_million: Option<Decimal>,
    pub embedding_cost_per_million: Option<Decimal>,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
}

impl ModelPrice {
    pub fn active_at(&self, at: DateTime<Utc>) -> bool {
        self.effective_from <= at && self.effective_until.is_none_or(|until| at < until)
    }

    pub fn cost(&self, input: u64, output: u64, cached_input: u64, markup: Decimal) -> Decimal {
        let million = Decimal::from(1_000_000u64);
        let input_cost = Decimal::from(input.saturating_sub(cached_input))
            * self.input_cost_per_million
            / million;
        let cached_cost = self
            .cached_input_cost_per_million
            .unwrap_or(self.input_cost_per_million)
            * Decimal::from(cached_input)
            / million;
        let output_cost = Decimal::from(output) * self.output_cost_per_million / million;
        (input_cost + cached_cost + output_cost) * (Decimal::ONE + markup)
    }
}

pub fn validate_no_overlap(prices: &[ModelPrice]) -> Result<(), PricingError> {
    for (index, left) in prices.iter().enumerate() {
        for right in prices.iter().skip(index + 1) {
            if left.provider_id == right.provider_id
                && left.upstream_model == right.upstream_model
                && left
                    .effective_until
                    .is_none_or(|until| right.effective_from < until)
                && right
                    .effective_until
                    .is_none_or(|until| left.effective_from < until)
            {
                return Err(PricingError::OverlappingWindows);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PricingError {
    #[error("pricing windows overlap")]
    OverlappingWindows,
    #[error("pricing catalog unavailable")]
    Unavailable,
}

#[async_trait]
pub trait PricingCatalog: Send + Sync {
    async fn active_price(
        &self,
        provider_id: &str,
        upstream_model: &str,
        at: DateTime<Utc>,
    ) -> Result<Option<ModelPrice>, PricingError>;
}

#[cfg(test)]
mod tests {
    use super::{ModelPrice, validate_no_overlap};
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn price(from: chrono::DateTime<Utc>, until: Option<chrono::DateTime<Utc>>) -> ModelPrice {
        ModelPrice {
            price_id: Uuid::new_v4(),
            provider_id: "p".into(),
            upstream_model: "m".into(),
            input_cost_per_million: Decimal::ONE,
            output_cost_per_million: Decimal::from(2),
            cached_input_cost_per_million: None,
            embedding_cost_per_million: None,
            effective_from: from,
            effective_until: until,
        }
    }

    #[test]
    fn rejects_overlapping_price_windows() {
        let now = Utc::now();
        assert!(
            validate_no_overlap(&[
                price(now, Some(now + Duration::days(1))),
                price(now + Duration::hours(1), None)
            ])
            .is_err()
        );
    }

    #[test]
    fn calculates_decimal_cost_with_markup() {
        let now = Utc::now();
        let value = price(now, None).cost(1_000_000, 500_000, 0, Decimal::new(1, 1));
        assert_eq!(value, Decimal::new(22, 1));
    }
}
