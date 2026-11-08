//! Periods: the time between one slot and the next.
//!
//! Interest accrues between payments, so a loan is computed period by
//! period, not instant by instant. A period's boundaries carry both times:
//! when the slot happens (`at`, adjusted) and when its rule put it
//! (`actual`, unadjusted). Contracts differ on which one accrual counts, so
//! both are given and the choice stays with the caller, as does the day
//! count.
//!
//! ```rust
//! use chrono::TimeZone;
//! use chrono_tz::Europe::London;
//! use fallible_iterator::FallibleIterator;
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use tkone_schedule::schedule::Schedule;
//!
//! // A loan drawn on 15 January 2026, repaid at each month end, moved to
//! // the previous business day.
//! let drawn = London.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
//! let repayments = Schedule::builder(drawn, WeekendSkipper::new()).rule("YY-MM-L~PBT00:00:00").count(3).build()?;
//!
//! // Count days between dates, not instants: the clocks go forward on
//! // 29 March, so midnight to midnight across it is 30 days and 23 hours.
//! let days = |a: chrono::DateTime<chrono_tz::Tz>, b: chrono::DateTime<chrono_tz::Tz>| {
//!     (b.date_naive() - a.date_naive()).num_days()
//! };
//! let days: Vec<_> = repayments
//!     .periods()?
//!     .map(|p| Ok((days(p.start.at, p.end.at), days(p.start.actual, p.end.actual))))
//!     .collect()?;
//! // 31 January 2026 is a Saturday: paid on Friday the 30th.
//! // Adjusted: 15 → 30 Jan → 27 Feb → 31 Mar. Unadjusted: 15 → 31 Jan → 28 Feb → 31 Mar.
//! assert_eq!(days, [(15, 16), (28, 28), (32, 31)]);
//! # Ok::<(), tkone_schedule::Error>(())
//! ```

use std::ops::RangeInclusive;

use chrono::DateTime;
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use super::{Schedule, Slot, Slots};
use crate::biz_day::BizDayProcessor;
use crate::Result;

/// One end of a [`Period`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boundary {
    /// When it happens: a slot's adjusted instant, or the schedule's start.
    pub at: DateTime<Tz>,
    /// When its rule put it, before adjustment: the earliest actual time
    /// among the slot's sources (with one rule, that rule's time), or the
    /// schedule's start.
    pub actual: DateTime<Tz>,
}

impl Boundary {
    fn of(slot: &Slot) -> Self {
        let actual = slot.sources.iter().map(|s| s.actual).min().unwrap_or(slot.at);
        Self { at: slot.at, actual }
    }
}

/// The time from one boundary to the next slot.
///
/// The first period starts at the schedule's start; each later one at the
/// slot that ended the one before. A slot at the start itself ends no
/// period: it starts the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period {
    /// Where the period starts.
    pub start: Boundary,
    /// Where it ends: [`slot`](Self::slot)'s boundary.
    pub end: Boundary,
    /// The slot that ends it, with the rules that produced it.
    pub slot: Slot,
}

/// The periods of a [`Schedule`], in time order; see
/// [`Schedule::periods`].
#[derive(Debug)]
pub struct Periods<B: BizDayProcessor> {
    slots: Slots<B>,
    start: Boundary,
    /// Periods ending before this are passed over.
    from: Option<DateTime<Tz>>,
}

impl<B: BizDayProcessor> Schedule<B> {
    /// Every period, from the schedule's start: see the
    /// [module documentation](crate::schedule::period).
    ///
    /// Ends when the slots do; on a schedule with no `until` or count, use
    /// [`periods_between`](Self::periods_between).
    ///
    /// # Errors
    ///
    /// As [`slots`](Self::slots).
    pub fn periods(&self) -> Result<Periods<B>> {
        Ok(Periods { slots: self.slots()?, start: self.opening(), from: None })
    }

    /// The periods that end in `range`, both ends included. The first one
    /// starts at the slot before it, or the schedule's start: the same
    /// periods as [`periods`](Self::periods) gives there.
    ///
    /// Slots are read from the schedule's start, to find where the first
    /// period starts.
    ///
    /// # Errors
    ///
    /// As [`slots_between`](Self::slots_between).
    pub fn periods_between(&self, range: RangeInclusive<DateTime<Tz>>) -> Result<Periods<B>> {
        let (from, to) = range.into_inner();
        let slots = self.slots_between(*self.start()..=to)?;
        Ok(Periods { slots, start: self.opening(), from: Some(from) })
    }

    fn opening(&self) -> Boundary {
        Boundary { at: *self.start(), actual: *self.start() }
    }
}

impl<B: BizDayProcessor> FallibleIterator for Periods<B> {
    type Item = Period;
    type Error = crate::Error;

    fn next(&mut self) -> Result<Option<Period>> {
        while let Some(slot) = self.slots.next()? {
            let end = Boundary::of(&slot);
            let start = std::mem::replace(&mut self.start, end);
            if slot.at == start.at || self.from.is_some_and(|from| slot.at < from) {
                continue;
            }
            return Ok(Some(Period { start, end, slot }));
        }
        Ok(None)
    }
}
