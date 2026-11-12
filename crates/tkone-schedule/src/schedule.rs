//! Schedules: rules with everything around them.
//!
//! A rule string (`YY-MM-L~PBT18:00:00`) describes one recurrence. A
//! [`Schedule`](crate::schedule::Schedule) adds what surrounds it: the start it is anchored to and the
//! time zone it is read in, the business days its adjustments use, an end
//! (`until`) or a count, several named rules, and exclusions. See
//! `docs/schedule-document.md` in the repository for the design.
//!
//! ```rust
//! use chrono::TimeZone;
//! use chrono_tz::Europe::London;
//! use fallible_iterator::FallibleIterator;
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use tkone_schedule::schedule::Schedule;
//!
//! let start = London.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
//! let schedule = Schedule::builder(start, WeekendSkipper::new())
//!     .named_rule("open", "YY-MM-DD~SBT09:00:00")
//!     .named_rule("month-end", "YY-MM-L~PBT18:00:00")
//!     .build()?;
//!
//! // 31 May 2026 is a Sunday: month end moves to Friday 29 May.
//! let after = London.with_ymd_and_hms(2026, 5, 29, 12, 0, 0).unwrap();
//! let slot = schedule.slots_after(&after)?.next()?.unwrap();
//! assert_eq!(slot.at, London.with_ymd_and_hms(2026, 5, 29, 18, 0, 0).unwrap());
//! assert_eq!(slot.sources[0].rule_id.as_deref(), Some("month-end"));
//! assert_eq!(slot.sources[0].actual, London.with_ymd_and_hms(2026, 5, 31, 18, 0, 0).unwrap());
//! # Ok::<(), tkone_schedule::Error>(())
//! ```

pub mod bundle;
mod derived;
#[cfg(feature = "serde")]
pub mod document;
pub mod period;
pub mod selector;
pub mod timeline;

pub use bundle::{Bundle, BundleBuilder};
pub use period::{Boundary, Period, Periods};
pub use selector::{Next, Selector, SelectorBuilder};
pub use timeline::{Entry, Moment, Moments, Timeline};

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ops::RangeInclusive;
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, NaiveTime};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use crate::biz_day::BizDayProcessor;
use crate::date::NaiveSpecIterator as DateSpecIterator;
use crate::datetime::SpecIterator;
use crate::utils::{just_before, resolve_local};
use crate::{DstPolicy, Error, IntervalMode, Occurrence, Result, TimeAnchor};

/// One instant of a [`Schedule`], with every rule that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// When it happens: the adjusted (observed) instant.
    pub at: DateTime<Tz>,
    /// The rules that produced `at`, in rule order, each with its actual
    /// (unadjusted) time. Usually one; more when rules coincide, or when an
    /// adjustment moves several dates to the same day (`~NB` moves Saturday
    /// and Sunday to Monday).
    pub sources: Vec<Source>,
}

impl Slot {
    /// Whether the rule with id `id` produced this slot: on a schedule with
    /// `accrue` every business day and `capitalise` at the month end, the
    /// month end's one slot is for both.
    ///
    /// ```rust
    /// use chrono::TimeZone;
    /// use fallible_iterator::FallibleIterator;
    /// use tkone_schedule::biz_day::WeekendSkipper;
    /// use tkone_schedule::schedule::Schedule;
    ///
    /// let start = chrono_tz::Europe::London.with_ymd_and_hms(2026, 5, 29, 0, 0, 0).unwrap();
    /// let savings = Schedule::builder(start, WeekendSkipper::new())
    ///     .named_rule("accrue", "YY-MM-DD~SBT18:00:00")
    ///     .named_rule("capitalise", "YY-MM-L~PBT18:00:00")
    ///     .build()?;
    /// // 31 May 2026 is a Sunday: Friday the 29th is the month end.
    /// let friday = savings.slots()?.next()?.unwrap();
    /// assert!(friday.is_for("accrue") && friday.is_for("capitalise"));
    /// # Ok::<(), tkone_schedule::Error>(())
    /// ```
    pub fn is_for(&self, id: &str) -> bool {
        self.sources.iter().any(|source| source.rule_id.as_deref() == Some(id))
    }
}

/// A rule's part in a [`Slot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The rule's id, if it was given one.
    pub rule_id: Option<String>,
    /// The time the rule produced before any business-day adjustment. For
    /// a derived rule (`{repayment}~2PT09:00:00`), the referred slot's date
    /// at the rule's time.
    pub actual: DateTime<Tz>,
    /// For a derived rule, the referred slot it comes from: "the reminder
    /// for the 16 February repayment". `None` for other rules.
    pub basis: Option<DateTime<Tz>>,
}

/// Which date an exclusion is tested against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "kebab-case"))]
pub enum ExcludeMatch {
    /// The rule's date before adjustment: excluding 25 December drops a
    /// `~NB` occurrence moved from the 25th to the 27th. The default.
    #[default]
    Actual,
    /// The date it happens: excluding 27 December drops whatever lands on it.
    Observed,
}

#[derive(Debug, Clone)]
struct Rule<B: BizDayProcessor> {
    id: Option<String>,
    spec: String,
    /// Referred to by derived rules; no slots of its own.
    basis: bool,
    /// For a derived rule, the rule it refers to, set by `build`.
    reference: Option<(Target<B>, derived::Reference)>,
}

/// The rule a derived rule refers to.
#[derive(Debug, Clone)]
enum Target<B: BizDayProcessor> {
    /// A rule of the same schedule, by index.
    Local(usize),
    /// A rule of another schedule in the bundle.
    Other(Arc<Referent<B>>),
}

/// Another schedule of a bundle, as a derived rule sees it: its slots, and
/// for an instance, its definition (only the slots it produces are due).
#[derive(Debug)]
pub(crate) struct Referent<B: BizDayProcessor> {
    pub schedule: Schedule<B>,
    pub definition: Option<Schedule<B>>,
}

/// Rules anchored at a start, read in its time zone, on a business-day
/// processor's days, with an optional end, count and exclusions.
///
/// Built with [`Schedule::builder`]; iterated with [`slots`](Self::slots)
/// or [`slots_after`](Self::slots_after).
#[derive(Debug, Clone)]
pub struct Schedule<B: BizDayProcessor> {
    bdp: B,
    start: DateTime<Tz>,
    until: Option<DateTime<Tz>>,
    count: Option<usize>,
    rules: Vec<Rule<B>>,
    exclude_dates: BTreeSet<NaiveDate>,
    exclude_rules: Vec<String>,
    exclude_match: ExcludeMatch,
    options: (DstPolicy, IntervalMode, TimeAnchor),
    /// The most business days any rule's adjustment moves a date later.
    reach: u32,
    /// The most business days any rule's adjustment moves a date earlier.
    back_reach: u32,
    /// The other schedules of its bundle that its rules refer to, by name.
    others: BTreeMap<String, Arc<Referent<B>>>,
}

/// Builds a [`Schedule`]; see [`Schedule::builder`].
#[derive(Debug, Clone)]
pub struct ScheduleBuilder<B: BizDayProcessor>(Schedule<B>);

impl<B: BizDayProcessor> Schedule<B> {
    /// A schedule anchored at `start`, in `start`'s time zone, on `bdp`'s
    /// business days.
    ///
    /// `start` anchors cadences (`2W`, `3M`) and the time of day, and is the
    /// earliest a slot can be. It is a slot only if a rule produces it.
    pub fn builder(start: DateTime<Tz>, bdp: B) -> ScheduleBuilder<B> {
        ScheduleBuilder(Schedule {
            bdp,
            start,
            until: None,
            count: None,
            rules: Vec::new(),
            exclude_dates: BTreeSet::new(),
            exclude_rules: Vec::new(),
            exclude_match: ExcludeMatch::default(),
            options: Default::default(),
            reach: 0,
            back_reach: 0,
            others: BTreeMap::new(),
        })
    }

    /// The start the schedule is anchored to.
    pub fn start(&self) -> &DateTime<Tz> {
        &self.start
    }

    /// The time zone the schedule is read in.
    pub fn zone(&self) -> Tz {
        self.start.timezone()
    }

    /// The last instant a slot can be at, if any.
    pub fn until(&self) -> Option<&DateTime<Tz>> {
        self.until.as_ref()
    }

    /// The business-day processor.
    pub fn bdp(&self) -> &B {
        &self.bdp
    }

    /// Every slot, from the start.
    pub fn slots(&self) -> Result<Slots<B>> {
        self.slots_after(&just_before(&self.start))
    }

    /// The slots strictly after `after`.
    ///
    /// The same slots as [`slots`](Self::slots) from that point: the result
    /// depends only on the schedule, not on where iteration starts. With a
    /// count, the schedule is replayed from its start to keep it.
    pub fn slots_after(&self, after: &DateTime<Tz>) -> Result<Slots<B>> {
        self.slots_in(after, self.until)
    }

    /// The slots in `range`, both ends included: the same slots as
    /// [`slots`](Self::slots) there.
    ///
    /// Always ends, even on a schedule with no `until`, and even when
    /// exclusions drop every date: the place to start when a test steps
    /// through ten years of a loan.
    ///
    /// ```rust
    /// use chrono::TimeZone;
    /// use chrono_tz::Europe::London;
    /// use fallible_iterator::FallibleIterator;
    /// use tkone_schedule::biz_day::WeekendSkipper;
    /// use tkone_schedule::schedule::Schedule;
    ///
    /// let start = London.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    /// let payments = Schedule::builder(start, WeekendSkipper::new()).rule("YY-MM-L~PBT00:00:00").build()?;
    /// let to = London.with_ymd_and_hms(2035, 12, 31, 0, 0, 0).unwrap();
    /// assert_eq!(payments.slots_between(start..=to)?.count()?, 120);
    /// # Ok::<(), tkone_schedule::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// As [`slots_after`](Self::slots_after).
    pub fn slots_between(&self, range: RangeInclusive<DateTime<Tz>>) -> Result<Slots<B>> {
        let (from, to) = range.into_inner();
        let to = to.with_timezone(&self.zone());
        let until = match self.until {
            Some(until) if until < to => until,
            _ => to,
        };
        self.slots_in(&just_before(&from), Some(until))
    }

    /// The slots whose rules put them on or after the start, but whose
    /// adjustment moved them before it: these are dropped, since nothing is
    /// before the start ([D1] in the design).
    ///
    /// For a schedule that starts now, that is right: the slot is in the
    /// past. For a fixed term, such as a loan drawn on Saturday 31 January
    /// with month-end repayments moved to the previous business day, it is
    /// a repayment that silently never happens. A test, or a check before
    /// a schedule is saved, can require this to be empty.
    ///
    /// ```rust
    /// use chrono::TimeZone;
    /// use chrono_tz::Europe::London;
    /// use tkone_schedule::biz_day::WeekendSkipper;
    /// use tkone_schedule::schedule::Schedule;
    ///
    /// let drawn = London.with_ymd_and_hms(2026, 1, 31, 0, 0, 0).unwrap();
    /// let repayments = Schedule::builder(drawn, WeekendSkipper::new()).rule("YY-MM-L~PBT00:00:00").build()?;
    /// let lost = repayments.moved_before_start()?;
    /// assert_eq!(lost[0].at, London.with_ymd_and_hms(2026, 1, 30, 0, 0, 0).unwrap());
    /// assert_eq!(lost[0].sources[0].actual, drawn);
    /// # Ok::<(), tkone_schedule::Error>(())
    /// ```
    ///
    /// [D1]: https://github.com/govinda-attal/tkone/blob/main/docs/schedule-document.md#d1-start-is-an-anchor-not-an-occurrence
    ///
    /// # Errors
    ///
    /// As [`slots`](Self::slots).
    pub fn moved_before_start(&self) -> Result<Vec<Slot>> {
        let mut slots = self.slots_in(&just_before(&self.start), Some(self.start))?;
        slots.before_start = true;
        slots.collect()
    }

    /// The slots strictly after `after` and no later than `until`.
    fn slots_in(&self, after: &DateTime<Tz>, until: Option<DateTime<Tz>>) -> Result<Slots<B>> {
        let after = after.with_timezone(&self.zone());
        let from =
            if self.count.is_some() || after < self.start { just_before(&self.start) } else { self.lookback(&after)? };
        let exclusions = Exclusions {
            dates: self.exclude_dates.clone(),
            rules: self
                .exclude_rules
                .iter()
                .map(|spec| DateSpecIterator::exclusion(spec, self.bdp.clone(), self.start.naive_local()))
                .collect::<Result<_>>()?,
            by: self.exclude_match,
        };
        let streams = self
            .rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| !rule.basis)
            .map(|(index, rule)| {
                let iter = match &rule.reference {
                    None => RuleIter::Spec(Box::new(SpecIterator::anchored(
                        &rule.spec,
                        self.bdp.clone(),
                        &self.start,
                        &from,
                        self.options,
                    )?)),
                    Some((target, reference)) => RuleIter::Derived(Box::new(derived::Derived {
                        reference: reference.clone(),
                        basis: match target {
                            Target::Local(to) => derived::Basis::Rule(Box::new(SpecIterator::anchored(
                                &self.rules[*to].spec,
                                self.bdp.clone(),
                                &self.start,
                                &from,
                                self.options,
                            )?)),
                            Target::Other(referent) => derived::Basis::Other(Box::new(derived::Referred {
                                slots: referent.schedule.slots_after(&from)?,
                                rule: reference.id.clone(),
                                definition: match &referent.definition {
                                    Some(definition) => Some((definition.slots_after(&from)?, None)),
                                    None => None,
                                },
                                last_date: self.last_actual(until.as_ref()),
                            })),
                        },
                        bdp: self.bdp.clone(),
                        zone: self.zone(),
                        policy: self.options.0,
                        last: None,
                        day: None,
                    })),
                };
                Ok(RuleStream { index, iter, pending: None, exhausted: false, last: None, batch: None })
            })
            .collect::<Result<_>>()?;
        Ok(Slots {
            ids: self.rules.iter().map(|rule| rule.id.clone()).collect(),
            streams,
            exclusions,
            ready: VecDeque::new(),
            after,
            start: self.start,
            until,
            last_actual: self.last_actual(until.as_ref()),
            before_start: false,
            remaining: self.count,
            done: false,
        })
    }

    /// The latest actual date a slot no later than `until` can have: an
    /// adjustment moves a date at most `back_reach` business days earlier.
    /// Rules stop there, even when exclusions drop every date. `None` when
    /// there is no `until`, or when it is too near the end of the business
    /// days to count past it.
    fn last_actual(&self, until: Option<&DateTime<Tz>>) -> Option<NaiveDate> {
        let until = until?;
        match self.back_reach {
            0 => Some(until.date_naive()),
            reach => self.bdp.add(&until.naive_local(), reach).ok().map(|dtm| dtm.date()),
        }
    }

    /// Where rule iterators start for `slots_after(after)`: just before a
    /// midnight early enough that no occurrence observed after `after` is
    /// missed. An adjustment moves a date at most `reach` business days
    /// later, so an earlier actual date cannot be observed after `after`.
    /// Starting at a midnight, never mid-day, keeps `Elapsed` steps counted
    /// from each day's first time rather than from `after`.
    fn lookback(&self, after: &DateTime<Tz>) -> Result<DateTime<Tz>> {
        let date = match self.reach {
            0 => after.date_naive(),
            reach => self.bdp.sub(&after.naive_local(), reach)?.date(),
        };
        let midnight = resolve_local(&self.zone(), date.and_time(NaiveTime::MIN), DstPolicy::Adjust)?;
        Ok(just_before(&midnight))
    }
}

impl<B: BizDayProcessor> ScheduleBuilder<B> {
    /// Add a rule, a datetime spec (`YY-MM-DD~SBT09:00:00`).
    ///
    /// A rule can also be derived from another rule's slots: its date part
    /// names that rule (`{repayment}~2PT09:00:00`, two business days before
    /// each repayment, at 09:00). Its dates are the referred rule's
    /// *adjusted* dates; then its own one adjustment applies, on this
    /// schedule's business days. After the reference come an optional
    /// adjustment and `T` with a time spec, read with the referred slot's
    /// time as other rules' are read with the start: `T09:00:00`,
    /// `T[09,13]:00:00` (two times a day), `T_:_:_` (the referred slot's
    /// time). The referred rule must not itself be derived.
    ///
    /// ```rust
    /// use chrono::TimeZone;
    /// use chrono_tz::Europe::London;
    /// use fallible_iterator::FallibleIterator;
    /// use tkone_schedule::biz_day::WeekendSkipper;
    /// use tkone_schedule::schedule::Schedule;
    ///
    /// let start = London.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    /// let schedule = Schedule::builder(start, WeekendSkipper::new())
    ///     .basis_rule("payment", "YY-MM-L~MFT12:00:00")
    ///     .named_rule("report", "{payment}~2PT09:00:00")
    ///     .build()?;
    /// // 31 May 2026 is a Sunday: paid on Friday 29 May; the report is two
    /// // business days before that, not before the 31st.
    /// let report = schedule.slots()?.next()?.unwrap();
    /// assert_eq!(report.at, London.with_ymd_and_hms(2026, 5, 27, 9, 0, 0).unwrap());
    /// assert_eq!(report.sources[0].basis, Some(London.with_ymd_and_hms(2026, 5, 29, 12, 0, 0).unwrap()));
    /// # Ok::<(), tkone_schedule::Error>(())
    /// ```
    pub fn rule(mut self, spec: impl Into<String>) -> Self {
        self.0.rules.push(Rule { id: None, spec: spec.into(), basis: false, reference: None });
        self
    }

    /// Add a rule with an id, carried on every slot it produces. See
    /// [`rule`](Self::rule).
    pub fn named_rule(mut self, id: impl Into<String>, spec: impl Into<String>) -> Self {
        self.0.rules.push(Rule { id: Some(id.into()), spec: spec.into(), basis: false, reference: None });
        self
    }

    /// Add a rule that derived rules refer to and that has no slots of its
    /// own: a loan's repayments, in a schedule of its reminders. See
    /// [`rule`](Self::rule).
    pub fn basis_rule(mut self, id: impl Into<String>, spec: impl Into<String>) -> Self {
        self.0.rules.push(Rule { id: Some(id.into()), spec: spec.into(), basis: true, reference: None });
        self
    }

    /// The last instant a slot can be at (inclusive).
    pub fn until(mut self, until: DateTime<Tz>) -> Self {
        self.0.until = Some(until);
        self
    }

    /// Stop after `n` slots, counted from the start after exclusions. With
    /// [`until`](Self::until), whichever comes first.
    pub fn count(mut self, n: usize) -> Self {
        self.0.count = Some(n);
        self
    }

    /// No slots on `date`.
    pub fn exclude_date(mut self, date: NaiveDate) -> Self {
        self.0.exclude_dates.insert(date);
        self
    }

    /// No slots on the dates `spec` matches, a date spec (`YY-12-24`).
    /// Cadences in it count from the start.
    pub fn exclude_rule(mut self, spec: impl Into<String>) -> Self {
        self.0.exclude_rules.push(spec.into());
        self
    }

    /// Which date exclusions are tested against.
    pub fn exclude_match(mut self, by: ExcludeMatch) -> Self {
        self.0.exclude_match = by;
        self
    }

    /// See [`DstPolicy`].
    pub fn dst_policy(mut self, policy: DstPolicy) -> Self {
        self.0.options.0 = policy;
        self
    }

    /// See [`IntervalMode`].
    pub fn interval_mode(mut self, mode: IntervalMode) -> Self {
        self.0.options.1 = mode;
        self
    }

    /// See [`TimeAnchor`].
    pub fn time_anchor(mut self, anchor: TimeAnchor) -> Self {
        self.0.options.2 = anchor;
        self
    }

    /// Check the rules and exclusions and build the schedule.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchedule`] when there are no rules, a rule or
    /// exclusion does not parse, two rules share an id, `until` is before the
    /// start, or the count is 0.
    pub fn build(self) -> Result<Schedule<B>> {
        let mut schedule = self.0;
        let invalid = |what: String| Error::InvalidSchedule(what);
        if schedule.rules.is_empty() {
            return Err(invalid("a schedule needs at least one rule".into()));
        }
        let zone = schedule.zone();
        schedule.until = schedule.until.map(|until| until.with_timezone(&zone));
        if schedule.until.is_some_and(|until| until < schedule.start) {
            return Err(invalid("`until` is before the start".into()));
        }
        if schedule.count == Some(0) {
            return Err(invalid("`count` must be at least 1".into()));
        }
        let mut ids = BTreeSet::new();
        let name = |n: usize, rule: &Rule<B>| match &rule.id {
            Some(id) => format!("rule `{id}`"),
            None => format!("rule {}", n + 1),
        };
        if schedule.rules.iter().all(|rule| rule.basis) {
            return Err(invalid("a schedule needs at least one rule that is not a basis".into()));
        }
        for (n, rule) in schedule.rules.iter().enumerate() {
            if rule.id.as_ref().is_some_and(|id| !ids.insert(id.clone())) {
                return Err(invalid(format!("{} is used twice", name(n, rule))));
            }
        }
        let mut referred = vec![false; schedule.rules.len()];
        for n in 0..schedule.rules.len() {
            let rule = &schedule.rules[n];
            let reference = match derived::Reference::parse(&rule.spec) {
                None => {
                    SpecIterator::anchored(
                        &rule.spec,
                        schedule.bdp.clone(),
                        &schedule.start,
                        &schedule.start,
                        schedule.options,
                    )
                    .map_err(|e| invalid(format!("{} (`{}`): {e}", name(n, rule), rule.spec)))?;
                    let date = date_spec(&rule.spec)?;
                    schedule.reach = schedule.reach.max(date.forward_reach());
                    schedule.back_reach = schedule.back_reach.max(date.backward_reach());
                    continue;
                }
                Some(reference) => reference.map_err(|e| invalid(format!("{}: {e}", name(n, rule))))?,
            };
            if let Some(doc) = &reference.doc {
                let referent = schedule.others.get(doc).cloned().ok_or_else(|| {
                    invalid(format!(
                        "{} refers to `{doc}.{}`, but `{doc}` is not in its bundle: build the schedules that refer to \
                         each other together, in a `Bundle`",
                        name(n, rule),
                        reference.id
                    ))
                })?;
                let other =
                    referent.schedule.rules.iter().find(|other| other.id.as_deref() == Some(reference.id.as_str()));
                let what = match other {
                    None => Some("which is not one of its rules"),
                    Some(other) if other.basis => Some("which is a basis, with no slots to refer to"),
                    Some(other) if other.reference.is_some() => {
                        Some("which is itself derived; refer to the rule it derives from")
                    }
                    Some(_) => None,
                };
                if let Some(what) = what {
                    return Err(invalid(format!("{} refers to `{doc}.{}`, {what}", name(n, rule), reference.id)));
                }
                // The other schedule's slots are observed instants: a derived
                // slot is later than one by at most its own forward reach.
                schedule.reach = schedule.reach.max(reference.forward_reach());
                schedule.back_reach = schedule.back_reach.max(reference.backward_reach());
                schedule.rules[n].reference = Some((Target::Other(referent), reference));
                continue;
            }
            let to =
                schedule.rules.iter().position(|other| other.id.as_deref() == Some(reference.id.as_str())).ok_or_else(
                    || invalid(format!("{} refers to `{}`, which no rule is", name(n, rule), reference.id)),
                )?;
            if derived::Reference::parse(&schedule.rules[to].spec).is_some() {
                return Err(invalid(format!(
                    "{} refers to `{}`, which is itself derived; refer to the rule it derives from",
                    name(n, rule),
                    reference.id
                )));
            }
            referred[to] = true;
            // A derived slot is later than its referred slot by at most its
            // own forward reach, and the referred slot than its actual date
            // by the referred rule's.
            let basis = date_spec(&schedule.rules[to].spec)?;
            schedule.reach = schedule.reach.max(basis.forward_reach() + reference.forward_reach());
            schedule.back_reach = schedule.back_reach.max(reference.backward_reach());
            schedule.rules[n].reference = Some((Target::Local(to), reference));
        }
        for (n, rule) in schedule.rules.iter().enumerate() {
            if rule.basis && !referred[n] {
                return Err(invalid(format!("{} is a basis, but no rule refers to it", name(n, rule))));
            }
        }
        for spec in &schedule.exclude_rules {
            DateSpecIterator::exclusion(spec, schedule.bdp.clone(), schedule.start.naive_local())
                .map_err(|e| invalid(format!("exclusion `{spec}`: {e}")))?;
        }
        Ok(schedule)
    }
}

/// The date part of a datetime rule.
fn date_spec(rule: &str) -> Result<crate::date::Spec> {
    rule.parse::<crate::datetime::Spec>()?.date_spec.parse()
}

/// Whether `rule`, a datetime rule or a derived one, asks the business-day
/// processor anything.
#[cfg(feature = "serde")]
pub(crate) fn rule_uses_business_days(rule: &str) -> Result<bool> {
    match derived::Reference::parse(rule) {
        Some(reference) => Ok(reference?.uses_business_days()),
        None => Ok(date_spec(rule)?.uses_business_days()),
    }
}

/// The slots of a [`Schedule`], in time order.
#[derive(Debug)]
pub struct Slots<B: BizDayProcessor> {
    ids: Vec<Option<String>>,
    streams: Vec<RuleStream<B>>,
    exclusions: Exclusions<B>,
    /// Slots of the current observed date, in order.
    ready: VecDeque<Slot>,
    after: DateTime<Tz>,
    start: DateTime<Tz>,
    until: Option<DateTime<Tz>>,
    /// See [`Schedule::last_actual`].
    last_actual: Option<NaiveDate>,
    /// Yield only the slots moved before the start, which are otherwise
    /// dropped; see [`Schedule::moved_before_start`].
    before_start: bool,
    remaining: Option<usize>,
    done: bool,
}

#[derive(Debug)]
struct Exclusions<B: BizDayProcessor> {
    dates: BTreeSet<NaiveDate>,
    rules: Vec<DateSpecIterator<B>>,
    by: ExcludeMatch,
}

impl<B: BizDayProcessor> Exclusions<B> {
    fn matches(&self, date: NaiveDate) -> Result<bool> {
        if self.dates.contains(&date) {
            return Ok(true);
        }
        for rule in &self.rules {
            if rule.matches_date(date)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// One rule's occurrences, grouped by observed date.
///
/// A rule's actual times always increase, but its observed times need not:
/// with `~NB` and two times a day, Saturday's moved 09:00 and 13:00 come
/// before Monday's own 09:00. Its observed dates do not decrease (moving to
/// the next or previous business day keeps dates in order), so a whole
/// observed date is gathered before it is sorted and merged.
#[derive(Debug)]
struct RuleStream<B: BizDayProcessor> {
    index: usize,
    iter: RuleIter<B>,
    /// The first occurrence of the next observed date, read ahead.
    pending: Option<Item>,
    exhausted: bool,
    /// The observed date of the last batch.
    last: Option<NaiveDate>,
    batch: Option<Batch>,
}

/// An occurrence, with the referred slot's instant for a derived rule.
type Item = (Occurrence<DateTime<Tz>>, Option<DateTime<Tz>>);

/// A rule's occurrences: its own, or derived from another rule's.
#[derive(Debug)]
enum RuleIter<B: BizDayProcessor> {
    Spec(Box<SpecIterator<Tz, B>>),
    Derived(Box<derived::Derived<B>>),
}

impl<B: BizDayProcessor> RuleIter<B> {
    fn next(&mut self) -> Result<Option<Item>> {
        match self {
            RuleIter::Spec(iter) => Ok(iter.next()?.map(|occurrence| (occurrence, None))),
            RuleIter::Derived(iter) => Ok(iter.next()?.map(|(occurrence, basis)| (occurrence, Some(basis)))),
        }
    }
}

/// (observed, actual, basis)
type Timed = (DateTime<Tz>, DateTime<Tz>, Option<DateTime<Tz>>);

#[derive(Debug)]
struct Batch {
    date: NaiveDate,
    items: Vec<Timed>,
}

impl<B: BizDayProcessor> RuleStream<B> {
    /// The next occurrence not excluded by its actual date, if its actual
    /// date is no later than `last_actual`.
    fn pull(&mut self, exclusions: &Exclusions<B>, last_actual: Option<NaiveDate>) -> Result<Option<Item>> {
        if self.exhausted {
            return Ok(None);
        }
        while let Some(item) = self.iter.next()? {
            let actual = item.0.actual().date_naive();
            if last_actual.is_some_and(|last| actual > last) {
                break;
            }
            if exclusions.by == ExcludeMatch::Actual && exclusions.matches(actual)? {
                continue;
            }
            return Ok(Some(item));
        }
        self.exhausted = true;
        Ok(None)
    }

    /// Read the next observed date's occurrences into `batch`, unless one is
    /// waiting or the rule is done. Dates after `until` end the rule.
    fn fill(
        &mut self,
        exclusions: &Exclusions<B>,
        until: Option<NaiveDate>,
        last_actual: Option<NaiveDate>,
    ) -> Result<()> {
        if self.batch.is_some() {
            return Ok(());
        }
        let first = match self.pending.take() {
            Some(occurrence) => occurrence,
            None => match self.pull(exclusions, last_actual)? {
                Some(occurrence) => occurrence,
                None => return Ok(()),
            },
        };
        let date = first.0.observed().date_naive();
        if until.is_some_and(|until| date > until) {
            self.exhausted = true;
            return Ok(());
        }
        if self.last.is_some_and(|last| date <= last) {
            return Err(Error::InvalidSchedule(format!(
                "a rule's adjusted dates went back to {date}; its slots cannot be ordered"
            )));
        }
        let triple = |(o, basis): &Item| (*o.observed(), *o.actual(), *basis);
        let mut items = vec![triple(&first)];
        while let Some(next) = self.pull(exclusions, last_actual)? {
            if next.0.observed().date_naive() != date {
                self.pending = Some(next);
                break;
            }
            items.push(triple(&next));
        }
        self.last = Some(date);
        self.batch = Some(Batch { date, items });
        Ok(())
    }
}

impl<B: BizDayProcessor> FallibleIterator for Slots<B> {
    type Item = Slot;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Slot>> {
        loop {
            while let Some(slot) = self.ready.pop_front() {
                if self.until.is_some_and(|until| slot.at > until) || self.remaining == Some(0) {
                    self.done = true;
                    self.ready.clear();
                    return Ok(None);
                }
                if slot.at < self.start {
                    let excluded = self.exclusions.by == ExcludeMatch::Observed
                        && self.exclusions.matches(slot.at.date_naive())?;
                    if self.before_start && !excluded {
                        return Ok(Some(slot));
                    }
                    continue;
                }
                if self.before_start {
                    self.done = true;
                    self.ready.clear();
                    return Ok(None);
                }
                if self.exclusions.by == ExcludeMatch::Observed && self.exclusions.matches(slot.at.date_naive())? {
                    continue;
                }
                if let Some(n) = &mut self.remaining {
                    *n -= 1;
                }
                if slot.at > self.after {
                    return Ok(Some(slot));
                }
            }
            if self.done {
                return Ok(None);
            }
            let until = self.until.map(|until| until.date_naive());
            for stream in &mut self.streams {
                stream.fill(&self.exclusions, until, self.last_actual)?;
            }
            let Some(date) = self.streams.iter().filter_map(|s| s.batch.as_ref().map(|b| b.date)).min() else {
                self.done = true;
                continue;
            };
            let mut items = Vec::new();
            for stream in &mut self.streams {
                if stream.batch.as_ref().is_some_and(|batch| batch.date == date) {
                    let batch = stream.batch.take().expect("checked above");
                    items.extend(batch.items.into_iter().map(|(at, actual, basis)| (at, stream.index, actual, basis)));
                }
            }
            items.sort();
            for (at, index, actual, basis) in items {
                let source = Source { rule_id: self.ids[index].clone(), actual, basis };
                match self.ready.back_mut() {
                    Some(slot) if slot.at == at => slot.sources.push(source),
                    _ => self.ready.push_back(Slot { at, sources: vec![source] }),
                }
            }
        }
    }
}
