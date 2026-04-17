#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid date spec: {0}")]
    InvalidDateSpec(String),
    #[error("invalid time spec: {0}")]
    InvalidTimeSpec(String),
    #[error("invalid date-time spec: {0}")]
    InvalidDateTimeSpec(String),
    #[error("invalid end spec")]
    InvalidEndSpec,
    #[error("schedule iterator did not converge")]
    IteratorNotConverged,
    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = core::result::Result<T, Error>;
