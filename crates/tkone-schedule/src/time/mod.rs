//! Intra-day time recurrence based on a `HH:MM:SS` spec mini-language.
//!
//! ## Entry points
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`Spec`] | Parsed representation of a time spec string |
//! | [`SpecIteratorBuilder`] | Fluent builder for [`SpecIterator`] and [`NaiveSpecIterator`] |
//! | [`SpecIterator`] | Timezone-aware fallible iterator over [`chrono::DateTime<Tz>`] |
//! | [`NaiveSpecIterator`] | Non-timezone-aware fallible iterator over [`chrono::NaiveDateTime`] |
//!
//! ## Spec Syntax
//!
//! ```text
//! <hours>:<minutes>:<seconds>
//! ```
//!
//! Each component is one of:
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `HH` / `MM` / `SS` | ForEach — drives if no `Every(n)` is present |
//! | `_` | AsIs — keep the current value unchanged |
//! | `nH` / `nM` / `nS` | Every *n* hours / minutes / seconds |
//! | `09` / `30` / `00` | At — pin to that exact value |
//!
//! **ForEach driving rule**: when no `Every` component is present the finest
//! `ForEach` field becomes `Every(1)` for its unit; coarser `ForEach` fields
//! carry their value unchanged.
//!
//! ## Examples
//!
//! ```rust
//! # use tkone_schedule::time::SpecIteratorBuilder;
//! # use chrono::{TimeZone, Utc};
//! # use fallible_iterator::FallibleIterator;
//! let start = Utc.with_ymd_and_hms(2024, 6, 1, 9, 0, 0).unwrap();
//!
//! // Every 30 minutes starting at 09:00
//! let times: Vec<_> = SpecIteratorBuilder::new_with_start("HH:30M:00", start)
//!     .build().unwrap().take(4).collect().unwrap();
//! // → 09:00, 09:30, 10:00, 10:30
//!
//! // Every hour at :00 until 13:00
//! let end = Utc.with_ymd_and_hms(2024, 6, 1, 13, 0, 0).unwrap();
//! let times: Vec<_> = SpecIteratorBuilder::new_with_start("1H:00:00", start)
//!     .with_end(end).build().unwrap().collect().unwrap();
//! // → 09:00, 10:00, 11:00, 12:00, 13:00
//! ```
//! *Run `cargo run -p tkone-schedule --example time_recurrence` for a complete program.*
#![doc = include_str!("time-spec.md")]

mod iter;
mod spec;

#[cfg(test)]
mod tests;

pub use iter::{NaiveSpecIterator, SpecIterator, SpecIteratorBuilder};

pub use spec::{Cycle, Spec};
