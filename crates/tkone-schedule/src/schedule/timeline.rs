//! Timelines: several schedules read as one, moment by moment.
//!
//! A [`Timeline`] holds schedules, each under a key of the caller's
//! choosing: a loan's rate resets and its repayments, an account's daily
//! accrual and its month-end capitalisation. Each keeps its own start, time
//! zone, calendar, exclusions and end. Read over a range, the timeline
//! yields [`Moment`]s in time order: one instant with every schedule that
//! has a slot at it, in the order the schedules were added. Code that steps
//! through the timeline processes them in that order: whether the day's
//! accrual comes before the month's capitalisation is declared, not left to
//! chance.
//!
//! That order is for whoever steps through the timeline. A scheduler does
//! not keep it: tkone-trigger runs one tick's jobs concurrently, and tempo
//! runs each document's slots as runs of their own. Steps that must run in
//! order at one instant belong in one schedule, as rules in that order: they
//! are then one slot, whose sources are in rule order, run once.
//!
//! Each [`Entry`] also gives [`since`](Entry::since), the boundary of its
//! schedule's slot before it (or the schedule's start): the period it
//! closes, as [`Schedule::periods`] would give it.
//!
//! `docs/timeline.md` in the repository describes the idea with examples.
//!
//! ```rust
//! use chrono::TimeZone;
//! use chrono_tz::Europe::London;
//! use fallible_iterator::FallibleIterator;
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use tkone_schedule::schedule::{Schedule, Timeline};
//!
//! let open = London.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
//! let accrue = Schedule::builder(open, WeekendSkipper::new()).rule("YY-MM-DD~SBT18:00:00").build()?;
//! let capitalise = Schedule::builder(open, WeekendSkipper::new()).rule("YY-MM-L~PBT18:00:00").build()?;
//!
//! let from = London.with_ymd_and_hms(2026, 5, 28, 0, 0, 0).unwrap();
//! let to = London.with_ymd_and_hms(2026, 6, 1, 23, 0, 0).unwrap();
//! let moments: Vec<_> = Timeline::new()
//!     .add("accrue", accrue)
//!     .add("capitalise", capitalise)
//!     .between(from..=to)?
//!     .map(|m| Ok((m.at.format("%a %d").to_string(), m.entries.iter().map(|e| e.key).collect::<Vec<_>>())))
//!     .collect()?;
//! // 31 May 2026 is a Sunday: the month end is Friday the 29th, after that
//! // day's accrual.
//! assert_eq!(moments, [
//!     ("Thu 28".to_string(), vec!["accrue"]),
//!     ("Fri 29".to_string(), vec!["accrue", "capitalise"]),
//!     ("Mon 01".to_string(), vec!["accrue"]),
//! ]);
//! # Ok::<(), tkone_schedule::Error>(())
//! ```

use std::fmt::Debug;
use std::ops::RangeInclusive;

use chrono::DateTime;
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use super::{Boundary, Schedule, Slot, Slots};
use crate::biz_day::BizDayProcessor;
use crate::{Error, Result};

/// Schedules under keys, read together; see the
/// [module documentation](crate::schedule::timeline).
#[derive(Debug, Clone)]
pub struct Timeline<K, B: BizDayProcessor> {
    schedules: Vec<(K, Schedule<B>)>,
}

/// One instant on a [`Timeline`], with every schedule that has a slot at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moment<K> {
    /// The instant, in the time zone of the first entry's schedule.
    pub at: DateTime<Tz>,
    /// The schedules' slots at it, in the order they were added. Never empty.
    pub entries: Vec<Entry<K>>,
}

/// One schedule's slot in a [`Moment`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<K> {
    /// The key the schedule was added under.
    pub key: K,
    /// The slot, in its schedule's time zone, with the rules that produced it.
    pub slot: Slot,
    /// The schedule's slot before this one, or the schedule's start for its
    /// first: the start of the period this slot ends. A slot at the start
    /// itself has its own instant here.
    pub since: Boundary,
}

impl<K, B: BizDayProcessor> Default for Timeline<K, B> {
    fn default() -> Self {
        Self { schedules: Vec::new() }
    }
}

impl<K: Clone + PartialEq + Debug, B: BizDayProcessor> Timeline<K, B> {
    /// An empty timeline.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `schedule` under `key`. Within a moment, entries come in the
    /// order their schedules were added.
    pub fn add(mut self, key: K, schedule: Schedule<B>) -> Self {
        self.schedules.push((key, schedule));
        self
    }

    /// The keys, in the order they were added.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.schedules.iter().map(|(key, _)| key)
    }

    /// The moments in `range`, both ends included. Always ends.
    ///
    /// Each schedule is read from its own start, so that the first entry's
    /// [`since`](Entry::since) is right.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchedule`] when there are no schedules or a key is
    /// used twice; errors from the schedules' iteration.
    pub fn between(&self, range: RangeInclusive<DateTime<Tz>>) -> Result<Moments<K, B>> {
        if self.schedules.is_empty() {
            return Err(Error::InvalidSchedule("a timeline needs at least one schedule".into()));
        }
        for (n, (key, _)) in self.schedules.iter().enumerate() {
            if self.schedules[..n].iter().any(|(other, _)| other == key) {
                return Err(Error::InvalidSchedule(format!("the key {key:?} is used twice")));
            }
        }
        let (from, to) = range.into_inner();
        let lanes = self
            .schedules
            .iter()
            .map(|(key, schedule)| {
                let start = *schedule.start();
                let slots = if to < start { None } else { Some(schedule.slots_between(start..=to)?) };
                Ok(Lane { key: key.clone(), slots, since: Boundary { at: start, actual: start }, head: None })
            })
            .collect::<Result<_>>()?;
        Ok(Moments { lanes, from })
    }
}

/// The moments of a [`Timeline`], in time order; see
/// [`Timeline::between`].
#[derive(Debug)]
pub struct Moments<K, B: BizDayProcessor> {
    lanes: Vec<Lane<K, B>>,
    /// Moments before this are passed over.
    from: DateTime<Tz>,
}

/// One schedule's slots, with the one read ahead.
#[derive(Debug)]
struct Lane<K, B: BizDayProcessor> {
    key: K,
    /// `None` once the schedule has no more slots in the range.
    slots: Option<Slots<B>>,
    /// The boundary before `head`.
    since: Boundary,
    head: Option<Slot>,
}

impl<K, B: BizDayProcessor> Lane<K, B> {
    /// Reads the next slot into `head`, unless one is waiting.
    fn fill(&mut self) -> Result<()> {
        if self.head.is_none() {
            if let Some(slots) = &mut self.slots {
                self.head = slots.next()?;
                if self.head.is_none() {
                    self.slots = None;
                }
            }
        }
        Ok(())
    }

    /// Takes `head` as an entry, and moves `since` past it.
    fn take(&mut self) -> Option<Entry<K>>
    where
        K: Clone,
    {
        let slot = self.head.take()?;
        let since = std::mem::replace(&mut self.since, Boundary::of(&slot));
        Some(Entry { key: self.key.clone(), slot, since })
    }
}

impl<K: Clone, B: BizDayProcessor> FallibleIterator for Moments<K, B> {
    type Item = Moment<K>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Moment<K>>> {
        loop {
            for lane in &mut self.lanes {
                lane.fill()?;
            }
            let Some(at) = self.lanes.iter().filter_map(|lane| lane.head.as_ref().map(|slot| slot.at)).min() else {
                return Ok(None);
            };
            let entries: Vec<Entry<K>> = self
                .lanes
                .iter_mut()
                .filter(|lane| lane.head.as_ref().is_some_and(|slot| slot.at == at))
                .filter_map(Lane::take)
                .collect();
            if at < self.from {
                continue;
            }
            let at = at.with_timezone(&entries[0].slot.at.timezone());
            return Ok(Some(Moment { at, entries }));
        }
    }
}
