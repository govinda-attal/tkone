use chrono::NaiveDate;

/// Errors from loading or querying a calendar.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    /// A calendar file is not valid. Each problem is listed.
    #[error("calendar {code}: {}", problems.join("; "))]
    Invalid {
        /// The calendar's code.
        code: String,
        /// What is wrong.
        problems: Vec<String>,
    },
    /// A file could not be read or parsed.
    #[error("{0}")]
    Parse(String),
    /// No calendar has this code.
    #[error("unknown calendar `{0}`")]
    Unknown(String),
    /// The date is outside the dates the calendar answers for. A calendar
    /// never guesses.
    #[error("calendar {code} covers {from} to {until}, not {date}")]
    OutOfRange {
        /// The calendar's code.
        code: String,
        /// The date asked about.
        date: NaiveDate,
        /// The calendar's first date.
        from: NaiveDate,
        /// The calendar's last date.
        until: NaiveDate,
    },
}

/// A [`Calendar`](crate::Calendar) used as a
/// [`BizDayProcessor`](tkone_schedule::biz_day::BizDayProcessor) reports its
/// errors as [`tkone_schedule::Error::Custom`].
impl From<Error> for tkone_schedule::Error {
    fn from(e: Error) -> Self {
        tkone_schedule::Error::Custom(e.to_string())
    }
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;
