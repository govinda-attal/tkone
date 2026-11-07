//! Selectors: an instance's choice among its definition's slots.
//!
//! A definition [`Schedule`] generates slots. An instance's [`Selector`] has
//! rules of its own, read with the definition's time zone, business days and
//! options, and picks the definition's slots at the instants its rules give.
//! It never produces an instant the definition does not: an instant it wants
//! that the definition lacks is *missed*.
//!
//! ```rust
//! use chrono::TimeZone;
//! use chrono_tz::Europe::London;
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use tkone_schedule::schedule::{Next, Schedule};
//!
//! // Accrual batches, three a business day.
//! let start = London.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
//! let definition = Schedule::builder(start, WeekendSkipper::new())
//!     .rule("YY-MM-DD~SBT[09,13,18]:00:00")
//!     .build()?;
//!
//! // An account accrues at month end, in the evening batch.
//! let opened = London.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap();
//! let account = definition.selector(opened).rule("YY-MM-L~PBT18:00:00").build()?;
//!
//! // 31 May 2026 is a Sunday: the month end is Friday 29 May's 18:00 batch.
//! let after = London.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
//! let horizon = London.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap();
//! let Next::Due(slot) = account.next_after(&definition, &after, &horizon, |_| {})? else { panic!() };
//! assert_eq!(slot.at, London.with_ymd_and_hms(2026, 5, 29, 18, 0, 0).unwrap());
//! assert_eq!(slot.sources[0].actual, London.with_ymd_and_hms(2026, 5, 31, 18, 0, 0).unwrap());
//! # Ok::<(), tkone_schedule::Error>(())
//! ```

use std::ops::RangeInclusive;

use chrono::{DateTime, NaiveDate};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use super::{ExcludeMatch, Schedule, ScheduleBuilder, Slot, Slots};
use crate::biz_day::BizDayProcessor;
use crate::utils::just_before;
use crate::{Error, Result};

/// An instance's selection from a definition [`Schedule`]; see the
/// [module documentation](self).
///
/// Built with [`Schedule::selector`]. Its start and `until` are the
/// instance's window; a count is turned into `until` when it is built, so a
/// selector never carries one.
#[derive(Debug, Clone)]
pub struct Selector<B: BizDayProcessor> {
    schedule: Schedule<B>,
}

/// Builds a [`Selector`]; see [`Schedule::selector`].
#[derive(Debug, Clone)]
pub struct SelectorBuilder<B: BizDayProcessor>(pub(super) ScheduleBuilder<B>);

/// What [`Selector::next_after`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// The selector's next slot that the definition produces. Its sources
    /// are the selector's rules.
    Due(Slot),
    /// The selector has no more slots: its window has ended.
    Done,
    /// The selector has slots, but none the definition produces before the
    /// horizon.
    NotFound,
}

impl<B: BizDayProcessor> Schedule<B> {
    /// A builder for a selection from this schedule, anchored at `start`
    /// and read with this schedule's time zone, business days and options.
    pub fn selector(&self, start: DateTime<Tz>) -> SelectorBuilder<B> {
        let mut builder = Schedule::builder(start.with_timezone(&self.zone()), self.bdp.clone());
        builder.0.options = self.options;
        SelectorBuilder(builder)
    }
}

impl<B: BizDayProcessor> SelectorBuilder<B> {
    /// See [`ScheduleBuilder::rule`].
    pub fn rule(self, spec: impl Into<String>) -> Self {
        Self(self.0.rule(spec))
    }

    /// See [`ScheduleBuilder::named_rule`].
    pub fn named_rule(self, id: impl Into<String>, spec: impl Into<String>) -> Self {
        Self(self.0.named_rule(id, spec))
    }

    /// The end of the window: the last instant a slot can be at (inclusive).
    pub fn until(self, until: DateTime<Tz>) -> Self {
        Self(self.0.until(until))
    }

    /// Stop after `n` of the selector's own slots; see
    /// [`build`](Self::build).
    pub fn count(self, n: usize) -> Self {
        Self(self.0.count(n))
    }

    /// See [`ScheduleBuilder::exclude_date`].
    pub fn exclude_date(self, date: NaiveDate) -> Self {
        Self(self.0.exclude_date(date))
    }

    /// See [`ScheduleBuilder::exclude_rule`].
    pub fn exclude_rule(self, spec: impl Into<String>) -> Self {
        Self(self.0.exclude_rule(spec))
    }

    /// See [`ScheduleBuilder::exclude_match`].
    pub fn exclude_match(self, by: ExcludeMatch) -> Self {
        Self(self.0.exclude_match(by))
    }

    /// Check the rules and build the selector.
    ///
    /// A count becomes `until`: the instant of the selector's nth slot,
    /// counted over its own slots, not over those the definition produces.
    /// Twelve monthly accruals end at the twelfth month even if one is
    /// missed, and the count costs nothing at run time.
    ///
    /// # Errors
    ///
    /// As [`ScheduleBuilder::build`].
    pub fn build(self) -> Result<Selector<B>> {
        let mut schedule = self.0.build()?;
        if let Some(n) = schedule.count {
            // The builder refuses a count of 0.
            let nth = schedule.slots()?.nth(n - 1)?;
            schedule.count = None;
            if let Some(slot) = nth {
                schedule.until = Some(slot.at);
            }
        }
        Ok(Selector { schedule })
    }
}

impl<B: BizDayProcessor> Selector<B> {
    /// The start of the window, and the anchor of the selector's rules.
    pub fn start(&self) -> &DateTime<Tz> {
        self.schedule.start()
    }

    /// The end of the window, if any; a count is here.
    pub fn until(&self) -> Option<&DateTime<Tz>> {
        self.schedule.until()
    }

    /// The selector's own slots, whether or not a definition produces them.
    pub fn schedule(&self) -> &Schedule<B> {
        &self.schedule
    }

    /// The first slot strictly after `after`, and no later than `horizon`,
    /// that the selector picks and `definition` produces.
    ///
    /// Slots the selector wants on the way that `definition` does not
    /// produce are passed to `missed`. The horizon bounds the search: a
    /// selector whose slots the definition never produces would otherwise
    /// search for ever.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchedule`] when the selector was not built from a
    /// schedule like `definition` (another time zone, business days or
    /// options); errors from either schedule's iteration.
    pub fn next_after(
        &self,
        definition: &Schedule<B>,
        after: &DateTime<Tz>,
        horizon: &DateTime<Tz>,
        mut missed: impl FnMut(Slot),
    ) -> Result<Next> {
        let mut join = self.join(definition, after)?;
        loop {
            match join.step(horizon)? {
                Step::Picked(slot) => return Ok(Next::Due(slot)),
                Step::Missed(slot) => missed(slot),
                Step::Beyond => return Ok(Next::NotFound),
                Step::End => return Ok(Next::Done),
            }
        }
    }

    /// The selector's slots in `window` that `definition` does not produce.
    /// Empty means the selector is a subset of the definition over the window.
    ///
    /// Only a bounded window can be checked: holiday calendars are lists
    /// with an end, and some rules' cycles are millions of years long.
    ///
    /// # Errors
    ///
    /// As [`next_after`](Self::next_after).
    pub fn uncovered(&self, definition: &Schedule<B>, window: RangeInclusive<DateTime<Tz>>) -> Result<Vec<Slot>> {
        let mut join = self.join(definition, &just_before(window.start()))?;
        let mut uncovered = Vec::new();
        loop {
            match join.step(window.end())? {
                Step::Picked(_) => {}
                Step::Missed(slot) => uncovered.push(slot),
                Step::Beyond | Step::End => return Ok(uncovered),
            }
        }
    }

    fn join(&self, definition: &Schedule<B>, after: &DateTime<Tz>) -> Result<Join<B>> {
        let ours = &self.schedule;
        if ours.zone() != definition.zone()
            || ours.options != definition.options
            || ours.bdp.code() != definition.bdp.code()
        {
            return Err(Error::InvalidSchedule(
                "the selector was built for a definition with another time zone, calendar or options".into(),
            ));
        }
        Ok(Join {
            selection: ours.slots_after(after)?,
            produced: definition.slots_after(after)?,
            head: None,
            produced_done: false,
        })
    }
}

/// The selector's slots, each matched against the definition's.
struct Join<B: BizDayProcessor> {
    selection: Slots<B>,
    produced: Slots<B>,
    /// The definition's first slot not before the last selected one.
    head: Option<Slot>,
    produced_done: bool,
}

enum Step {
    Picked(Slot),
    Missed(Slot),
    /// The selector's next slot is after the horizon.
    Beyond,
    /// The selector has no more slots.
    End,
}

impl<B: BizDayProcessor> Join<B> {
    fn step(&mut self, horizon: &DateTime<Tz>) -> Result<Step> {
        let Some(slot) = self.selection.next()? else {
            return Ok(Step::End);
        };
        if slot.at > *horizon {
            return Ok(Step::Beyond);
        }
        loop {
            if self.head.is_none() && !self.produced_done {
                self.head = self.produced.next()?;
                self.produced_done = self.head.is_none();
            }
            match &self.head {
                Some(produced) if produced.at < slot.at => self.head = None,
                Some(produced) if produced.at == slot.at => return Ok(Step::Picked(slot)),
                _ => return Ok(Step::Missed(slot)),
            }
        }
    }
}
