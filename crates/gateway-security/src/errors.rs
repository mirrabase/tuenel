use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SecurityError {
    #[error("security inspection failed")]
    InspectionFailed,
    #[error("security policy blocked the request")]
    Blocked,
    #[error("prompt injection risk detected")]
    PromptInjectionDetected,
    #[error("secret exposure detected")]
    SecretExposureDetected,
    #[error("sensitive data detected")]
    SensitiveDataDetected,
    #[error("human approval is required")]
    ApprovalRequired,
    #[error("security content is too large")]
    ContentTooLarge,
}
