//! # time
//! Provides an recurrence based iterators for both Naive and timezone-aware datetimes.
//! These iterators are instantiated with a time based recurrence specification.

mod iter;
mod spec;

pub use iter::{NaiveSpecIterator, SpecIterator};

pub use spec::{Cycle, Spec, SPEC_EXPR};
