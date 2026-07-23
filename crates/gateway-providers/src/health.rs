use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use super::{ProviderHealth, ProviderHealthStatus};
use async_trait::async_trait;
use std::{sync::Arc, time::Duration};

/// Process-local circuit state for one provider.
#[derive(Debug)]
pub struct ProviderHealthTracker {
    state: AtomicU8,
    consecutive_failures: AtomicU32,
    failure_threshold: u32,
}

#[async_trait]
pub trait ProviderHealthRepository: Send + Sync {
    async fn record_provider_health(
        &self,
        provider_id: &str,
        health: ProviderHealth,
    ) -> Result<(), super::ProviderError>;
}

#[derive(Clone)]
pub struct ProviderHealthMonitor {
    registry: super::ProviderRegistry,
    repository: Arc<dyn ProviderHealthRepository>,
    interval: Duration,
    timeout: Duration,
}

impl ProviderHealthMonitor {
    pub fn new(
        registry: super::ProviderRegistry,
        repository: Arc<dyn ProviderHealthRepository>,
        interval: Duration,
        timeout: Duration,
    ) -> Self {
        Self {
            registry,
            repository,
            interval,
            timeout,
        }
    }
    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            interval.tick().await;
            for id in self.registry.ids() {
                if let Some(provider) = self.registry.get(id) {
                    let health =
                        match tokio::time::timeout(self.timeout, provider.health_check()).await {
                            Ok(Ok(health)) => health,
                            _ => ProviderHealth {
                                status: ProviderHealthStatus::Unhealthy,
                                consecutive_failures: 1,
                                latest_success_at: None,
                                latest_failure_at: Some(chrono::Utc::now()),
                            },
                        };
                    let _ = self.repository.record_provider_health(id, health).await;
                }
            }
        }
    }
}

impl ProviderHealthTracker {
    pub fn new(failure_threshold: u32) -> Self {
        Self {
            state: AtomicU8::new(0),
            consecutive_failures: AtomicU32::new(0),
            failure_threshold: failure_threshold.max(1),
        }
    }

    pub fn is_available(&self) -> bool {
        self.state.load(Ordering::Acquire) != 2
    }

    pub fn success(&self) {
        self.consecutive_failures.store(0, Ordering::Release);
        self.state.store(0, Ordering::Release);
    }

    pub fn failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::AcqRel) + 1;
        self.state.store(
            if failures >= self.failure_threshold {
                2
            } else {
                1
            },
            Ordering::Release,
        );
    }

    pub fn snapshot(&self) -> ProviderHealth {
        let status = match self.state.load(Ordering::Acquire) {
            0 => ProviderHealthStatus::Healthy,
            1 => ProviderHealthStatus::Degraded,
            _ => ProviderHealthStatus::Unhealthy,
        };
        ProviderHealth {
            status,
            consecutive_failures: self.consecutive_failures.load(Ordering::Acquire),
            latest_success_at: None,
            latest_failure_at: None,
        }
    }
}
