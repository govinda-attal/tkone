//! `tkone.calendar`: bindings for `tkone-calendar`.

use chrono::NaiveDate;
use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tkone_calendar::Calendars;

create_exception!(tkone, CalendarError, PyValueError, "A calendar is unknown, or a date is outside it.");

pub(crate) fn calendar_err(e: tkone_calendar::Error) -> PyErr {
    CalendarError::new_err(e.to_string())
}

/// A holiday calendar, such as London (`GBLO`) or a joint one (`GBLO+USNY`).
#[pyclass(module = "tkone.calendar", frozen, from_py_object)]
#[derive(Clone)]
pub(crate) struct Calendar(pub(crate) tkone_calendar::Calendar);

/// One holiday: its name, the date it falls on, and its day off.
#[pyclass(module = "tkone.calendar", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
struct Holiday {
    name: String,
    actual: NaiveDate,
    observed: NaiveDate,
}

#[pymethods]
impl Holiday {
    fn __repr__(&self) -> String {
        format!("Holiday({:?}, actual={}, observed={})", self.name, self.actual, self.observed)
    }
}

impl Holiday {
    fn of(h: &tkone_calendar::Holiday) -> Self {
        Holiday { name: h.name.clone(), actual: h.actual, observed: h.observed }
    }
}

#[pymethods]
impl Calendar {
    /// The calendar with this code; codes joined with `+` give a joint one.
    #[new]
    pub(crate) fn new(code: &str) -> PyResult<Self> {
        Calendars::builtin().get(code).map(Self).map_err(calendar_err)
    }

    /// A calendar written out in full, for tests: the dates from `valid_from`
    /// to `valid_until`, with `weekend` (day names, `"Sat"` or `"Saturday"`)
    /// and `days_off` as the only days that are not business days. It never
    /// changes, unlike the built-in data.
    #[staticmethod]
    #[pyo3(signature = (code, valid_from, valid_until, weekend=vec!["Sat".to_string(), "Sun".to_string()], days_off=vec![]))]
    fn fixed(
        code: &str,
        valid_from: NaiveDate,
        valid_until: NaiveDate,
        weekend: Vec<String>,
        days_off: Vec<NaiveDate>,
    ) -> PyResult<Self> {
        let weekend = weekend
            .iter()
            .map(|day| day.parse::<chrono::Weekday>().map_err(|_| CalendarError::new_err(format!("`{day}` is not a day of the week"))))
            .collect::<PyResult<Vec<_>>>()?;
        tkone_calendar::Calendar::fixed(code, valid_from..=valid_until, &weekend, days_off).map(Self).map_err(calendar_err)
    }

    /// This calendar with `dates` as days off too: a closure at short notice.
    /// The code and dates covered stay the same.
    fn with_days_off(&self, dates: Vec<NaiveDate>) -> PyResult<Self> {
        self.0.with_days_off(dates).map(Self).map_err(calendar_err)
    }

    #[getter]
    fn code(&self) -> &str {
        self.0.code()
    }
    #[getter]
    fn name(&self) -> &str {
        self.0.name()
    }
    /// The IANA time zone, or `None` for a joint calendar across zones.
    #[getter]
    fn zone(&self) -> Option<&str> {
        self.0.zone()
    }
    #[getter]
    fn valid_from(&self) -> NaiveDate {
        self.0.valid_from()
    }
    #[getter]
    fn valid_until(&self) -> NaiveDate {
        self.0.valid_until()
    }
    #[getter]
    fn confirmed_until(&self) -> NaiveDate {
        self.0.confirmed_until()
    }

    fn is_business_day(&self, day: NaiveDate) -> PyResult<bool> {
        self.0.is_business_day(day).map_err(calendar_err)
    }
    fn is_holiday(&self, day: NaiveDate) -> PyResult<bool> {
        self.0.is_holiday(day).map_err(calendar_err)
    }
    /// `"confirmed"` up to `confirmed_until`, `"projected"` after it.
    fn status(&self, day: NaiveDate) -> PyResult<&'static str> {
        Ok(match self.0.status(day).map_err(calendar_err)? {
            tkone_calendar::Status::Confirmed => "confirmed",
            tkone_calendar::Status::Projected => "projected",
        })
    }
    /// The holidays whose day off is in `year`.
    fn holidays(&self, year: i32) -> Vec<Holiday> {
        self.0.holidays(year).map(Holiday::of).collect()
    }
    /// The holidays whose day off is from `start` to `end`, inclusive: to copy
    /// a built-in calendar's days off into a fixed one.
    fn holidays_between(&self, start: NaiveDate, end: NaiveDate) -> Vec<Holiday> {
        self.0.holidays_between(start, end).map(Holiday::of).collect()
    }
    fn __repr__(&self) -> String {
        format!("Calendar({:?})", self.0.code())
    }
}

/// The codes of the built-in calendars.
#[pyfunction]
fn calendar_codes() -> Vec<String> {
    Calendars::builtin().codes().map(str::to_string).collect()
}

/// Registers `tkone.calendar` on `parent`.
pub(crate) fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new(py, "calendar")?;
    m.add_class::<Calendar>()?;
    m.add_class::<Holiday>()?;
    m.add_function(wrap_pyfunction!(calendar_codes, &m)?)?;
    let err = py.get_type::<CalendarError>();
    err.setattr("__module__", "tkone.calendar")?;
    m.add("CalendarError", err)?;
    parent.add_submodule(&m)
}
