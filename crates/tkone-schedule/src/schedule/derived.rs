//! Rules derived from another rule's slots: `{repayment}~2PT09:00:00`.
//!
//! A rule's date part can name another rule instead of a date pattern: one
//! of the same schedule (`{repayment}`), or of another schedule in the same
//! [`Bundle`](super::Bundle) (`{collections.repayment}`). Its dates are that
//! rule's adjusted dates, then its own one adjustment applies. "Two business
//! days before the repayment, after Modified Following" is written once, as
//! two rules, without counting back from the unadjusted date.

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use super::{Slot, Slots};
use crate::biz_day::BizDayProcessor;
use crate::datetime::SpecIterator;
use crate::utils::resolve_local;
use crate::{DstPolicy, Error, Occurrence, Result};

/// A parsed `{[document.]id}[~adjustment]Ttime`.
#[derive(Debug, Clone)]
pub(super) struct Reference {
    /// The other schedule of the bundle the rule is in, if not this one.
    pub doc: Option<String>,
    /// The rule referred to.
    pub id: String,
    /// The adjustment, as a date spec with every date (`YY-MM-DD~2P`).
    pub adjustment: crate::date::Spec,
    /// The times of day, read with the referred slot's time as the start's
    /// is for other rules: `_:_:_` is that time, `[09,13]:00:00` two times.
    pub time: crate::time::Spec,
}

impl Reference {
    /// `None` when `rule` is not a reference (does not start with `{`).
    pub fn parse(rule: &str) -> Option<Result<Self>> {
        let rest = rule.strip_prefix('{')?;
        Some(Self::parse_rest(rule, rest))
    }

    fn parse_rest(rule: &str, rest: &str) -> Result<Self> {
        let invalid = |why: &str| Error::InvalidSchedule(format!("`{rule}`: {why}"));
        let (name, rest) = rest.split_once('}').ok_or_else(|| invalid("a reference is `{id}`, closed by `}`"))?;
        let (doc, id) = match name.split_once('.') {
            Some((doc, id)) => (Some(doc), id),
            None => (None, name),
        };
        let bad = |part: &str| part.is_empty() || part.contains(char::is_whitespace);
        if bad(id) || doc.is_some_and(bad) {
            return Err(invalid("a reference names a rule id, or a document and a rule: `{collections.repayment}`"));
        }
        let (adjustment, time) = rest
            .split_once('T')
            .ok_or_else(|| invalid("a derived rule is `{id}`, an optional adjustment (`~2P`), then `T` and a time"))?;
        if !adjustment.is_empty() && !adjustment.starts_with('~') {
            return Err(invalid("only an adjustment (`~2P`, `~NB`, …) can follow a reference"));
        }
        let adjustment = format!("YY-MM-DD{adjustment}")
            .parse::<crate::date::Spec>()
            .map_err(|e| invalid(&format!("the adjustment: {e}")))?;
        let time = time.parse::<crate::time::Spec>().map_err(|e| invalid(&format!("the time: {e}")))?;
        Ok(Self { doc: doc.map(str::to_string), id: id.to_string(), adjustment, time })
    }

    /// How many business days later than its referred slot a derived slot
    /// can be.
    pub fn forward_reach(&self) -> u32 {
        self.adjustment.forward_reach()
    }

    /// How many business days earlier than its referred slot a derived slot
    /// can be.
    pub fn backward_reach(&self) -> u32 {
        self.adjustment.backward_reach()
    }

    #[cfg(feature = "serde")]
    pub fn uses_business_days(&self) -> bool {
        self.adjustment.uses_business_days()
    }
}

/// A derived occurrence, with the referred slot's instant.
pub(super) type Derivation = (Occurrence<DateTime<Tz>>, DateTime<Tz>);

/// Where a derived rule's referred slots come from.
#[derive(Debug)]
pub(super) enum Basis<B: BizDayProcessor> {
    /// A rule of the same schedule: its occurrences.
    Rule(Box<SpecIterator<Tz, B>>),
    /// A rule of another schedule in the bundle: its slots.
    Other(Box<Referred<B>>),
}

/// Another schedule's slots for one of its rules. For an instance, only
/// those its definition produces: the slots it is due at, not those it
/// wants and misses.
#[derive(Debug)]
pub(super) struct Referred<B: BizDayProcessor> {
    pub slots: Slots<B>,
    pub rule: String,
    /// An instance's definition: its slots, and the one read ahead.
    pub definition: Option<(Slots<B>, Option<Slot>)>,
    /// Slots on a later date (in the derived rule's zone) cannot give a
    /// derived slot within `until`: the search stops there.
    pub last_date: Option<NaiveDate>,
}

impl<B: BizDayProcessor> Basis<B> {
    /// The next referred instant, in `zone`.
    fn next(&mut self, zone: &Tz) -> Result<Option<DateTime<Tz>>> {
        let referred = match self {
            Basis::Rule(rule) => return Ok(rule.next()?.map(|occurrence| *occurrence.observed())),
            Basis::Other(referred) => referred,
        };
        while let Some(slot) = referred.slots.next()? {
            let at = slot.at.with_timezone(zone);
            if referred.last_date.is_some_and(|last| at.date_naive() > last) {
                return Ok(None);
            }
            if !slot.sources.iter().any(|source| source.rule_id.as_deref() == Some(referred.rule.as_str())) {
                continue;
            }
            let Some((produced, head)) = &mut referred.definition else {
                return Ok(Some(at));
            };
            // Keep the instance's slot only if its definition produces it.
            loop {
                if head.is_none() {
                    *head = produced.next()?;
                    if head.is_none() {
                        // The definition has ended: nothing more is due.
                        return Ok(None);
                    }
                }
                match head {
                    Some(next) if next.at < slot.at => *head = None,
                    Some(next) if next.at == slot.at => return Ok(Some(at)),
                    _ => break,
                }
            }
        }
        Ok(None)
    }
}

/// A derived rule's occurrences, from its referred rule's.
#[derive(Debug)]
pub(super) struct Derived<B: BizDayProcessor> {
    pub reference: Reference,
    /// The referred rule's slots.
    pub basis: Basis<B>,
    pub bdp: B,
    pub zone: Tz,
    pub policy: DstPolicy,
    /// The last actual time given: one referred date can come from several
    /// referred occurrences (`~NB` moves Saturday and Sunday to Monday).
    pub last: Option<DateTime<Tz>>,
    /// The date whose times are being given, if any.
    pub day: Option<Day>,
}

/// A referred slot's date, adjusted, and where its times have got to.
#[derive(Debug)]
pub(super) struct Day {
    /// The referred slot.
    from: DateTime<Tz>,
    /// The derived date before adjustment: the referred slot's date.
    actual: NaiveDate,
    /// The derived date after it.
    observed: NaiveDate,
    /// The referred slot's time on the observed date: the time spec's anchor.
    anchor: NaiveDateTime,
    /// The last time given on the observed date, or just before it began.
    local: NaiveDateTime,
}

impl<B: BizDayProcessor> Derived<B> {
    /// The next occurrence, with the referred slot's instant.
    pub fn next(&mut self) -> Result<Option<Derivation>> {
        loop {
            if self.day.is_none() && !self.next_day()? {
                return Ok(None);
            }
            let day = self.day.as_mut().expect("set above");
            let Some(next) = crate::time::next_after(&self.reference.time, day.anchor, day.local) else {
                self.day = None;
                continue;
            };
            if next.date() != day.observed {
                self.day = None;
                continue;
            }
            day.local = next;
            let (from, at) = (day.from, next.time());
            let actual = resolve_local(&self.zone, day.actual.and_time(at), self.policy)?;
            let observed = resolve_local(&self.zone, next, self.policy)?;
            if self.last.is_some_and(|last| actual <= last) {
                continue;
            }
            self.last = Some(actual);
            if let Some(zone) = self.bdp.time_zone() {
                let code = self.bdp.code().unwrap_or_else(|| "the calendar".into());
                crate::datetime::check_calendar_date(&code, &zone, &observed)?;
            }
            let occurrence = if observed > actual {
                Occurrence::AdjustedLater(actual, observed)
            } else if observed < actual {
                Occurrence::AdjustedEarlier(actual, observed)
            } else {
                Occurrence::Exact(actual)
            };
            return Ok(Some((occurrence, from)));
        }
    }

    /// Move to the next referred slot whose date the adjustment keeps
    /// (`~SB` drops some); `false` when there are none.
    fn next_day(&mut self) -> Result<bool> {
        while let Some(from) = self.basis.next(&self.zone)? {
            let date = from.date_naive();
            let Some(adjusted) = crate::date::adjust_one(&self.reference.adjustment, from.naive_local(), &self.bdp)?
            else {
                continue; // `~SB`
            };
            let observed = adjusted.observed().date();
            let midnight = observed.and_time(NaiveTime::MIN);
            self.day = Some(Day {
                from,
                actual: date,
                observed,
                anchor: observed.and_time(from.naive_local().time()),
                local: midnight - Duration::nanoseconds(1),
            });
            return Ok(true);
        }
        Ok(false)
    }
}
