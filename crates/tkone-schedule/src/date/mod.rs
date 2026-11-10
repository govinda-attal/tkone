//! Calendar-day recurrence based on a flexible date spec mini-language.
//!
//! ## Entry points
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`Spec`](crate::date::Spec) | Parsed representation of a date spec string |
//! | [`SpecIteratorBuilder`](crate::date::SpecIteratorBuilder) | Fluent builder for [`SpecIterator`](crate::date::SpecIterator) and [`NaiveSpecIterator`](crate::date::NaiveSpecIterator) |
//! | [`SpecIterator`](crate::date::SpecIterator) | Timezone-aware fallible iterator over [`crate::Occurrence<DateTime<Tz>>`] |
//! | [`NaiveSpecIterator`](crate::date::NaiveSpecIterator) | Non-timezone-aware fallible iterator over [`crate::Occurrence<NaiveDateTime>`] |
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
//! # use tkone_schedule::biz_day::WeekendSkipper;
//! # use tkone_schedule::date::SpecIteratorBuilder;
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
//! let _: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-FRI", bdp.clone(), start)
//!     .build().unwrap().take(4).collect().unwrap();
//! // → 2025-01-03, 2025-01-10, 2025-01-17, 2025-01-24
//!
//! // Bounded: last nearest weekday of each month until end of year
//! let end = Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap();
//! let _: Vec<_> = SpecIteratorBuilder::new_with_start("YY-1M-L~W", bdp, start)
//!     .with_end(end).build().unwrap().collect().unwrap();
//! ```
//! *Run `cargo run -p tkone-schedule --example date_recurrence` for a complete program.*
#![doc = include_str!("date-spec.md")]

mod describe;
mod easter;
mod engine;
mod iter;
mod spec;

#[cfg(test)]
mod tests;

pub(crate) use iter::{needs_start, resume_needs_start};
pub use iter::{NaiveSpecIterator, SpecIterator, SpecIteratorBuilder};

/// Describes a date spec in plain English: [`Spec::describe`] on the parsed
/// spec.
///
/// ```rust
/// assert_eq!(
///     tkone_schedule::date::describe("YY-MM-L~PB").unwrap(),
///     "on the last day of every month, moved to the previous business day when it is not one"
/// );
/// ```
///
/// # Errors
///
/// Returns an error if the spec does not parse.
pub fn describe(spec: &str) -> crate::Result<String> {
    Ok(spec.parse::<Spec>()?.describe())
}

pub use easter::Easter;

/// `spec`'s adjustment applied to one date-time, as the iterators apply it;
/// `None` when `~SB` drops it. For rules derived from another rule's slots.
pub(crate) fn adjust_one<B: crate::biz_day::BizDayProcessor>(
    spec: &Spec,
    dtm: chrono::NaiveDateTime,
    bdp: &B,
) -> crate::Result<Option<crate::Occurrence<chrono::NaiveDateTime>>> {
    let ctx = engine::IterContext { anchor: dtm, bd_processor: bdp.clone() };
    let raw = crate::Occurrence::Exact(dtm);
    if engine::skipped(spec, &raw, &ctx)? {
        return Ok(None);
    }
    engine::adjust(spec, raw, &ctx).map(Some)
}
pub use spec::{
    parse_spec, BizDayAdjustment, BizDayCalendar, Cycle, DayCycle, LastDayOption, NextNthDayOption, Spec,
    WeekdayOption, EASTER_OFFSETS,
};
