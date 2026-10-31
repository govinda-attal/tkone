//! `with_count`, shared by the three builders: stop after *n* results,
//! counted from the start. Excluded occurrences (`!`, or `with_exclusions`)
//! are not counted.
//!
//! A counted schedule is resumed by replaying it from its stored start, which
//! is cheap because it has at most *n* results, and keeps the count exact.

use chrono::{DateTime, FixedOffset};
use fallible_iterator::FallibleIterator;

use crate::prelude::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct Limits {
    /// The number given to `with_count`.
    pub(crate) count: Option<usize>,
    /// The stored start given to `resume_with_start`.
    pub(crate) resume_start: Option<DateTime<FixedOffset>>,
}

impl Limits {
    /// How to build a resumed (`local` is the stored cursor) counted
    /// schedule: `Some(start)` to replay it from its start, or an error when
    /// the start is unknown.
    pub(crate) fn replay_from<T>(&self, local: Option<T>) -> Result<Option<DateTime<FixedOffset>>> {
        match (self.count, local, self.resume_start) {
            (Some(_), Some(_), None) => Err(Error::UnsupportedResume(
                "a schedule with `with_count` must be resumed with `resume_with_start`".into(),
            )),
            (Some(_), Some(_), start) => Ok(start),
            _ => Ok(None),
        }
    }
}

/// Skip the results of `iter` whose `key` is not after `last`, without
/// consuming the first one that is.
pub(crate) fn skip_through<I, K>(mut iter: I, last: &K, key: fn(&I::Item) -> K) -> Result<I>
where
    I: FallibleIterator<Error = Error> + Clone,
    K: Ord,
{
    loop {
        let mut probe = iter.clone();
        match probe.next()? {
            Some(item) if key(&item) <= *last => iter = probe,
            _ => return Ok(iter),
        }
    }
}
