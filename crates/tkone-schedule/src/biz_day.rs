//! Business day processing traits and implementations.
//!
//! The central abstraction is [`BizDayProcessor`](crate::biz_day::BizDayProcessor): any type that can answer
//! "is this date a business day?" and advance/retreat by a given number of
//! business days. Supply your own implementation to encode market holidays,
//! custom cut-off calendars, etc.
//!
//! The built-in [`WeekendSkipper`](crate::biz_day::WeekendSkipper) treats every Monday–Friday as a business
//! day, regardless of public holidays.

use std::fmt::Debug;

use crate::{prelude::*, utils::DateLikeUtils};
use chrono::{Datelike, Duration, NaiveDateTime};

/// Trait for pluggable business day logic.
///
/// Implement this to encode holiday calendars, exchange-specific cut-off
/// rules, or any other definition of "business day". The library uses this
/// trait when resolving [`crate::date::BizDayAdjustment`] values and when
/// interpreting `nBD` day-spec tokens.
///
/// All methods receive and return [`chrono::NaiveDateTime`] so that
/// implementations can (optionally) respect intra-day cut-off times, though
/// most calendar-only implementations will ignore the time component.
///
/// # Implementing
///
/// ```rust
/// use tkone_schedule::biz_day::{BizDayProcessor, Direction};
/// use tkone_schedule::{Error, Result};
/// use chrono::{Datelike, NaiveDateTime};
///
/// #[derive(Debug, Clone)]
/// struct Mon2FriOnly;
///
/// impl BizDayProcessor for Mon2FriOnly {
///     fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool> {
///         let wd = dtm.weekday();
///         Ok(!matches!(wd, chrono::Weekday::Sat | chrono::Weekday::Sun))
///     }
///
///     fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime> {
///         match direction {
///             Direction::Next    => self.add(dtm, 1),
///             Direction::Prev    => self.sub(dtm, 1),
///             Direction::Nearest => {
///                 if self.is_biz_day(dtm)? { Ok(*dtm) } else { self.add(dtm, 1) }
///             }
///         }
///     }
///
///     fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
///         let mut cur = *dtm;
///         let mut n = 0;
///         while n < num {
///             cur += chrono::Duration::days(1);
///             if self.is_biz_day(&cur)? { n += 1; }
///         }
///         Ok(cur)
///     }
///
///     fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
///         let mut cur = *dtm;
///         let mut n = 0;
///         while n < num {
///             cur -= chrono::Duration::days(1);
///             if self.is_biz_day(&cur)? { n += 1; }
///         }
///         Ok(cur)
///     }
/// }
/// ```
pub trait BizDayProcessor: Debug + Clone + Send + Sync + 'static {
    /// Returns `true` when `dtm` falls on a business day.
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool>;

    /// Finds the nearest business day relative to `dtm` in the given
    /// [`Direction`].
    ///
    /// The library calls this only for a `dtm` that is **not** a business day
    /// (to resolve `~NB`, `~PB`, `~B`, …), so implementations need not special-
    /// case a `dtm` that already is one.
    ///
    /// The [`Direction::Nearest`] variant's tie-breaking rule is
    /// implementation-defined. [`WeekendSkipper`] uses the following convention:
    /// - If the date is the 1st of the month and falls on a weekend, it steps
    ///   *forward*.
    /// - If the date is the last of the month and falls on a weekend, it steps
    ///   *backward*.
    /// - Saturday steps back one day (to Friday).
    /// - Sunday steps forward one day (to Monday).
    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime>;

    /// Advances `dtm` by exactly `num` business days.
    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime>;

    /// Retreats `dtm` by exactly `num` business days.
    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime>;

    /// The code of the holiday calendar this is (`GBLO`, or `GBLO+USNY` for a
    /// joint one), if it is one. A schedule document's `calendar` and a
    /// selector's definition are checked against it. `None` by default.
    fn code(&self) -> Option<String> {
        None
    }

    /// The time zone of the place whose business days these are, if it has
    /// one. A datetime schedule judges business days on its own local dates;
    /// when a run's date there is not its date in this zone (02:00 UTC is
    /// still the evening before in New York), the iterator fails with
    /// [`Error::Calendar`] rather than use the wrong
    /// day. `None` by default: no check.
    fn time_zone(&self) -> Option<chrono_tz::Tz> {
        None
    }
}

/// Direction used when searching for a nearby business day.
#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub enum Direction {
    /// Nearest business day; tie-breaking is implementation-defined.
    /// [`WeekendSkipper`] returns the date itself when it is a business day.
    /// This is the default.
    #[default]
    Nearest,
    /// The closest business day strictly *before* the given date.
    Prev,
    /// The closest business day strictly *after* the given date.
    Next,
}

/// A [`BizDayProcessor`] that considers every Monday–Friday a business day,
/// regardless of public holidays.
///
/// This is the simplest useful implementation and the one used internally by
/// the library for the `~NW` / `~PW` weekday-adjustment variants.
///
/// # Examples
///
/// ```rust
/// use tkone_schedule::biz_day::{BizDayProcessor, Direction, WeekendSkipper};
/// use chrono::{Datelike, NaiveDate};
///
/// let bdp = WeekendSkipper::new();
///
/// let sat = NaiveDate::from_ymd_opt(2024, 3, 30).unwrap().and_hms_opt(0,0,0).unwrap();
/// assert!(!bdp.is_biz_day(&sat).unwrap());
///
/// // Saturday → next business day is Monday 2024-04-01
/// let mon = bdp.find_biz_day(&sat, Direction::Next).unwrap();
/// assert_eq!(mon.weekday(), chrono::Weekday::Mon);
/// ```
#[derive(Debug, Clone, Default)]
pub struct WeekendSkipper {}

impl WeekendSkipper {
    /// Creates a new `WeekendSkipper`.
    pub fn new() -> Self {
        Self {}
    }

    fn nearest_biz_day(&self, dtm: &NaiveDateTime) -> Result<NaiveDateTime> {
        if self.is_biz_day(dtm)? {
            return Ok(*dtm);
        }

        let mut current_date = *dtm;
        let step = Duration::days(1);
        if dtm.day() == 1 {
            while current_date.weekday() == chrono::Weekday::Sat || current_date.weekday() == chrono::Weekday::Sun {
                current_date += step;
            }
            return Ok(current_date);
        }

        let last_day_month = dtm.to_last_day_of_month();
        if dtm.day() == last_day_month.day() {
            while current_date.weekday() == chrono::Weekday::Sat || current_date.weekday() == chrono::Weekday::Sun {
                current_date -= step;
            }
            return Ok(current_date);
        }

        if dtm.weekday() == chrono::Weekday::Sat {
            return Ok(current_date - step);
        }

        Ok(current_date + step)
    }
}

impl BizDayProcessor for WeekendSkipper {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool> {
        let weekday = dtm.weekday();
        Ok(weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun)
    }

    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        let mut days_added = 0;
        let mut current_date = *dtm;

        while days_added < num {
            current_date += Duration::days(1);
            if self.is_biz_day(&current_date)? {
                days_added += 1;
            }
        }

        Ok(current_date)
    }

    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        let mut days_subtracted = 0;
        let mut current_date = *dtm;

        while days_subtracted < num {
            current_date -= Duration::days(1);
            if self.is_biz_day(&current_date)? {
                days_subtracted += 1;
            }
        }

        Ok(current_date)
    }

    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime> {
        match direction {
            Direction::Nearest => self.nearest_biz_day(dtm),
            Direction::Prev => self.sub(dtm, 1),
            Direction::Next => self.add(dtm, 1),
        }
    }
}
