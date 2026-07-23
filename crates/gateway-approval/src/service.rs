use std::{sync::Arc, time::Duration};

use chrono::{Duration as ChronoDuration, Utc};
use gateway_types::{
    ApprovalId, ApprovalRequest, ApprovalResourceType, ApprovalStatus, Principal, ToolRiskLevel,
};
use tracing::Instrument;
use uuid::Uuid;

use crate::{ApprovalDecision, ApprovalError, ApprovalRepository, ExecutionClaim, expired};

#[derive(Clone)]
pub struct ApprovalService {
    repository: Arc<dyn ApprovalRepository>,
    expiration: Duration,
}

impl ApprovalService {
    pub fn new(repository: Arc<dyn ApprovalRepository>, expiration: Duration) -> Self {
        Self {
            repository,
            expiration,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        principal: &Principal,
        request_id: Uuid,
        resource_type: ApprovalResourceType,
        resource_id: String,
        action: String,
        sanitized_arguments: serde_json::Value,
        risk_level: ToolRiskLevel,
        request_hash: String,
    ) -> Result<ApprovalRequest, ApprovalError> {
        let now = Utc::now();
        let request = ApprovalRequest {
            approval_id: ApprovalId::new(),
            tenant_id: principal.tenant_id.clone(),
            project_id: principal.project_id.clone(),
            principal_id: principal.principal_id.clone(),
            request_id,
            resource_type,
            resource_id,
            action,
            sanitized_arguments,
            risk_level,
            status: ApprovalStatus::Pending,
            request_hash,
            expires_at: now
                + ChronoDuration::from_std(self.expiration)
                    .map_err(|_| ApprovalError::Unavailable)?,
            created_at: now,
        };
        self.repository.insert_approval(request.clone()).instrument(tracing::info_span!("gateway.approval.create",request_id=%request_id,tenant_id=%principal.tenant_id,principal_id=%principal.principal_id)).await?;
        gateway_observability::metrics().approvals_pending.inc();
        Ok(request)
    }

    pub async fn get(
        &self,
        tenant_id: &str,
        approval_id: ApprovalId,
    ) -> Result<ApprovalRequest, ApprovalError> {
        self.expire().await?;
        let request = self
            .repository
            .approval(tenant_id, approval_id)
            .await?
            .ok_or(ApprovalError::NotFound)?;
        if request.status == ApprovalStatus::Pending && expired(request.expires_at, Utc::now()) {
            return Err(ApprovalError::Expired);
        }
        Ok(request)
    }

    pub async fn list(
        &self,
        tenant_id: &str,
        status: Option<ApprovalStatus>,
        limit: u32,
    ) -> Result<Vec<ApprovalRequest>, ApprovalError> {
        self.expire().await?;
        self.repository
            .list_approvals(tenant_id, status, limit.min(200))
            .await
    }

    pub async fn decide(
        &self,
        tenant_id: &str,
        approval_id: ApprovalId,
        administrator: &Principal,
        approve: bool,
        reason: Option<String>,
    ) -> Result<ApprovalRequest, ApprovalError> {
        let current = self.get(tenant_id, approval_id).await?;
        if current.status != ApprovalStatus::Pending {
            return Err(match current.status {
                ApprovalStatus::Rejected => ApprovalError::Rejected,
                ApprovalStatus::Expired => ApprovalError::Expired,
                _ => ApprovalError::Replay,
            });
        }
        let result=self.repository.decide_approval(ApprovalDecision { approval_id, tenant_id: tenant_id.into(), decided_by: administrator.principal_id.clone(), status: if approve { ApprovalStatus::Approved } else { ApprovalStatus::Rejected }, sanitized_reason: reason.map(|value| value.chars().take(512).collect()), decided_at: Utc::now() }).instrument(tracing::info_span!("gateway.approval.resolve",tenant_id=%tenant_id,principal_id=%administrator.principal_id,status=if approve{"approved"}else{"rejected"})).await?;
        let metrics = gateway_observability::metrics();
        metrics.approvals_pending.dec();
        if approve {
            metrics.approvals_approved.inc()
        } else {
            metrics.approvals_rejected.inc()
        }
        Ok(result)
    }

    pub async fn authorize_retry(
        &self,
        principal: &Principal,
        approval_id: ApprovalId,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<ExecutionClaim, ApprovalError> {
        if idempotency_key.trim().is_empty() || idempotency_key.len() > 255 {
            return Err(ApprovalError::Replay);
        }
        let request = self.get(&principal.tenant_id, approval_id).await?;
        if request.principal_id != principal.principal_id || request.request_hash != request_hash {
            return Err(ApprovalError::Forbidden);
        }
        match request.status {
            ApprovalStatus::Approved => {
                self.repository
                    .claim_execution(
                        &request,
                        &principal.principal_id,
                        idempotency_key,
                        request_hash,
                    )
                    .await
            }
            ApprovalStatus::Pending => Err(ApprovalError::Pending),
            ApprovalStatus::Rejected => Err(ApprovalError::Rejected),
            ApprovalStatus::Expired => Err(ApprovalError::Expired),
            ApprovalStatus::Cancelled => Err(ApprovalError::Replay),
        }
    }

    pub async fn complete(
        &self,
        approval_id: ApprovalId,
        idempotency_key: &str,
        result: serde_json::Value,
    ) -> Result<(), ApprovalError> {
        self.repository
            .complete_execution(approval_id, idempotency_key, result)
            .await
    }
    pub async fn fail(
        &self,
        approval_id: ApprovalId,
        idempotency_key: &str,
        indeterminate: bool,
    ) -> Result<(), ApprovalError> {
        self.repository
            .fail_execution(approval_id, idempotency_key, indeterminate)
            .await
    }

    async fn expire(&self) -> Result<(), ApprovalError> {
        let count = self.repository.expire_approvals(Utc::now()).await?;
        if count > 0 {
            let metrics = gateway_observability::metrics();
            metrics.approvals_expired.inc_by(count);
            metrics
                .approvals_pending
                .sub(i64::try_from(count).unwrap_or(i64::MAX));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApprovalDecision, ApprovalRepository, ExecutionClaim};
    use async_trait::async_trait;
    use gateway_types::{ApprovalStatus, AuthenticationMethod, Principal};
    use tokio::sync::Mutex;
    #[derive(Default)]
    struct Repo {
        approval: Mutex<Option<ApprovalRequest>>,
        execution: Mutex<Option<(String, String, Option<serde_json::Value>)>>,
    }
    #[async_trait]
    impl ApprovalRepository for Repo {
        async fn insert_approval(&self, value: ApprovalRequest) -> Result<(), ApprovalError> {
            *self.approval.lock().await = Some(value);
            Ok(())
        }
        async fn approval(
            &self,
            tenant: &str,
            id: ApprovalId,
        ) -> Result<Option<ApprovalRequest>, ApprovalError> {
            Ok(self
                .approval
                .lock()
                .await
                .clone()
                .filter(|value| value.tenant_id == tenant && value.approval_id == id))
        }
        async fn list_approvals(
            &self,
            tenant: &str,
            status: Option<ApprovalStatus>,
            _: u32,
        ) -> Result<Vec<ApprovalRequest>, ApprovalError> {
            Ok(self
                .approval
                .lock()
                .await
                .clone()
                .filter(|value| {
                    value.tenant_id == tenant && status.is_none_or(|status| status == value.status)
                })
                .into_iter()
                .collect())
        }
        async fn decide_approval(
            &self,
            decision: ApprovalDecision,
        ) -> Result<ApprovalRequest, ApprovalError> {
            let mut guard = self.approval.lock().await;
            let value = guard.as_mut().ok_or(ApprovalError::NotFound)?;
            value.status = decision.status;
            Ok(value.clone())
        }
        async fn expire_approvals(&self, now: chrono::DateTime<Utc>) -> Result<u64, ApprovalError> {
            let mut guard = self.approval.lock().await;
            if let Some(value) = guard
                .as_mut()
                .filter(|value| value.status == ApprovalStatus::Pending && value.expires_at <= now)
            {
                value.status = ApprovalStatus::Expired;
                Ok(1)
            } else {
                Ok(0)
            }
        }
        async fn claim_execution(
            &self,
            _: &ApprovalRequest,
            _: &str,
            key: &str,
            hash: &str,
        ) -> Result<ExecutionClaim, ApprovalError> {
            let mut guard = self.execution.lock().await;
            match guard.as_ref() {
                None => {
                    *guard = Some((key.into(), hash.into(), None));
                    Ok(ExecutionClaim::Claimed)
                }
                Some((stored_key, stored_hash, result))
                    if stored_key == key && stored_hash == hash =>
                {
                    Ok(result
                        .clone()
                        .map_or(ExecutionClaim::Indeterminate, ExecutionClaim::Completed))
                }
                _ => Err(ApprovalError::Replay),
            }
        }
        async fn complete_execution(
            &self,
            _: ApprovalId,
            key: &str,
            result: serde_json::Value,
        ) -> Result<(), ApprovalError> {
            let mut guard = self.execution.lock().await;
            if let Some((_, _, value)) = guard.as_mut().filter(|(stored, _, _)| stored == key) {
                *value = Some(result);
                Ok(())
            } else {
                Err(ApprovalError::Replay)
            }
        }
        async fn fail_execution(
            &self,
            _: ApprovalId,
            _: &str,
            _: bool,
        ) -> Result<(), ApprovalError> {
            Ok(())
        }
    }
    fn principal() -> Principal {
        Principal {
            principal_id: "user".into(),
            tenant_id: "tenant".into(),
            project_id: None,
            user_id: Some("user".into()),
            roles: vec![],
            scopes: vec![],
            virtual_key_id: None,
            authentication_method: AuthenticationMethod::Jwt,
        }
    }
    #[tokio::test]
    async fn lifecycle_expiration_and_idempotency() {
        let repository = Arc::new(Repo::default());
        let service = ApprovalService::new(repository, Duration::from_secs(60));
        let principal = principal();
        let request = service
            .create(
                &principal,
                Uuid::now_v7(),
                ApprovalResourceType::McpTool,
                "server".into(),
                "delete".into(),
                serde_json::json!({}),
                ToolRiskLevel::Destructive,
                "hash".into(),
            )
            .await
            .unwrap();
        service
            .decide("tenant", request.approval_id, &principal, true, None)
            .await
            .unwrap();
        assert_eq!(
            service
                .authorize_retry(&principal, request.approval_id, "once", "hash")
                .await
                .unwrap(),
            ExecutionClaim::Claimed
        );
        service
            .complete(request.approval_id, "once", serde_json::json!({"ok":true}))
            .await
            .unwrap();
        assert!(matches!(
            service
                .authorize_retry(&principal, request.approval_id, "once", "hash")
                .await
                .unwrap(),
            ExecutionClaim::Completed(_)
        ));
        assert_eq!(
            service
                .authorize_retry(&principal, request.approval_id, "twice", "hash")
                .await
                .unwrap_err(),
            ApprovalError::Replay
        );
    }
    #[tokio::test]
    async fn expired_approval_is_denied() {
        let repository = Arc::new(Repo::default());
        let service = ApprovalService::new(repository.clone(), Duration::from_secs(60));
        let principal = principal();
        let request = service
            .create(
                &principal,
                Uuid::now_v7(),
                ApprovalResourceType::McpTool,
                "server".into(),
                "delete".into(),
                serde_json::json!({}),
                ToolRiskLevel::Destructive,
                "hash".into(),
            )
            .await
            .unwrap();
        repository
            .approval
            .lock()
            .await
            .as_mut()
            .unwrap()
            .expires_at = Utc::now() - ChronoDuration::seconds(1);
        assert_eq!(
            service
                .get("tenant", request.approval_id)
                .await
                .unwrap()
                .status,
            ApprovalStatus::Expired
        );
    }
}
