use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ApprovalError {
    #[error("approval request not found")]
    NotFound,
    #[error("approval request is not accessible")]
    Forbidden,
    #[error("approval is pending")]
    Pending,
    #[error("approval was rejected")]
    Rejected,
    #[error("approval expired")]
    Expired,
    #[error("approval cannot be replayed")]
    Replay,
    #[error("approval persistence unavailable")]
    Unavailable,
}
