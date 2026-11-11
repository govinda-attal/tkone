//! Bundles: schedules that refer to each other, built together.
//!
//! A derived rule can refer to a rule of another schedule in its bundle:
//! `{collections.repayment}~2PT09:00:00` is two business days before each of
//! the `collections` schedule's repayments, as adjusted there, on its own
//! calendar. A bundle is what an application submits together, such as
//! everything one loan runs: its collections, reminders and closure. It is
//! built as a whole, or not at all:
//!
//! - every reference names a schedule of the bundle and a rule of it that
//!   has slots of its own (not a basis, and not itself derived);
//! - schedules do not refer to each other in a loop, even through others;
//! - a definition refers only to definitions: shared definitions never
//!   depend on an instance.
//!
//! A reference to an instance follows the slots it is due at: those its
//! definition produces, not those it wants and misses. It follows the
//! schedule, never what has run.
//!
//! ```rust
//! use chrono::TimeZone;
//! use chrono_tz::Europe::London;
//! use fallible_iterator::FallibleIterator;
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use tkone_schedule::schedule::{Bundle, Schedule};
//!
//! let start = London.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
//! // Shared definitions: money moves at 12:00, notices go out at 09:00.
//! let servicing = Schedule::builder(start, WeekendSkipper::new()).rule("YY-MM-DD~SBT12:00:00").build()?;
//! let notices = Schedule::builder(start, WeekendSkipper::new()).rule("YY-MM-DD~SBT09:00:00").build()?;
//!
//! // One loan: its repayments, and a reminder two business days before each.
//! let loan = Bundle::builder()
//!     .instance("collections", &servicing, servicing.selector(start).named_rule("repayment", "YY-MM-L~MFT12:00:00"))
//!     .instance("reminders", &notices, notices.selector(start).named_rule("reminder", "{collections.repayment}~2PT09:00:00"))
//!     .build()?;
//!
//! // 31 May 2026 is a Sunday: repaid Friday 29 May, reminded Wednesday 27 May.
//! let reminder = loan.schedule("reminders").unwrap().slots()?.next()?.unwrap();
//! assert_eq!(reminder.at, London.with_ymd_and_hms(2026, 5, 27, 9, 0, 0).unwrap());
//! assert_eq!(reminder.sources[0].basis, Some(London.with_ymd_and_hms(2026, 5, 29, 12, 0, 0).unwrap()));
//! # Ok::<(), tkone_schedule::Error>(())
//! ```

use std::collections::BTreeMap;
use std::sync::Arc;

use super::derived::Reference;
use super::{Referent, Schedule, ScheduleBuilder, Selector, SelectorBuilder};
use crate::biz_day::BizDayProcessor;
use crate::{Error, Result};

/// Schedules that refer to each other, built together; see the
/// [module documentation](crate::schedule::bundle).
#[derive(Debug, Clone)]
pub struct Bundle<B: BizDayProcessor> {
    members: BTreeMap<String, Member<B>>,
}

/// One schedule of a [`Bundle`].
#[derive(Debug, Clone)]
pub enum Member<B: BizDayProcessor> {
    /// A definition: a schedule that generates slots.
    Definition(Schedule<B>),
    /// An instance: a selection from its definition's slots.
    Instance {
        /// The definition it selects from, which is not part of the bundle.
        definition: Box<Schedule<B>>,
        /// Its selection.
        selector: Selector<B>,
    },
}

/// Builds a [`Bundle`]; see [`Bundle::builder`].
#[derive(Debug, Clone)]
pub struct BundleBuilder<B: BizDayProcessor> {
    pending: Vec<(String, Pending<B>)>,
}

#[derive(Debug, Clone)]
enum Pending<B: BizDayProcessor> {
    Definition(ScheduleBuilder<B>),
    Instance { definition: Box<Schedule<B>>, builder: SelectorBuilder<B> },
}

impl<B: BizDayProcessor> Pending<B> {
    fn builder(&self) -> &ScheduleBuilder<B> {
        match self {
            Pending::Definition(builder) => builder,
            Pending::Instance { builder, .. } => &builder.0,
        }
    }

    fn is_instance(&self) -> bool {
        matches!(self, Pending::Instance { .. })
    }
}

impl<B: BizDayProcessor> Bundle<B> {
    /// An empty bundle to add schedules to.
    pub fn builder() -> BundleBuilder<B> {
        BundleBuilder { pending: Vec::new() }
    }

    /// The schedules' names, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.members.keys().map(String::as_str)
    }

    /// The member called `name`.
    pub fn member(&self, name: &str) -> Option<&Member<B>> {
        self.members.get(name)
    }

    /// The slots of the member called `name`: a definition's, or the
    /// slots an instance's selection wants (see [`Selector::schedule`]).
    pub fn schedule(&self, name: &str) -> Option<&Schedule<B>> {
        self.members.get(name).map(|member| match member {
            Member::Definition(schedule) => schedule,
            Member::Instance { selector, .. } => selector.schedule(),
        })
    }

    /// The instance called `name`: its selection, and its definition.
    pub fn instance(&self, name: &str) -> Option<(&Selector<B>, &Schedule<B>)> {
        match self.members.get(name)? {
            Member::Instance { definition, selector } => Some((selector, &**definition)),
            Member::Definition(_) => None,
        }
    }
}

impl<B: BizDayProcessor> BundleBuilder<B> {
    /// Add a definition called `name`.
    pub fn definition(mut self, name: impl Into<String>, builder: ScheduleBuilder<B>) -> Self {
        self.pending.push((name.into(), Pending::Definition(builder)));
        self
    }

    /// Add an instance called `name`, selecting from `definition` (built
    /// from it with [`Schedule::selector`]). The definition is not part of
    /// the bundle.
    pub fn instance(mut self, name: impl Into<String>, definition: &Schedule<B>, builder: SelectorBuilder<B>) -> Self {
        self.pending.push((name.into(), Pending::Instance { definition: Box::new(definition.clone()), builder }));
        self
    }

    /// Check the references and build every schedule, those referred to
    /// first. One that fails fails the bundle.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchedule`] when a name is empty, contains `.` or is
    /// used twice; a reference names a schedule not in the bundle, or a
    /// definition refers to an instance; schedules refer to each other in a
    /// loop; or a schedule fails its own checks (with its name).
    pub fn build(self) -> Result<Bundle<B>> {
        let invalid = |what: String| Error::InvalidSchedule(what);
        let mut index = BTreeMap::new();
        for (n, (name, _)) in self.pending.iter().enumerate() {
            if name.is_empty() || name.contains('.') || name.contains(char::is_whitespace) {
                return Err(invalid(format!("`{name}` cannot name a schedule: it must be a word without `.`")));
            }
            if index.insert(name.clone(), n).is_some() {
                return Err(invalid(format!("the schedule `{name}` is added twice")));
            }
        }
        // Which schedules each one refers to.
        let mut refers: Vec<Vec<usize>> = vec![Vec::new(); self.pending.len()];
        for (n, (name, pending)) in self.pending.iter().enumerate() {
            for rule in &pending.builder().0.rules {
                let Some(Ok(Reference { doc: Some(doc), id, .. })) = Reference::parse(&rule.spec) else {
                    continue;
                };
                if doc == *name {
                    return Err(invalid(format!("`{name}` refers to `{doc}.{id}`: write its own rules as `{{{id}}}`")));
                }
                let Some(&to) = index.get(&doc) else {
                    return Err(invalid(format!("`{name}` refers to `{doc}.{id}`, but there is no schedule `{doc}`")));
                };
                if !pending.is_instance() && self.pending[to].1.is_instance() {
                    return Err(invalid(format!(
                        "the definition `{name}` refers to `{doc}.{id}`, an instance: definitions refer only to \
                         definitions"
                    )));
                }
                if !refers[n].contains(&to) {
                    refers[n].push(to);
                }
            }
        }
        let order = order(&self.pending, &refers)?;
        let mut built: Vec<Option<Arc<Referent<B>>>> = vec![None; self.pending.len()];
        let mut members = BTreeMap::new();
        let mut pending: Vec<Option<(String, Pending<B>)>> = self.pending.into_iter().map(Some).collect();
        for n in order {
            let (name, member) = pending[n].take().expect("each is built once");
            let others: BTreeMap<String, Arc<Referent<B>>> = refers[n]
                .iter()
                .map(|&to| {
                    let referent = built[to].clone().expect("built before those that refer to it");
                    (name_of(&index, to), referent)
                })
                .collect();
            let named = |e: Error| match e {
                Error::InvalidSchedule(why) => Error::InvalidSchedule(format!("`{name}`: {why}")),
                other => other,
            };
            let (referent, member) = match member {
                Pending::Definition(mut builder) => {
                    builder.0.others = others;
                    let schedule = builder.build().map_err(named)?;
                    (Referent { schedule: schedule.clone(), definition: None }, Member::Definition(schedule))
                }
                Pending::Instance { definition, mut builder } => {
                    builder.0 .0.others = others;
                    let selector = builder.build().map_err(named)?;
                    let referent =
                        Referent { schedule: selector.schedule().clone(), definition: Some((*definition).clone()) };
                    (referent, Member::Instance { definition, selector })
                }
            };
            built[n] = Some(Arc::new(referent));
            members.insert(name, member);
        }
        Ok(Bundle { members })
    }
}

fn name_of(index: &BTreeMap<String, usize>, n: usize) -> String {
    index.iter().find(|(_, &i)| i == n).map(|(name, _)| name.clone()).expect("every index has a name")
}

/// The schedules in an order where each comes after those it refers to;
/// a loop is an error naming it.
fn order<B: BizDayProcessor>(pending: &[(String, Pending<B>)], refers: &[Vec<usize>]) -> Result<Vec<usize>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Visiting,
        Done,
    }
    fn visit(
        n: usize,
        refers: &[Vec<usize>],
        marks: &mut [Mark],
        path: &mut Vec<usize>,
        out: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        match marks[n] {
            Mark::Done => return None,
            Mark::Visiting => {
                let from = path.iter().position(|&p| p == n).expect("on the path");
                let mut cycle = path[from..].to_vec();
                cycle.push(n);
                return Some(cycle);
            }
            Mark::New => {}
        }
        marks[n] = Mark::Visiting;
        path.push(n);
        for &to in &refers[n] {
            if let Some(cycle) = visit(to, refers, marks, path, out) {
                return Some(cycle);
            }
        }
        path.pop();
        marks[n] = Mark::Done;
        out.push(n);
        None
    }
    let mut marks = vec![Mark::New; pending.len()];
    let mut out = Vec::new();
    for n in 0..pending.len() {
        if let Some(cycle) = visit(n, refers, &mut marks, &mut Vec::new(), &mut out) {
            let names: Vec<&str> = cycle.iter().map(|&i| pending[i].0.as_str()).collect();
            return Err(Error::InvalidSchedule(format!(
                "schedules refer to each other in a loop: {}",
                names.join(" → ")
            )));
        }
    }
    Ok(out)
}
