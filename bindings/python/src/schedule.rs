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
        after: Option<DateTime<FixedOffset>>,
        calendar: Option<&Bound<'_, PyAny>>,
        tz: Option<&str>,
    ) -> PyResult<Occurrences> {
        let zone = match tz {
            Some(name) => name.parse::<chrono_tz::Tz>().map_err(|_| ScheduleError::new_err(format!("`{name}` is not an IANA time zone")))?,
            None => chrono_tz::UTC,
        };
        let after = after.map(|a| a.with_timezone(&zone)).unwrap_or_else(|| Utc::now().with_timezone(&zone));
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
        after: Option<DateTime<FixedOffset>>,
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

/// A schedule document: rules with their start, time zone, calendar (a
/// `tkone.calendar` code), end, count and exclusions. Built from a `dict`
/// or a JSON string; read YAML with a YAML library first.
#[pyclass(module = "tkone.schedule", frozen)]
struct Document {
    doc: ScheduleDoc,
    schedule: DocSchedule,
}

#[pymethods]
impl Document {
    #[new]
    fn new(doc: &Bound<'_, PyAny>) -> PyResult<Self> {
        let json: String = match doc.extract::<String>() {
            Ok(text) => text,
            Err(_) => doc.py().import("json")?.call_method1("dumps", (doc,))?.extract()?,
        };
        let doc: ScheduleDoc = serde_json::from_str(&json).map_err(|e| ScheduleError::new_err(e.to_string()))?;
        let schedule = doc.schedule(Calendars::builtin()).map_err(schedule_err)?;
        Ok(Self { doc, schedule })
    }

    /// The slots, in order: strictly after `after` (an aware `datetime`), or
    /// from the start.
    #[pyo3(signature = (after=None))]
    fn slots(&self, after: Option<DateTime<FixedOffset>>) -> PyResult<Slots> {
        let slots = match after {
            Some(after) => self.schedule.slots_after(&after.with_timezone(&self.schedule.zone())),
            None => self.schedule.slots(),
        };
        Ok(Slots(slots.map_err(schedule_err)?))
    }

    /// The first `n` slots, as a list (see `slots`).
    #[pyo3(signature = (n, after=None))]
    fn take(&self, n: usize, after: Option<DateTime<FixedOffset>>) -> PyResult<Vec<Slot>> {
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
    m.add_function(wrap_pyfunction!(describe, &m)?)?;
    let err = py.get_type::<ScheduleError>();
    err.setattr("__module__", "tkone.schedule")?;
    m.add("ScheduleError", err)?;
    parent.add_submodule(&m)
}
