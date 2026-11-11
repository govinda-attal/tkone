//! `tkone.schedule`: bindings for `tkone-schedule`.
//!
//! A [`Schedule`] is one rule (date, time or datetime, told apart by trying
//! each); its occurrences run on a calendar passed in, or weekends only. A
//! [`Document`] is a schedule document: rules with their start, time zone,
//! calendar, end, count and exclusions. Times are returned as aware
//! `datetime`s in the schedule's time zone (`zoneinfo.ZoneInfo`).

use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use fallible_iterator::FallibleIterator;
use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tkone_calendar::Calendars;
use tkone_schedule::biz_day::{BizDayProcessor, Direction, WeekendSkipper};
use tkone_schedule::schedule::document::{BusinessDays, ScheduleDoc};
use tkone_schedule::{date, datetime, time};

use crate::calendar::Calendar;

create_exception!(tkone, ScheduleError, PyValueError, "A spec is invalid, or a schedule cannot go on.");

fn schedule_err(e: tkone_schedule::Error) -> PyErr {
    ScheduleError::new_err(e.to_string())
}

// ── Instants ─────────────────────────────────────────────────────────────────

/// An aware `datetime` with any `tzinfo` (`zoneinfo.ZoneInfo`,
/// `datetime.timezone`, …), as an instant. A naive one is refused: it has no
/// instant.
#[derive(Clone, Copy)]
struct Aware(DateTime<FixedOffset>);

impl<'a, 'py> FromPyObject<'a, 'py> for Aware {
    type Error = PyErr;

    fn extract(obj: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if obj.getattr("tzinfo")?.is_none() || obj.call_method0("utcoffset")?.is_none() {
            return Err(ScheduleError::new_err("a datetime with no time zone has no instant; give it a tzinfo"));
        }
        let utc = obj.py().import("datetime")?.getattr("timezone")?.getattr("utc")?;
        Ok(Aware(obj.call_method1("astimezone", (utc,))?.extract()?))
    }
}

impl Aware {
    fn in_zone(self, zone: &chrono_tz::Tz) -> DateTime<chrono_tz::Tz> {
        self.0.with_timezone(zone)
    }
}

// ── Business days ────────────────────────────────────────────────────────────

/// Weekends only, or a holiday calendar.
#[derive(Debug, Clone)]
enum Bdp {
    Weekends(WeekendSkipper),
    Calendar(tkone_calendar::Calendar),
}

impl BizDayProcessor for Bdp {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> tkone_schedule::Result<bool> {
        match self {
            Bdp::Weekends(w) => w.is_biz_day(dtm),
            Bdp::Calendar(c) => c.is_biz_day(dtm),
        }
    }
    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> tkone_schedule::Result<NaiveDateTime> {
        match self {
            Bdp::Weekends(w) => w.find_biz_day(dtm, direction),
            Bdp::Calendar(c) => c.find_biz_day(dtm, direction),
        }
    }
    fn add(&self, dtm: &NaiveDateTime, num: u32) -> tkone_schedule::Result<NaiveDateTime> {
        match self {
            Bdp::Weekends(w) => w.add(dtm, num),
            Bdp::Calendar(c) => c.add(dtm, num),
        }
    }
    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> tkone_schedule::Result<NaiveDateTime> {
        match self {
            Bdp::Weekends(w) => w.sub(dtm, num),
            Bdp::Calendar(c) => c.sub(dtm, num),
        }
    }
    fn code(&self) -> Option<String> {
        match self {
            Bdp::Weekends(w) => w.code(),
            Bdp::Calendar(c) => BizDayProcessor::code(c),
        }
    }
    fn time_zone(&self) -> Option<chrono_tz::Tz> {
        match self {
            Bdp::Weekends(w) => w.time_zone(),
            Bdp::Calendar(c) => c.time_zone(),
        }
    }
}

// ── Occurrences ──────────────────────────────────────────────────────────────

/// One occurrence: the time the rule produced (`actual`) and the time after
/// any business-day adjustment (`observed`).
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Occurrence {
    actual: DateTime<chrono_tz::Tz>,
    observed: DateTime<chrono_tz::Tz>,
    /// `"exact"`, `"later"` (moved later) or `"earlier"` (moved earlier).
    adjustment: &'static str,
}

#[pymethods]
impl Occurrence {
    fn __repr__(&self) -> String {
        match self.adjustment {
            "exact" => format!("Occurrence({})", self.observed.to_rfc3339()),
            how => format!("Occurrence({}, moved {how} to {})", self.actual.to_rfc3339(), self.observed.to_rfc3339()),
        }
    }
}

impl From<tkone_schedule::Occurrence<DateTime<chrono_tz::Tz>>> for Occurrence {
    fn from(o: tkone_schedule::Occurrence<DateTime<chrono_tz::Tz>>) -> Self {
        use tkone_schedule::Occurrence as O;
        let (actual, observed, adjustment) = match o {
            O::Exact(t) => (t, t, "exact"),
            O::AdjustedLater(a, t) => (a, t, "later"),
            O::AdjustedEarlier(a, t) => (a, t, "earlier"),
        };
        Self { actual, observed, adjustment }
    }
}

enum Inner {
    Date(Box<date::SpecIterator<chrono_tz::Tz, Bdp>>),
    Time(Box<time::SpecIterator<chrono_tz::Tz>>),
    DateTime(Box<datetime::SpecIterator<chrono_tz::Tz, Bdp>>),
}

/// An iterator over a schedule's occurrences.
#[pyclass(module = "tkone.schedule")]
struct Occurrences(Inner);

#[pymethods]
impl Occurrences {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<Occurrence>> {
        let next = match &mut self.0 {
            Inner::Date(i) => i.next().map(|o| o.map(Occurrence::from)),
            Inner::DateTime(i) => i.next().map(|o| o.map(Occurrence::from)),
            Inner::Time(i) => i.next().map(|o| o.map(|t| Occurrence { actual: t, observed: t, adjustment: "exact" })),
        };
        next.map_err(schedule_err)
    }
}

// ── Schedules ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Date,
    Time,
    DateTime,
}

/// A rule: date (`YY-MM-BD#3`), time (`09:30:00`) or datetime
/// (`YY-MM-DD~SBT16:30:00`).
#[pyclass(module = "tkone.schedule", frozen)]
struct Schedule {
    spec: String,
    kind: Kind,
}

impl Schedule {
    fn business_days(&self, calendar: Option<&Bound<'_, PyAny>>) -> PyResult<Bdp> {
        match calendar {
            Some(c) if c.is_instance_of::<Calendar>() => Ok(Bdp::Calendar(c.extract::<Calendar>()?.0)),
            Some(c) => Ok(Bdp::Calendar(Calendar::new(&c.extract::<String>()?)?.0)),
            None => Ok(Bdp::Weekends(WeekendSkipper::new())),
        }
    }
}

#[pymethods]
impl Schedule {
    #[new]
    fn new(spec: &str) -> PyResult<Self> {
        let tries = [
            (Kind::DateTime, datetime::describe(spec)),
            (Kind::Date, date::describe(spec)),
            (Kind::Time, time::describe(spec)),
        ];
        if let Some((kind, _)) = tries.iter().find(|(_, r)| r.is_ok()) {
            return Ok(Self { spec: spec.to_string(), kind: *kind });
        }
        // Report the error of the kind the text looks like.
        let looks = if spec.contains(':') && spec.split(':').next().is_some_and(|h| h.contains('-') && h.contains('T'))
        {
            Kind::DateTime
        } else if spec.contains(':') {
            Kind::Time
        } else {
            Kind::Date
        };
        let (_, err) = tries.into_iter().find(|(k, _)| *k == looks).expect("every kind is tried");
        Err(schedule_err(err.expect_err("no kind parsed")))
    }

    #[getter]
    fn spec(&self) -> &str {
        &self.spec
    }

    /// `"date"`, `"time"` or `"datetime"`.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.kind {
            Kind::Date => "date",
            Kind::Time => "time",
            Kind::DateTime => "datetime",
        }
    }

    /// The spec in plain English.
    fn describe(&self) -> PyResult<String> {
        match self.kind {
            Kind::Date => date::describe(&self.spec),
            Kind::Time => time::describe(&self.spec),
            Kind::DateTime => datetime::describe(&self.spec),
        }
        .map_err(schedule_err)
    }

    /// The occurrences, in order.
    ///
    /// - `after`: an aware `datetime`; occurrences strictly after it. Without
    ///   it, from now.
    /// - `calendar`: a `Calendar` or a code (`"GBLO"`). Without it, weekends
    ///   only.
    /// - `tz`: an IANA zone to read the schedule in (else UTC).
    #[pyo3(signature = (after=None, calendar=None, tz=None))]
    fn occurrences(
        &self,
        after: Option<Aware>,
        calendar: Option<&Bound<'_, PyAny>>,
        tz: Option<&str>,
    ) -> PyResult<Occurrences> {
        let zone = match tz {
            Some(name) => name
                .parse::<chrono_tz::Tz>()
                .map_err(|_| ScheduleError::new_err(format!("`{name}` is not an IANA time zone")))?,
            None => chrono_tz::UTC,
        };
        let after = after.map(|a| a.in_zone(&zone)).unwrap_or_else(|| Utc::now().with_timezone(&zone));
        let inner = match self.kind {
            Kind::Time => {
                if calendar.is_some() {
                    return Err(ScheduleError::new_err("a time spec has no business days; pass no calendar"));
                }
                let builder = time::SpecIteratorBuilder::new_after(&self.spec, after);
                Inner::Time(Box::new(builder.build().map_err(schedule_err)?))
            }
            Kind::Date => {
                let bdp = self.business_days(calendar)?;
                let builder = date::SpecIteratorBuilder::new_after(&self.spec, bdp, after);
                Inner::Date(Box::new(builder.build().map_err(schedule_err)?))
            }
            Kind::DateTime => {
                let bdp = self.business_days(calendar)?;
                let builder = datetime::SpecIteratorBuilder::new_after(&self.spec, bdp, after);
                Inner::DateTime(Box::new(builder.build().map_err(schedule_err)?))
            }
        };
        Ok(Occurrences(inner))
    }

    /// The first `n` occurrences, as a list (see `occurrences`).
    #[pyo3(signature = (n, after=None, calendar=None, tz=None))]
    fn take(
        &self,
        n: usize,
        after: Option<Aware>,
        calendar: Option<&Bound<'_, PyAny>>,
        tz: Option<&str>,
    ) -> PyResult<Vec<Occurrence>> {
        let mut iter = self.occurrences(after, calendar, tz)?;
        let mut out = Vec::new();
        while out.len() < n {
            match iter.__next__()? {
                Some(o) => out.push(o),
                None => break,
            }
        }
        Ok(out)
    }

    fn __repr__(&self) -> String {
        format!("Schedule({:?})", self.spec)
    }
}

// ── Documents ────────────────────────────────────────────────────────────────

/// One rule's part in a `Slot`.
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Source {
    /// The rule's id, if it has one.
    rule_id: Option<String>,
    /// The time the rule produced, before any business-day adjustment.
    actual: DateTime<chrono_tz::Tz>,
    /// For a derived rule (`{repayment}~2PT09:00:00`), the referred slot.
    basis: Option<DateTime<chrono_tz::Tz>>,
}

#[pymethods]
impl Source {
    fn __repr__(&self) -> String {
        format!("Source({:?}, {})", self.rule_id.as_deref().unwrap_or("-"), self.actual.to_rfc3339())
    }
}

/// One instant of a document's schedule, with every rule that produced it.
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Slot {
    /// When it happens: the adjusted (observed) time.
    at: DateTime<chrono_tz::Tz>,
    /// The rules that produced it, in rule order.
    sources: Vec<Source>,
}

#[pymethods]
impl Slot {
    fn __repr__(&self) -> String {
        let ids: Vec<_> = self.sources.iter().map(|s| s.rule_id.as_deref().unwrap_or("-")).collect();
        format!("Slot({}, {})", self.at.to_rfc3339(), ids.join(","))
    }
}

impl From<tkone_schedule::schedule::Slot> for Slot {
    fn from(slot: tkone_schedule::schedule::Slot) -> Self {
        let sources =
            slot.sources.into_iter().map(|s| Source { rule_id: s.rule_id, actual: s.actual, basis: s.basis }).collect();
        Self { at: slot.at, sources }
    }
}

type DocSchedule = tkone_schedule::schedule::Schedule<BusinessDays<tkone_calendar::Calendar>>;

/// An iterator over a document's slots.
#[pyclass(module = "tkone.schedule")]
struct Slots(tkone_schedule::schedule::Slots<BusinessDays<tkone_calendar::Calendar>>);

#[pymethods]
impl Slots {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<Slot>> {
        self.0.next().map(|s| s.map(Slot::from)).map_err(schedule_err)
    }
}

/// One end of a `Period`: when it happens (`at`, adjusted) and when its rule
/// put it (`actual`, unadjusted).
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Boundary {
    at: DateTime<chrono_tz::Tz>,
    actual: DateTime<chrono_tz::Tz>,
}

#[pymethods]
impl Boundary {
    fn __repr__(&self) -> String {
        format!("Boundary({}, actual={})", self.at.to_rfc3339(), self.actual.to_rfc3339())
    }
}

impl From<tkone_schedule::schedule::Boundary> for Boundary {
    fn from(b: tkone_schedule::schedule::Boundary) -> Self {
        Self { at: b.at, actual: b.actual }
    }
}

/// The time from one boundary to the next slot: the first period starts at
/// the document's start.
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Period {
    start: Boundary,
    end: Boundary,
    /// The slot that ends it.
    slot: Slot,
}

#[pymethods]
impl Period {
    fn __repr__(&self) -> String {
        format!("Period({} -> {})", self.start.at.to_rfc3339(), self.end.at.to_rfc3339())
    }
}

/// An iterator over a document's periods.
#[pyclass(module = "tkone.schedule")]
struct Periods(tkone_schedule::schedule::Periods<BusinessDays<tkone_calendar::Calendar>>);

#[pymethods]
impl Periods {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<Period>> {
        let next = self.0.next().map_err(schedule_err)?;
        Ok(next.map(|p| Period { start: p.start.into(), end: p.end.into(), slot: p.slot.into() }))
    }
}

/// Reads a document given as a `dict` or a JSON string.
fn parse_doc(doc: &Bound<'_, PyAny>) -> PyResult<ScheduleDoc> {
    let json: String = match doc.extract::<String>() {
        Ok(text) => text,
        Err(_) => doc.py().import("json")?.call_method1("dumps", (doc,))?.extract()?,
    };
    serde_json::from_str(&json).map_err(|e| ScheduleError::new_err(e.to_string()))
}

/// The built-in calendars, with `calendars` in place of those with their codes.
fn calendar_set(calendars: Vec<Calendar>) -> PyResult<Calendars> {
    let mut set = Calendars::builtin().clone();
    for calendar in calendars {
        set.insert(calendar.0).map_err(crate::calendar::calendar_err)?;
    }
    Ok(set)
}

fn to_json(doc: &ScheduleDoc) -> PyResult<String> {
    serde_json::to_string(doc).map_err(|e| ScheduleError::new_err(e.to_string()))
}

/// What `Document` and `Selector` read from their schedule.
mod read {
    use super::*;

    fn zoned(schedule: &DocSchedule, t: Aware) -> DateTime<chrono_tz::Tz> {
        t.in_zone(&schedule.zone())
    }

    pub(super) fn slots(schedule: &DocSchedule, after: Option<Aware>) -> PyResult<Slots> {
        let slots = match after {
            Some(after) => schedule.slots_after(&zoned(schedule, after)),
            None => schedule.slots(),
        };
        Ok(Slots(slots.map_err(schedule_err)?))
    }

    pub(super) fn take(schedule: &DocSchedule, n: usize, after: Option<Aware>) -> PyResult<Vec<Slot>> {
        let mut iter = slots(schedule, after)?;
        let mut out = Vec::new();
        while out.len() < n {
            match iter.__next__()? {
                Some(s) => out.push(s),
                None => break,
            }
        }
        Ok(out)
    }

    pub(super) fn between(schedule: &DocSchedule, start: Aware, end: Aware) -> PyResult<Slots> {
        let slots = schedule.slots_between(zoned(schedule, start)..=zoned(schedule, end)).map_err(schedule_err)?;
        Ok(Slots(slots))
    }

    pub(super) fn periods(schedule: &DocSchedule, start: Option<Aware>, end: Option<Aware>) -> PyResult<Periods> {
        let periods = match (start, end) {
            (None, None) => schedule.periods(),
            (start, Some(end)) => {
                let start = start.map(|s| zoned(schedule, s)).unwrap_or(*schedule.start());
                schedule.periods_between(start..=zoned(schedule, end))
            }
            (Some(_), None) => return Err(ScheduleError::new_err("periods from a `start` need an `end`")),
        };
        Ok(Periods(periods.map_err(schedule_err)?))
    }

    pub(super) fn moved_before_start(schedule: &DocSchedule) -> PyResult<Vec<Slot>> {
        Ok(schedule.moved_before_start().map_err(schedule_err)?.into_iter().map(Slot::from).collect())
    }
}

/// A schedule document: rules with their start, time zone, calendar (a
/// `tkone.calendar` code), end, count and exclusions. Built from a `dict`
/// or a JSON string; read YAML with a YAML library first.
///
/// `calendars`: `Calendar`s to use for their codes instead of the built-in
/// ones, such as a `Calendar.fixed("GBLO", ...)` in a test.
#[pyclass(module = "tkone.schedule", frozen)]
struct Document {
    doc: ScheduleDoc,
    schedule: DocSchedule,
}

#[pymethods]
impl Document {
    #[new]
    #[pyo3(signature = (doc, calendars=vec![]))]
    fn new(doc: &Bound<'_, PyAny>, calendars: Vec<Calendar>) -> PyResult<Self> {
        let doc = parse_doc(doc)?;
        let schedule = if calendars.is_empty() {
            doc.schedule(Calendars::builtin())
        } else {
            doc.schedule(&calendar_set(calendars)?)
        }
        .map_err(schedule_err)?;
        Ok(Self { doc, schedule })
    }

    /// An instance document read as a selection from this definition: see
    /// `Selector`.
    fn selector(slf: PyRef<'_, Self>, doc: &Bound<'_, PyAny>) -> PyResult<Selector> {
        Selector::new(doc, slf)
    }

    /// The start: the anchor, and the earliest a slot can be.
    #[getter]
    fn start(&self) -> DateTime<chrono_tz::Tz> {
        *self.schedule.start()
    }

    /// The last instant a slot can be at, if any.
    #[getter]
    fn until(&self) -> Option<DateTime<chrono_tz::Tz>> {
        self.schedule.until().copied()
    }

    /// The slots, in order: strictly after `after` (an aware `datetime`), or
    /// from the start.
    #[pyo3(signature = (after=None))]
    fn slots(&self, after: Option<Aware>) -> PyResult<Slots> {
        read::slots(&self.schedule, after)
    }

    /// The first `n` slots, as a list (see `slots`).
    #[pyo3(signature = (n, after=None))]
    fn take(&self, n: usize, after: Option<Aware>) -> PyResult<Vec<Slot>> {
        read::take(&self.schedule, n, after)
    }

    /// The slots from `start` to `end`, both included. Always ends, even
    /// with no `until`.
    fn between(&self, start: Aware, end: Aware) -> PyResult<Slots> {
        read::between(&self.schedule, start, end)
    }

    /// The periods, from one slot to the next: each with `start` and `end`
    /// boundaries, adjusted (`at`) and unadjusted (`actual`). Without `end`,
    /// every period (which ends only if the document does); with it, those
    /// that end from `start` (default: the document's start) to `end`.
    #[pyo3(signature = (start=None, end=None))]
    fn periods(&self, start: Option<Aware>, end: Option<Aware>) -> PyResult<Periods> {
        read::periods(&self.schedule, start, end)
    }

    /// The slots a rule put on or after the start that an adjustment moved
    /// before it, and so are dropped. For a loan, a payment that never
    /// happens; a test can require there are none.
    fn moved_before_start(&self) -> PyResult<Vec<Slot>> {
        read::moved_before_start(&self.schedule)
    }

    /// The document as JSON, with only the fields given.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.doc)
    }

    fn __repr__(&self) -> String {
        format!("Document({})", self.to_json().unwrap_or_default())
    }
}

// ── Selectors ────────────────────────────────────────────────────────────────

type DocSelector = tkone_schedule::schedule::Selector<BusinessDays<tkone_calendar::Calendar>>;

/// What `Selector.next_after` found: `kind` is `"due"` (with `slot`),
/// `"done"` (the selector's window has ended) or `"not_found"` (none the
/// definition produces before the horizon). `missed` holds the slots the
/// selector wanted on the way that the definition does not produce.
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Next {
    kind: &'static str,
    slot: Option<Slot>,
    missed: Vec<Slot>,
}

#[pymethods]
impl Next {
    fn __repr__(&self) -> String {
        match &self.slot {
            Some(slot) => format!("Next(due, {})", slot.at.to_rfc3339()),
            None => format!("Next({})", self.kind),
        }
    }
}

/// An instance: a document read as a selection from a definition's slots.
/// It never produces an instant the definition does not; one it wants that
/// the definition lacks is missed. Built with `Selector(doc, definition)` or
/// `definition.selector(doc)`; the document may not set `time_zone`,
/// `calendar` or `options`, which are the definition's.
#[pyclass(module = "tkone.schedule", frozen)]
struct Selector {
    doc: ScheduleDoc,
    selector: DocSelector,
    definition: DocSchedule,
}

#[pymethods]
impl Selector {
    #[new]
    fn new(doc: &Bound<'_, PyAny>, definition: PyRef<'_, Document>) -> PyResult<Self> {
        let doc = parse_doc(doc)?;
        let selector = doc.selector(&definition.schedule).map_err(schedule_err)?;
        Ok(Self { doc, selector, definition: definition.schedule.clone() })
    }

    /// The first slot strictly after `after`, and no later than `horizon`,
    /// that this selects and the definition produces.
    fn next_after(&self, after: Aware, horizon: Aware) -> PyResult<Next> {
        let zone = self.definition.zone();
        let mut missed = Vec::new();
        let next = self
            .selector
            .next_after(&self.definition, &after.in_zone(&zone), &horizon.in_zone(&zone), |slot| {
                missed.push(Slot::from(slot))
            })
            .map_err(schedule_err)?;
        use tkone_schedule::schedule::Next as N;
        Ok(match next {
            N::Due(slot) => Next { kind: "due", slot: Some(slot.into()), missed },
            N::Done => Next { kind: "done", slot: None, missed },
            N::NotFound => Next { kind: "not_found", slot: None, missed },
        })
    }

    /// The slots it wants from `start` to `end` that the definition does not
    /// produce: empty means it is a subset of the definition there.
    fn uncovered(&self, start: Aware, end: Aware) -> PyResult<Vec<Slot>> {
        let zone = self.definition.zone();
        let slots = self.selector.uncovered(&self.definition, start.in_zone(&zone)..=end.in_zone(&zone));
        Ok(slots.map_err(schedule_err)?.into_iter().map(Slot::from).collect())
    }

    /// The start of its window, and the anchor of its rules.
    #[getter]
    fn start(&self) -> DateTime<chrono_tz::Tz> {
        *self.selector.start()
    }

    /// The end of its window, if any; a `count` is here.
    #[getter]
    fn until(&self) -> Option<DateTime<chrono_tz::Tz>> {
        self.selector.until().copied()
    }

    /// The slots it wants, strictly after `after` or from the start, whether
    /// or not the definition produces them (see `uncovered`).
    #[pyo3(signature = (after=None))]
    fn slots(&self, after: Option<Aware>) -> PyResult<Slots> {
        read::slots(self.selector.schedule(), after)
    }

    /// The first `n` slots it wants, as a list (see `slots`).
    #[pyo3(signature = (n, after=None))]
    fn take(&self, n: usize, after: Option<Aware>) -> PyResult<Vec<Slot>> {
        read::take(self.selector.schedule(), n, after)
    }

    /// The slots it wants from `start` to `end`, both included.
    fn between(&self, start: Aware, end: Aware) -> PyResult<Slots> {
        read::between(self.selector.schedule(), start, end)
    }

    /// Its periods, from one slot it wants to the next; see `Document.periods`.
    #[pyo3(signature = (start=None, end=None))]
    fn periods(&self, start: Option<Aware>, end: Option<Aware>) -> PyResult<Periods> {
        read::periods(self.selector.schedule(), start, end)
    }

    /// See `Document.moved_before_start`.
    fn moved_before_start(&self) -> PyResult<Vec<Slot>> {
        read::moved_before_start(self.selector.schedule())
    }

    /// The document as JSON, with only the fields given.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.doc)
    }

    fn __repr__(&self) -> String {
        format!("Selector({})", self.to_json().unwrap_or_default())
    }
}

// ── Bundles ──────────────────────────────────────────────────────────────────

/// Documents that refer to each other (`{collections.repayment}`), built
/// together as a set: all of them, or none.
///
/// - `definitions`: `{name: doc}`.
/// - `instances`: `{name: (doc, definition)}`, each `definition` a
///   `Document` outside the set that the instance selects from.
/// - `calendars`: as for `Document`.
///
/// References stay in the set, never loop, and a definition refers only to
/// definitions. A reference to an instance follows the slots it is due at.
/// `Bundle(instances=...)`: each name with its document and definition.
type InstanceArgs<'py> = std::collections::BTreeMap<String, (Bound<'py, PyAny>, PyRef<'py, Document>)>;

#[pyclass(module = "tkone.schedule", frozen)]
struct Bundle {
    docs: std::collections::BTreeMap<String, ScheduleDoc>,
    bundle: tkone_schedule::schedule::Bundle<BusinessDays<tkone_calendar::Calendar>>,
}

#[pymethods]
impl Bundle {
    #[new]
    #[pyo3(signature = (definitions=None, instances=None, calendars=vec![]))]
    fn new(
        definitions: Option<std::collections::BTreeMap<String, Bound<'_, PyAny>>>,
        instances: Option<InstanceArgs<'_>>,
        calendars: Vec<Calendar>,
    ) -> PyResult<Self> {
        let set = calendar_set(calendars)?;
        let mut builder = tkone_schedule::schedule::Bundle::builder();
        let mut docs = std::collections::BTreeMap::new();
        for (name, doc) in definitions.unwrap_or_default() {
            let doc = parse_doc(&doc)?;
            builder = builder.definition(name.clone(), doc.schedule_builder(&set).map_err(schedule_err)?);
            docs.insert(name, doc);
        }
        for (name, (doc, definition)) in instances.unwrap_or_default() {
            let doc = parse_doc(&doc)?;
            let selector = doc.selector_builder(&definition.schedule).map_err(schedule_err)?;
            builder = builder.instance(name.clone(), &definition.schedule, selector);
            docs.insert(name, doc);
        }
        Ok(Self { docs, bundle: builder.build().map_err(schedule_err)? })
    }

    /// The documents' names, in order.
    #[getter]
    fn names(&self) -> Vec<String> {
        self.bundle.names().map(str::to_string).collect()
    }

    /// The definition called `name`, as a `Document`.
    fn document(&self, name: &str) -> PyResult<Document> {
        match self.bundle.member(name) {
            Some(tkone_schedule::schedule::bundle::Member::Definition(schedule)) => {
                Ok(Document { doc: self.docs[name].clone(), schedule: schedule.clone() })
            }
            Some(_) => Err(ScheduleError::new_err(format!("`{name}` is an instance: use `selector`"))),
            None => Err(ScheduleError::new_err(format!("there is no document `{name}`"))),
        }
    }

    /// The instance called `name`, as a `Selector`.
    fn selector(&self, name: &str) -> PyResult<Selector> {
        match self.bundle.instance(name) {
            Some((selector, definition)) => Ok(Selector {
                doc: self.docs[name].clone(),
                selector: selector.clone(),
                definition: definition.clone(),
            }),
            None if self.bundle.member(name).is_some() => {
                Err(ScheduleError::new_err(format!("`{name}` is a definition: use `document`")))
            }
            None => Err(ScheduleError::new_err(format!("there is no document `{name}`"))),
        }
    }

    fn __repr__(&self) -> String {
        format!("Bundle({:?})", self.names())
    }
}

// ── Timelines ────────────────────────────────────────────────────────────────

/// One document's slot in a `Moment`, with `since`: the boundary before it
/// (its previous slot, or the document's start).
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Entry {
    key: String,
    slot: Slot,
    since: Boundary,
}

#[pymethods]
impl Entry {
    fn __repr__(&self) -> String {
        format!("Entry({:?}, {})", self.key, self.slot.at.to_rfc3339())
    }
}

/// One instant on a `Timeline`, with every document that has a slot at it,
/// in the order they were added.
#[pyclass(module = "tkone.schedule", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Moment {
    at: DateTime<chrono_tz::Tz>,
    entries: Vec<Entry>,
}

#[pymethods]
impl Moment {
    fn __repr__(&self) -> String {
        let keys: Vec<_> = self.entries.iter().map(|e| e.key.as_str()).collect();
        format!("Moment({}, {})", self.at.to_rfc3339(), keys.join("+"))
    }
}

/// An iterator over a timeline's moments.
#[pyclass(module = "tkone.schedule")]
struct Moments(tkone_schedule::schedule::Moments<String, BusinessDays<tkone_calendar::Calendar>>);

#[pymethods]
impl Moments {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> PyResult<Option<Moment>> {
        let next = self.0.next().map_err(schedule_err)?;
        Ok(next.map(|m| Moment {
            at: m.at,
            entries: m
                .entries
                .into_iter()
                .map(|e| Entry { key: e.key, slot: e.slot.into(), since: e.since.into() })
                .collect(),
        }))
    }
}

/// Several documents read as one: `Timeline([("fixing", fixings),
/// ("repayment", repayments)])`. Each keeps its own start, zone, calendar
/// and end. Within a moment, entries come in the order given here.
#[pyclass(module = "tkone.schedule", frozen)]
struct Timeline(tkone_schedule::schedule::Timeline<String, BusinessDays<tkone_calendar::Calendar>>);

#[pymethods]
impl Timeline {
    #[new]
    fn new(lanes: Vec<(String, Bound<'_, PyAny>)>) -> PyResult<Self> {
        let mut timeline = tkone_schedule::schedule::Timeline::new();
        for (key, lane) in lanes {
            let schedule = if let Ok(doc) = lane.cast::<Document>() {
                doc.get().schedule.clone()
            } else if let Ok(selector) = lane.cast::<Selector>() {
                selector.get().selector.schedule().clone()
            } else {
                return Err(ScheduleError::new_err(format!("`{key}`: a lane is a `Document` or a `Selector`")));
            };
            timeline = timeline.add(key, schedule);
        }
        Ok(Self(timeline))
    }

    /// The keys, in the order given.
    #[getter]
    fn keys(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }

    /// The moments from `start` to `end`, both included. Always ends.
    fn between(&self, start: Aware, end: Aware) -> PyResult<Moments> {
        let utc = |t: Aware| t.in_zone(&chrono_tz::UTC);
        Ok(Moments(self.0.between(utc(start)..=utc(end)).map_err(schedule_err)?))
    }

    fn __repr__(&self) -> String {
        format!("Timeline({:?})", self.keys())
    }
}

/// The spec in plain English.
#[pyfunction]
fn describe(spec: &str) -> PyResult<String> {
    Schedule::new(spec)?.describe()
}

/// Registers `tkone.schedule` on `parent`.
pub(crate) fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new(py, "schedule")?;
    m.add_class::<Schedule>()?;
    m.add_class::<Occurrence>()?;
    m.add_class::<Occurrences>()?;
    m.add_class::<Document>()?;
    m.add_class::<Slot>()?;
    m.add_class::<Source>()?;
    m.add_class::<Slots>()?;
    m.add_class::<Boundary>()?;
    m.add_class::<Period>()?;
    m.add_class::<Periods>()?;
    m.add_class::<Timeline>()?;
    m.add_class::<Selector>()?;
    m.add_class::<Next>()?;
    m.add_class::<Bundle>()?;
    m.add_class::<Moment>()?;
    m.add_class::<Entry>()?;
    m.add_class::<Moments>()?;
    m.add_function(wrap_pyfunction!(describe, &m)?)?;
    let err = py.get_type::<ScheduleError>();
    err.setattr("__module__", "tkone.schedule")?;
    m.add("ScheduleError", err)?;
    parent.add_submodule(&m)
}
