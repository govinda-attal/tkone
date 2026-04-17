#![feature(btree_cursors)]

//! # lib-schedule
//!
//! A scheduling and recurrence library built on flexible mini-language specs for dates,
//! times, and combined datetimes. Supports business day processing, timezone awareness,
//! and fallible iteration.
//!
//! ## Core Concepts
//!
//! The library is organised around three independent spec types, each with a
//! corresponding recurrence iterator:
//!
//! | Module | Spec type | Iterator item | Use when… |
//! |--------|-----------|---------------|-----------|
//! | [`date`] | `"YY-1M-31L"` | `NextResult<DateTime<Tz>>` | calendar-day recurrence |
//! | [`time`] | `"1H:00:00"` | `DateTime<Tz>` | intra-day time recurrence |
//! | [`datetime`] | `"YY-1M-31L~NBT11:00:00"` | `NextResult<DateTime<Tz>>` | combined date + time |
//!
//! ## Quick Start
//!
//! ### 1 — Find the first matching date, then iterate from it
//!
//! The most common pattern is to derive a concrete *start datetime* from the spec
//! itself — i.e. "what is the very next occurrence?" — and then hand that datetime
//! back to the iterator so it becomes the first item of the series.
//!
//! ```rust
//! # use lib_schedule::biz_day::WeekendSkipper;
//! # use lib_schedule::date::SpecIteratorBuilder;
//! # use chrono::{SubsecRound, Utc};
//! # use chrono_tz::America::New_York;
//! # use fallible_iterator::FallibleIterator;
//! # let bdp = WeekendSkipper::new();
//! # let now = Utc::now().with_timezone(&New_York).trunc_subsecs(0);
//! // Step 1: find the first occurrence strictly after now
//! let start = SpecIteratorBuilder::new_after("YY-1M-L", bdp.clone(), now)
//!     .build().unwrap().next().unwrap().unwrap()
//!     .observed().clone();
//!
//! // Step 2: iterate from that start date (inclusive)
//! let iter = SpecIteratorBuilder::new_with_start("YY-1M-L", bdp, start)
//!     .build().unwrap();
//!
//! # let dates: Vec<_> = iter.take(3).collect().unwrap();
//! // r.observed() → settlement date   r.actual() → raw calendar date
//! ```
//! *Run `cargo run -p lib-schedule --example date_recurrence` for the full program.*
//!
//! ### 2 — Combined date + time recurrence
//!
//! Append `T<time_spec>` to a date spec to create a [`datetime`] schedule.
//! The iterator visits each valid calendar date in order and emits every
//! matching time within that day before advancing.
//!
//! ```rust
//! # use lib_schedule::biz_day::WeekendSkipper;
//! # use lib_schedule::datetime::SpecIteratorBuilder;
//! # use chrono::{SubsecRound, Utc};
//! # use chrono_tz::Europe::London;
//! # use fallible_iterator::FallibleIterator;
//! # let bdp = WeekendSkipper::new();
//! # let now = Utc::now().with_timezone(&London).trunc_subsecs(0);
//! // "~NB" = shift to next business day if the date falls on a weekend
//! let start = SpecIteratorBuilder::new_after("YY-1M-L~NBT11:00:00", bdp.clone(), now)
//!     .build().unwrap().next().unwrap().unwrap()
//!     .observed().clone();
//!
//! let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~NBT11:00:00", bdp, start)
//!     .build().unwrap();
//!
//! # let _: Vec<_> = iter.take(4).collect().unwrap();
//! ```
//! *Run `cargo run -p lib-schedule --example datetime_recurrence` for the full program.*
//!
//! ### 3 — Time-only recurrence
//!
//! ```rust
//! # use lib_schedule::time::SpecIteratorBuilder;
//! # use chrono::{TimeZone, Utc};
//! # use fallible_iterator::FallibleIterator;
//! # let start = Utc.with_ymd_and_hms(2024, 6, 1, 9, 0, 0).unwrap();
//! // Every 30 minutes starting at 09:00
//! let iter = SpecIteratorBuilder::new_with_start("HH:30M:00", start).build().unwrap();
//! # let _: Vec<_> = iter.take(4).collect().unwrap();
//! // → 09:00, 09:30, 10:00, 10:30
//! ```
//! *Run `cargo run -p lib-schedule --example time_recurrence` for the full program.*
//!
//! ## Running the Bundled Examples
//!
//! The crate ships runnable examples under `examples/`. Run any of them with:
//!
//! ```text
//! cargo run -p lib-schedule --example date_recurrence
//! cargo run -p lib-schedule --example datetime_recurrence
//! cargo run -p lib-schedule --example time_recurrence
//! ```
//!
//! ## Spec Syntax Reference
//!
//! ### Date Spec — `<years>-<months>-<days>[~<adj>]`
//!
//! #### Years
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `YY`  | Every calendar year |
//! | `_`   | Keep current year (no-op) |
//! | `nY`  | Every *n* years, aligned to the iterator start date |
//! | `2025` | Exactly year 2025 |
//! | `[2024,2025]` | Years 2024 or 2025 |
//!
//! #### Months
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `MM`  | Every calendar month |
//! | `_`   | Keep current month (no-op) |
//! | `nM`  | Every *n* months, aligned to the iterator start date |
//! | `03`  | March only |
//! | `[01,06,12]` | January, June, or December |
//!
//! #### Days
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `DD` or `_` | Every calendar day |
//! | `15`  | 15th of the month |
//! | `[5,10,15]` | 5th, 10th, and 15th |
//! | `L`   | Last day of the month |
//! | `31L` | 31st, or last day if the month is shorter |
//! | `31N` | 31st, or 1st of next month if the month is shorter |
//! | `31O` | 31st, or overflow remainder days into next month (e.g. → Mar 3 when Feb has 28 days) |
//! | `nD`  | Advance *n* calendar days |
//! | `nBD` | Advance *n* business days |
//! | `nWD` | Advance *n* weekdays (Mon–Fri) |
//! | `MON` / `TUE` / … | Every occurrence of that weekday in the month |
//! | `[MON,FRI]` | Every Monday and Friday |
//! | `WED#2` | 2nd Wednesday of the month |
//! | `FRI#L` | Last Friday of the month |
//! | `THU#2L` | 2nd-to-last Thursday of the month |
//!
//! #### Business Day Adjustment (`~`)
//!
//! Applied after the raw calendar date is resolved. Directional variants
//! (`~NB`, `~PB`, `~B`, `~NW`, `~PW`, `~W`) are **conditional** — they only shift
//! when the raw date is not already a business/week day. Numeric variants
//! (`~nP`, `~nN`) are **unconditional** offsets.
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `~NB` | Next business day (shift forward if not already a business day) |
//! | `~PB` | Previous business day (shift back if not already a business day) |
//! | `~B`  | Nearest business day (shift to whichever direction is closer) |
//! | `~NW` | Next weekday — Mon–Fri (shift forward if not already a weekday) |
//! | `~PW` | Previous weekday — Mon–Fri (shift back if not already a weekday) |
//! | `~W`  | Nearest weekday — Mon–Fri (shift to whichever direction is closer) |
//! | `~3P` | 3 business days earlier (unconditional) |
//! | `~2N` | 2 business days later (unconditional) |
//!
//! ### Time Spec — `<hours>:<minutes>:<seconds>`
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `HH`  | ForEach — drives if no `Every` is present |
//! | `_`   | AsIs — keep current hour |
//! | `nH`  | Every *n* hours |
//! | `09`  | At hour 9 (two-digit) |
//! | `MM`  | ForEach minute |
//! | `_`   | AsIs minute |
//! | `nM`  | Every *n* minutes |
//! | `30`  | At minute 30 |
//! | `SS`  | ForEach second |
//! | `_`   | AsIs second |
//! | `nS`  | Every *n* seconds |
//! | `00`  | At second 0 |
//!
//! **ForEach driving rule**: when no `Every` component exists, the finest
//! `ForEach` field becomes `Every(1)` for its unit; coarser `ForEach` fields
//! carry their current value unchanged.
//!
//! ### DateTime Spec — `<date_spec>T<time_spec>`
//!
//! The `T` separator is detected by the pattern that follows it (`HH:`, `nH:`,
//! or a two-digit clock hour `dd:`), so weekday tokens like `TUE` and `THU`
//! in the date spec are not confused with the separator.
//!
//! ```text
//! "YY-1M-31L~NBT11:00:00"  →  date="YY-1M-31L~NB"   time="11:00:00"
//! "YY-MM-DDT1H:00:00"      →  date="YY-MM-DD"         time="1H:00:00"
//! "YY-MM-THUT09:30:00"     →  date="YY-MM-THU"         time="09:30:00"
//! ```
//!
//! ## `NextResult` and Business Day Adjustments
//!
//! Date and datetime iterators yield [`NextResult<T>`] rather than plain `T`.
//! This distinguishes unadjusted occurrences from ones where the business day
//! rule moved the settlement date:
//!
//! - [`NextResult::Single`] — no adjustment; `actual == observed`.
//! - [`NextResult::AdjustedEarlier`] — rule moved the date *earlier*.
//! - [`NextResult::AdjustedLater`] — rule moved the date *later*.
//!
//! Use `.observed()` for the settlement date and `.actual()` for the raw
//! calendar date.

/// The `biz_day` module contains the [`biz_day::BizDayProcessor`] trait and
/// built-in implementations for business day calculations.
pub mod biz_day;
/// The `date` module provides calendar-day recurrence via [`date::Spec`] and
/// [`date::SpecIteratorBuilder`].
pub mod date;
/// The `datetime` module combines a date spec and a time spec into a single
/// recurrence schedule via [`datetime::Spec`] and [`datetime::SpecIteratorBuilder`].
pub mod datetime;
/// The `time` module provides intra-day time recurrence via [`time::Spec`] and
/// [`time::SpecIteratorBuilder`].
pub mod time;

mod error;
mod prelude;
mod utils;

pub use error::{Error, Result};

/// Controls how timezone-aware iterators resolve local datetimes that fall in
/// a DST transition window.
///
/// Applies to [`date::SpecIterator`], [`time::SpecIterator`], and
/// [`datetime::SpecIterator`] whenever a naive datetime produced by the
/// schedule spec must be mapped to an unambiguous `DateTime<Tz>`.
///
/// Configure via `.with_dst_policy(…)` on any [`SpecIteratorBuilder`](date::SpecIteratorBuilder).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DstPolicy {
    /// Silently resolve DST edge cases — the default.
    ///
    /// - **Spring-forward gap** (e.g. `02:30` does not exist): advance one
    ///   hour and take the latest UTC offset.
    /// - **Fall-back overlap** (e.g. `01:30` is ambiguous): take the earliest
    ///   UTC offset (still in summer time).
    #[default]
    Adjust,
    /// Return [`Error::AmbiguousLocalTime`] instead of silently resolving.
    Strict,
}

/// Outcome of a single scheduling step, distinguishing raw calendar dates from
/// business-day-adjusted settlement dates.
///
/// # Variants
///
/// | Variant | Condition | `actual()` | `observed()` |
/// |---------|-----------|------------|--------------|
/// | `Single(t)` | No adjustment applied | `t` | `t` |
/// | `AdjustedEarlier(a, o)` | Settlement moved earlier | `a` | `o` (`o < a`) |
/// | `AdjustedLater(a, o)` | Settlement moved later | `a` | `o` (`o > a`) |
///
/// # Examples
///
/// ```rust
/// use lib_schedule::NextResult;
/// use chrono::{NaiveDate, NaiveDateTime};
///
/// let actual   = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap().and_hms_opt(0,0,0).unwrap();
/// let observed = NaiveDate::from_ymd_opt(2024, 3, 29).unwrap().and_hms_opt(0,0,0).unwrap(); // Fri
///
/// let result = NextResult::AdjustedEarlier(actual, observed);
/// assert_eq!(result.actual(),   &actual);
/// assert_eq!(result.observed(), &observed);
/// assert!(result.observed() < result.actual());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum NextResult<T: Clone> {
    /// No business day adjustment was necessary; the raw calendar date is the
    /// settlement date.
    Single(T),
    /// The business day rule shifted the settlement date *later* than the raw
    /// calendar date. The first field is the raw date; the second is the
    /// settlement date.
    AdjustedLater(T, T),
    /// The business day rule shifted the settlement date *earlier* than the raw
    /// calendar date. The first field is the raw date; the second is the
    /// settlement date.
    AdjustedEarlier(T, T),
}

impl<T: Clone> NextResult<T> {
    /// Returns the inner value if this is a [`NextResult::Single`], otherwise `None`.
    pub fn single(self) -> Option<T> {
        match self {
            NextResult::Single(t) => Some(t),
            _ => None,
        }
    }

    /// The chronologically earlier of the two dates.
    ///
    /// - `Single(t)` → `t`
    /// - `AdjustedEarlier(a, o)` → `o` (settlement is earlier than raw)
    /// - `AdjustedLater(a, o)` → `a` (raw is earlier than settlement)
    pub fn earlier(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(_, t)
            | NextResult::AdjustedLater(t, _) => t,
        }
    }

    /// The chronologically later of the two dates.
    ///
    /// - `Single(t)` → `t`
    /// - `AdjustedEarlier(a, o)` → `a` (raw is later than settlement)
    /// - `AdjustedLater(a, o)` → `o` (settlement is later than raw)
    pub fn later(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(t, _)
            | NextResult::AdjustedLater(_, t) => t,
        }
    }

    /// The settlement (post-adjustment) date.
    ///
    /// This is the date on which the event is *observed* — i.e. the result of
    /// applying any business day rule. Equal to `actual()` when no adjustment
    /// was made ([`NextResult::Single`]).
    pub fn observed(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(_, t)
            | NextResult::AdjustedLater(_, t) => t,
        }
    }

    /// The raw calendar date before any business day adjustment.
    ///
    /// Equal to `observed()` when no adjustment was made ([`NextResult::Single`]).
    pub fn actual(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(t, _)
            | NextResult::AdjustedLater(t, _) => t,
        }
    }

    /// Returns `(actual, observed)` as a tuple.
    ///
    /// For [`NextResult::Single`] both elements are the same reference.
    pub fn as_tuple(&self) -> (&T, &T) {
        match self {
            NextResult::Single(t) => (t, t),
            NextResult::AdjustedEarlier(actual, adjusted)
            | NextResult::AdjustedLater(actual, adjusted) => (actual, adjusted),
        }
    }
}
