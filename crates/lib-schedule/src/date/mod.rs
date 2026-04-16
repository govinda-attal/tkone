//! Calendar-day recurrence based on a flexible date spec mini-language.
//!
//! ## Entry points
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`Spec`] | Parsed representation of a date spec string |
//! | [`SpecIteratorBuilder`] | Fluent builder for [`SpecIterator`] and [`NaiveSpecIterator`] |
//! | [`SpecIterator`] | Timezone-aware fallible iterator over [`crate::NextResult<DateTime<Tz>>`] |
//! | [`NaiveSpecIterator`] | Non-timezone-aware fallible iterator over [`crate::NextResult<NaiveDateTime>`] |
//!
//! ## Spec Syntax
//!
//! ```text
//! <years>-<months>-<days>[~<adj>]
//! ```
//!
//! See the [crate-level documentation](crate) for a full syntax reference.
//!
//! ## Common Patterns
//!
//! ```rust
//! # use lib_schedule::biz_day::WeekendSkipper;
//! # use lib_schedule::date::SpecIteratorBuilder;
//! # use chrono::{TimeZone, Utc};
//! # use fallible_iterator::FallibleIterator;
//! # let bdp = WeekendSkipper::new();
//! // Last day of every month
//! let start = Utc.with_ymd_and_hms(2024, 1, 31, 0, 0, 0).unwrap();
//! let dates: Vec<_> = SpecIteratorBuilder::new_with_start("YY-1M-L", bdp.clone(), start)
//!     .build().unwrap().take(3).collect().unwrap();
//! // → 2024-01-31, 2024-02-29, 2024-03-31
//!
//! // Every Friday
//! let start = Utc.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap();
//! # let _: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-FRI", bdp.clone(), start)
//! #     .build().unwrap().take(4).collect().unwrap();
//!
//! // Bounded: last biz-day of each month until end of year
//! let end = Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap();
//! # let _: Vec<_> = SpecIteratorBuilder::new_with_start("YY-1M-L~W", bdp, start)
//! #     .with_end(end).build().unwrap().collect().unwrap();
//! ```
//! *Run `cargo run -p lib-schedule --example date_recurrence` for a complete program.*

mod component;
mod iter;
mod spec;

#[cfg(test)]
mod tests;

pub use iter::{NaiveSpecIterator, SpecIterator, SpecIteratorBuilder};

pub use spec::{
    parse_spec, BizDayAdjustment, Cycle, DayCycle, LastDayOption, Spec, WeekdayOption,
};
