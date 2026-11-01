#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid date spec: {0}")]
    InvalidDateSpec(String),
    #[error("invalid time spec: {0}")]
    InvalidTimeSpec(String),
    #[error("invalid date-time spec: {0}")]
    InvalidDateTimeSpec(String),
    /// A start or end written in the spec (`@2025-01-06 ..2026-01-06`,
    /// `..+30Y`) is malformed or inconsistent, or a duration end has no start
    /// to count from.
    #[error("invalid start or end in spec: {0}")]
    InvalidSpecBounds(String),
    #[error("invalid end spec")]
    InvalidEndSpec,
    #[error("schedule iterator did not converge")]
    IteratorNotConverged,
    #[error("ambiguous or non-existent local time in target timezone: {0}")]
    AmbiguousLocalTime(String),
    /// A union spec (`A | B`) in [`IntervalMode::Elapsed`](crate::IntervalMode::Elapsed)
    /// can be iterated but not resumed: each part's elapsed steps depend on its
    /// own previous occurrence, which is not stored.
    #[error("cannot resume: {0}")]
    UnsupportedResume(String),
    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = core::result::Result<T, Error>;
