use async_trait::async_trait;
use gateway_types::{ApprovalId, ApprovalRequest, ApprovalStatus};

use crate::{ApprovalDecision, ApprovalError, ExecutionClaim};

#[async_trait]
pub trait ApprovalRepository: Send + Sync {
    async fn insert_approval(&self, request: ApprovalRequest) -> Result<(), ApprovalError>;
    async fn approval(
        &self,
        tenant_id: &str,
        approval_id: ApprovalId,
    ) -> Result<Option<ApprovalRequest>, ApprovalError>;
    async fn list_approvals(
        &self,
        tenant_id: &str,
        status: Option<ApprovalStatus>,
        limit: u32,
    ) -> Result<Vec<ApprovalRequest>, ApprovalError>;
    async fn decide_approval(
        &self,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRequest, ApprovalError>;
    async fn expire_approvals(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, ApprovalError>;
    async fn claim_execution(
        &self,
        request: &ApprovalRequest,
        principal_id: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<ExecutionClaim, ApprovalError>;
    async fn complete_execution(
        &self,
        approval_id: ApprovalId,
        idempotency_key: &str,
        result: serde_json::Value,
    ) -> Result<(), ApprovalError>;
    async fn fail_execution(
        &self,
        approval_id: ApprovalId,
        idempotency_key: &str,
        indeterminate: bool,
    ) -> Result<(), ApprovalError>;
}
