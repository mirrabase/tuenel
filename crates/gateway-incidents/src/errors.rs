use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum IncidentError {
    #[error("security incident not found")]
    NotFound,
    #[error("security incident persistence unavailable")]
    Unavailable,
}
