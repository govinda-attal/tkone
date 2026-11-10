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
        let looks = if spec.contains(':') && spec.split(':').next().is_some_and(|h| h.contains('-') && h.contains('T')) {
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
            Some(name) => name.parse::<chrono_tz::Tz>().map_err(|_| ScheduleError::new_err(format!("`{name}` is not an IANA time zone")))?,
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
        let sources = slot.sources.into_iter().map(|s| Source { rule_id: s.rule_id, actual: s.actual, basis: s.basis }).collect();
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

impl Document {
    fn zoned(&self, t: Aware) -> DateTime<chrono_tz::Tz> {
        t.in_zone(&self.schedule.zone())
    }
}

#[pymethods]
impl Document {
    #[new]
    #[pyo3(signature = (doc, calendars=vec![]))]
    fn new(doc: &Bound<'_, PyAny>, calendars: Vec<Calendar>) -> PyResult<Self> {
        let json: String = match doc.extract::<String>() {
            Ok(text) => text,
            Err(_) => doc.py().import("json")?.call_method1("dumps", (doc,))?.extract()?,
        };
        let doc: ScheduleDoc = serde_json::from_str(&json).map_err(|e| ScheduleError::new_err(e.to_string()))?;
        let schedule = if calendars.is_empty() {
            doc.schedule(Calendars::builtin())
        } else {
            let mut set = Calendars::builtin().clone();
            for calendar in calendars {
                set.insert(calendar.0).map_err(crate::calendar::calendar_err)?;
            }
            doc.schedule(&set)
        }
        .map_err(schedule_err)?;
        Ok(Self { doc, schedule })
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

    /// The slots from `start` to `end`, both included. Always ends, even
    /// with no `until`.
    fn between(&self, start: Aware, end: Aware) -> PyResult<Slots> {
        let slots = self.schedule.slots_between(self.zoned(start)..=self.zoned(end)).map_err(schedule_err)?;
        Ok(Slots(slots))
    }

    /// The periods, from one slot to the next: each with `start` and `end`
    /// boundaries, adjusted (`at`) and unadjusted (`actual`). Without `end`,
    /// every period (which ends only if the document does); with it, those
    /// that end from `start` (default: the document's start) to `end`.
    #[pyo3(signature = (start=None, end=None))]
    fn periods(&self, start: Option<Aware>, end: Option<Aware>) -> PyResult<Periods> {
        let periods = match (start, end) {
            (None, None) => self.schedule.periods(),
            (start, Some(end)) => {
                let start = start.map(|s| self.zoned(s)).unwrap_or(*self.schedule.start());
                self.schedule.periods_between(start..=self.zoned(end))
            }
            (Some(_), None) => return Err(ScheduleError::new_err("periods from a `start` need an `end`")),
        };
        Ok(Periods(periods.map_err(schedule_err)?))
    }

    /// The slots a rule put on or after the start that an adjustment moved
    /// before it, and so are dropped. For a loan, a payment that never
    /// happens; a test can require there are none.
    fn moved_before_start(&self) -> PyResult<Vec<Slot>> {
        Ok(self.schedule.moved_before_start().map_err(schedule_err)?.into_iter().map(Slot::from).collect())
    }

    /// The slots, in order: strictly after `after` (an aware `datetime`), or
    /// from the start.
    #[pyo3(signature = (after=None))]
    fn slots(&self, after: Option<Aware>) -> PyResult<Slots> {
        let slots = match after {
            Some(after) => self.schedule.slots_after(&self.zoned(after)),
            None => self.schedule.slots(),
        };
        Ok(Slots(slots.map_err(schedule_err)?))
    }

    /// The first `n` slots, as a list (see `slots`).
    #[pyo3(signature = (n, after=None))]
    fn take(&self, n: usize, after: Option<Aware>) -> PyResult<Vec<Slot>> {
        let mut iter = self.slots(after)?;
        let mut out = Vec::new();
        while out.len() < n {
            match iter.__next__()? {
                Some(s) => out.push(s),
                None => break,
            }
        }
        Ok(out)
    }

    /// The document as JSON, with only the fields given.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.doc).map_err(|e| ScheduleError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("Document({})", self.to_json().unwrap_or_default())
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
    fn new(lanes: Vec<(String, PyRef<'_, Document>)>) -> Self {
        let timeline = lanes
            .iter()
            .fold(tkone_schedule::schedule::Timeline::new(), |t, (key, doc)| t.add(key.clone(), doc.schedule.clone()));
        Self(timeline)
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
    m.add_class::<Moment>()?;
    m.add_class::<Entry>()?;
    m.add_class::<Moments>()?;
    m.add_function(wrap_pyfunction!(describe, &m)?)?;
    let err = py.get_type::<ScheduleError>();
    err.setattr("__module__", "tkone.schedule")?;
    m.add("ScheduleError", err)?;
    parent.add_submodule(&m)
}
