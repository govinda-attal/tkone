#![feature(let_chains)]

//! # lib-schedule
//!
//! `lib-schedule` is a crate for handling scheduling, date and time calculations, and business day processing.
//! It provides various utilities and structures to work with dates, times, and business days in different time zones.
//!
//! ## Modules
//!
//! - [`biz_day`]: Contains utilities and structures for business day processing.
//! - [`date`]: Provides date-related utilities and structures.
//! - [`time`]: Contains time-related utilities and structures.
//! - [`datetime`]: Contains date and time-related utilities and structures.

/// The `biz_day` module contains utilities and structures for business day processing.
pub mod biz_day;
/// The `date` module provides date-related utilities and structures.
pub mod date;
/// The `datetime` module contains date and time-related utilities and structures.
pub mod datetime;
/// The `time` module contains time-related utilities and structures.
pub mod time;

/// The `error` module defines error types used throughout the crate.
mod error;
mod prelude;
mod utils;

/// Represents the result of a scheduling operation.
///
/// The `NextResult` enum is used to indicate the result of a scheduling operation. It can either be a single result
/// or an adjusted result with two values.
///
/// # Variants
///
/// - `Single(T)`: Represents a single result.
/// - `AdjustedEarlier(T, T)`: Represents an adjusted result where the first value is the earlier adjustment.
/// - `AdjustedLater(T, T)`: Represents an adjusted result where the first value is the later adjustment.
///
/// # Methods
///
/// - `final_value(&self) -> &T`: Returns the final value of the scheduling operation.
/// - `actual(&self) -> &T`: Returns the actual value of the scheduling operation.
/// - `as_tuple(&self) -> (&T, &T)`: Returns the result as a tuple of two values.
#[derive(Debug, Clone)]
pub enum NextResult<T: Clone> {
    /// A single result.
    Single(T),
    /// An adjusted result with the earlier adjustment.
    AdjustedLater(T, T),
    /// An adjusted result with the later adjustment.
    AdjustedEarlier(T, T),
}

impl<T: Clone> NextResult<T> {
    /// Returns the single value if the result is `Single`.
    ///
    /// This method returns the single value if the result is `Single`. Otherwise, it returns `None`.
    pub fn single(self) -> Option<T> {
        match self {
            NextResult::Single(t) => Some(t),
            _ => None,
        }
    }

    /// Returns the earlier value of the scheduling operation.
    ///
    /// This method returns the earlier value of the scheduling operation, which is the second value in the case of
    /// `AdjustedEarlier`, and the first value in the case of `AdjustedLater` and `Single`.
    pub fn earlier(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(_, t)
            | NextResult::AdjustedLater(t, _) => t,
        }
    }

    /// Returns the later value of the scheduling operation.
    ///
    /// This method returns the later value of the scheduling operation, which is the first value in the case of
    /// `AdjustedEarlier`, and the second value in the case of `AdjustedLater` and `Single`.
    pub fn later(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(t, _)
            | NextResult::AdjustedLater(_, t) => t,
        }
    }

    /// Returns the observed value of the scheduling operation.
    ///
    /// This method returns the observed value of the scheduling operation, which is the second value in the case of
    /// `AdjustedEarlier` and `AdjustedLater`, and the single value in the case of `Single`.
    pub fn observed(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(_, t)
            | NextResult::AdjustedLater(_, t) => t,
        }
    }

    /// Returns the actual value of the scheduling operation.
    ///
    /// This method returns the actual value of the scheduling operation, which is the first value in the case of
    /// `AdjustedEarlier` and `AdjustedLater`, and the single value in the case of `Single`.
    pub fn actual(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(t, _)
            | NextResult::AdjustedLater(t, _) => t,
        }
    }

    /// Returns the result as a tuple of two values.
    ///
    /// This method returns the result as a tuple of two values. In the case of `Single`, both values in the tuple
    /// are the same. In the case of `AdjustedEarlier` and `AdjustedLater`, the tuple contains both values.
    pub fn as_tuple(&self) -> (&T, &T) {
        match self {
            NextResult::Single(t) => (t, t),
            NextResult::AdjustedEarlier(actual, adjusted)
            | NextResult::AdjustedLater(actual, adjusted) => (actual, adjusted),
        }
    }
}

#[cfg(test)]
mod tests {
    use biz_day::WeekendSkipper;
    use chrono::{DateTime, TimeZone};
    use chrono_tz::America::New_York;
    use fallible_iterator::FallibleIterator;

    use super::*;
    #[test]
    fn test_works() {
        let tmp = datetime::SpecIteratorBuilder::new_with_start(
            "YY:1M:08:WT11:00:00",
            WeekendSkipper::new(),
            New_York.with_ymd_and_hms(2024, 11, 30, 11, 0, 0).unwrap(),
        )
        .with_end(New_York.with_ymd_and_hms(2025, 7, 31, 11, 00, 0).unwrap())
        .build()
        .unwrap();
        let tmp = tmp
            .take(10)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap();
        dbg!(&tmp);
    }
}
