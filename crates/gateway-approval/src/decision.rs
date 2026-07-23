use chrono::{DateTime, Utc};
use gateway_types::{ApprovalId, ApprovalStatus};

#[derive(Clone, Debug, PartialEq)]
pub struct ApprovalDecision {
    pub approval_id: ApprovalId,
    pub tenant_id: String,
    pub decided_by: String,
    pub status: ApprovalStatus,
    pub sanitized_reason: Option<String>,
    pub decided_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExecutionClaim {
    Claimed,
    Completed(serde_json::Value),
    Indeterminate,
}
