#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid date spec: {0}")]
    InvalidDateSpec(String),
    #[error("invalid time spec: {0}")]
    InvalidTimeSpec(String),
    #[error("invalid date-time spec: {0}")]
    InvalidDateTimeSpec(String),
    /// A calendar is unknown, or judges business days in another time zone
    /// than the schedule's.
    #[error("calendar: {0}")]
    Calendar(String),
    /// A [`Schedule`](crate::schedule::Schedule) is inconsistent: no rules, a
    /// rule that does not parse, a repeated rule id, …
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),
    #[error("invalid end spec")]
    InvalidEndSpec,
    /// A rule found no date within its search limit. A rule that can never
    /// happen whatever the start (`YY-02-30`) is refused when it is read;
    /// one that never happens from this start is reported here (`4Y-02-29`
    /// counted from a year that is not a leap year).
    #[error(
        "the rule found no date within its search limit: it may never happen from this start \
         (`4Y-02-29` counted from a year that is not a leap year never does)"
    )]
    IteratorNotConverged,
    #[error("ambiguous or non-existent local time in target timezone: {0}")]
    AmbiguousLocalTime(String),
    /// A schedule cannot be resumed from its last occurrence alone: a cycle
    /// with positions, or a count, needs `resume_with_start`.
    #[error("cannot resume: {0}")]
    UnsupportedResume(String),
    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = core::result::Result<T, Error>;
