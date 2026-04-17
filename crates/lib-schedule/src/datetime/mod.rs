//! Combined date + time recurrence.
//!
//! [`Spec`] combines a [`crate::date::Spec`] and a [`crate::time::Spec`] with
//! a `T` separator into a single schedule string. The corresponding iterator
//! visits each valid calendar date in order and, within that date, emits every
//! time that matches the time spec — a *date-first* strategy.
//!
//! ## Spec format
//!
//! ```text
//! <date_spec>T<time_spec>
//! ```
//!
//! The `T` separator is recognised by the pattern that **immediately follows**
//! it (`HH:`, `nH:`, or a two-digit clock hour `dd:`), so weekday tokens such
//! as `TUE` and `THU` in the date part are never misidentified as separators.
//!
//! ## Examples
//!
//! | Spec string | Meaning |
//! |-------------|---------|
//! | `"YY-1M-31L~WT11:00:00"` | Last day of every month adjusted to nearest weekday at 11:00 |
//! | `"YY-MM-DDTHH:30M:00"` | Every day, every 30 minutes |
//! | `"YY-MM-FRI#LT16:30:00"` | Last Friday of each month at 16:30 |
//! | `"YY-MM-THUT09:30:00"` | Every Thursday at 09:30 |
//!
//! ## Quick start
//!
//! ```rust
//! # use lib_schedule::biz_day::WeekendSkipper;
//! # use lib_schedule::datetime::SpecIteratorBuilder;
//! # use chrono::{SubsecRound, Utc};
//! # use chrono_tz::Europe::London;
//! # use fallible_iterator::FallibleIterator;
//! # let bdp = WeekendSkipper::new();
//! # let now = Utc::now().with_timezone(&London).trunc_subsecs(0);
//! // Step 1: find the first occurrence
//! let start = SpecIteratorBuilder::new_after("YY-1M-L~WT11:00:00", bdp.clone(), now)
//!     .build().unwrap().next().unwrap().unwrap()
//!     .observed().clone();
//!
//! // Step 2: iterate from that start date
//! let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~WT11:00:00", bdp, start)
//!     .build().unwrap();
//!
//! let _: Vec<_> = iter.take(3).collect().unwrap();
//! ```
//! *Run `cargo run -p lib-schedule --example datetime_recurrence` for a complete program.*
#![doc = include_str!("date-time-spec.md")]

mod iter;
mod spec;

#[cfg(test)]
mod tests;

pub use iter::{NaiveSpecIterator, SpecIterator, SpecIteratorBuilder};
pub use spec::Spec;
