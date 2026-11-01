use std::fmt;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::model::{CalendarFile, Kind};
use crate::{Error, Result};

/// One holiday in a calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holiday {
    /// The holiday's name.
    pub name: String,
    /// The date the holiday falls on.
    pub actual: NaiveDate,
    /// The day off: the actual date, or the weekday it moved to when it fell
    /// on a weekend.
    pub observed: NaiveDate,
}

/// Whether a date's holiday data has been checked against published sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// On or before the calendar's `confirmed_until`.
    Confirmed,
    /// After it: the rules apply, but one-off holidays not yet announced are
    /// missing.
    Projected,
}

/// The days of the week that are never business days.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Weekend([bool; 7]);

impl Weekend {
    pub(crate) fn contains(&self, wd: Weekday) -> bool {
        self.0[wd.num_days_from_monday() as usize]
    }

    pub(crate) fn insert(&mut self, wd: Weekday) {
        self.0[wd.num_days_from_monday() as usize] = true;
    }

    fn union(&self, other: &Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] || other.0[i]))
    }
}

/// A holiday calendar: which dates are business days between its
/// `valid_from` and `valid_until`.
///
/// Cheap to clone. Use it as a
/// [`BizDayProcessor`](tkone_schedule::biz_day::BizDayProcessor) to run
/// schedules on its business days. A date outside its range is an error,
/// never a guess.
#[derive(Clone)]
pub struct Calendar {
    inner: Arc<Inner>,
}

struct Inner {
    code: String,
    name: String,
    kind: Option<Kind>,
    zone: Option<String>,
    weekend: Weekend,
    valid_from: NaiveDate,
    valid_until: NaiveDate,
    confirmed_until: NaiveDate,
    /// Sorted by observed date.
    holidays: Vec<Holiday>,
    /// One bit per day from `valid_from`: a holiday's day off.
    off: Vec<u64>,
}

impl fmt::Debug for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Calendar")
            .field("code", &self.inner.code)
            .field("valid_from", &self.inner.valid_from)
            .field("valid_until", &self.inner.valid_until)
            .finish_non_exhaustive()
    }
}

impl Calendar {
    pub(crate) fn new(
        code: String,
        name: String,
        kind: Option<Kind>,
        zone: Option<String>,
        weekend: Weekend,
        (valid_from, valid_until, confirmed_until): (NaiveDate, NaiveDate, NaiveDate),
        mut holidays: Vec<Holiday>,
    ) -> Self {
        holidays.sort_by(|a, b| (a.observed, a.actual, &a.name).cmp(&(b.observed, b.actual, &b.name)));
        // A joint calendar gets the same holiday from each of its calendars.
        holidays.dedup();
        let days = (valid_until - valid_from).num_days() as usize + 1;
        let mut off = vec![0u64; days.div_ceil(64)];
        for h in &holidays {
            let i = (h.observed - valid_from).num_days() as usize;
            off[i / 64] |= 1 << (i % 64);
        }
        Self {
            inner: Arc::new(Inner {
                code,
                name,
                kind,
                zone,
                weekend,
                valid_from,
                valid_until,
                confirmed_until,
                holidays,
                off,
            }),
        }
    }

    /// Checks a calendar file and builds the calendar. Every problem in the
    /// file is reported at once in [`Error::Invalid`].
    pub fn from_file(file: &CalendarFile) -> Result<Self> {
        crate::resolve::build(file)
    }

    /// Reads and builds one calendar from YAML.
    #[cfg(feature = "yaml")]
    pub fn from_yaml_str(yaml: &str) -> Result<Self> {
        let file: CalendarFile = serde_yaml_ng::from_str(yaml).map_err(|e| Error::Parse(e.to_string()))?;
        Self::from_file(&file)
    }

    /// A calendar whose days off are those of all the `calendars`: a business
    /// day in it is a business day in every one. It covers the dates they all
    /// cover. Its code joins theirs with `+` (`GBLO+USNY`).
    pub fn joint(calendars: &[Calendar]) -> Result<Self> {
        let [first, rest @ ..] = calendars else {
            return Err(Error::Unknown(String::new()));
        };
        if rest.is_empty() {
            return Ok(first.clone());
        }
        let code = calendars.iter().map(|c| c.code()).collect::<Vec<_>>().join("+");
        let valid_from = calendars.iter().map(|c| c.valid_from()).max().unwrap();
        let valid_until = calendars.iter().map(|c| c.valid_until()).min().unwrap();
        let confirmed_until = calendars.iter().map(|c| c.confirmed_until()).min().unwrap();
        if valid_from > valid_until {
            return Err(Error::Invalid { code, problems: vec!["the calendars cover no date in common".into()] });
        }
        let weekend = calendars.iter().fold(Weekend::default(), |w, c| w.union(&c.inner.weekend));
        let holidays = calendars
            .iter()
            .flat_map(|c| c.inner.holidays.iter())
            .filter(|h| (valid_from..=valid_until).contains(&h.observed))
            .cloned()
            .collect();
        let name = calendars.iter().map(|c| c.name()).collect::<Vec<_>>().join(" + ");
        let kind = calendars.iter().map(|c| c.kind()).reduce(|a, b| if a == b { a } else { None }).flatten();
        let zone = calendars.iter().map(|c| c.zone()).reduce(|a, b| if a == b { a } else { None }).flatten();
        Ok(Self::new(
            code,
            name,
            kind,
            zone.map(str::to_string),
            weekend,
            (valid_from, valid_until, confirmed_until.max(valid_from).min(valid_until)),
            holidays,
        ))
    }

    /// The calendar's code, such as `GBLO`.
    pub fn code(&self) -> &str {
        &self.inner.code
    }

    /// The calendar's name.
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// What kind of calendar it is; `None` for a joint calendar of different
    /// kinds.
    pub fn kind(&self) -> Option<Kind> {
        self.inner.kind
    }

    /// The calendar's IANA time zone; `None` for a joint calendar of
    /// different zones.
    pub fn zone(&self) -> Option<&str> {
        self.inner.zone.as_deref()
    }

    /// The first date the calendar answers for.
    pub fn valid_from(&self) -> NaiveDate {
        self.inner.valid_from
    }

    /// The last date the calendar answers for.
    pub fn valid_until(&self) -> NaiveDate {
        self.inner.valid_until
    }

    /// The last date checked against published sources.
    pub fn confirmed_until(&self) -> NaiveDate {
        self.inner.confirmed_until
    }

    fn index(&self, date: NaiveDate) -> Result<usize> {
        if !(self.inner.valid_from..=self.inner.valid_until).contains(&date) {
            return Err(Error::OutOfRange {
                code: self.inner.code.clone(),
                date,
                from: self.inner.valid_from,
                until: self.inner.valid_until,
            });
        }
        Ok((date - self.inner.valid_from).num_days() as usize)
    }

    /// Whether `date` is a weekend day. Defined for every date.
    pub fn is_weekend(&self, date: NaiveDate) -> bool {
        self.inner.weekend.contains(date.weekday())
    }

    /// Whether `date` is a holiday's day off (weekends aside).
    pub fn is_holiday(&self, date: NaiveDate) -> Result<bool> {
        let i = self.index(date)?;
        Ok(self.inner.off[i / 64] & (1 << (i % 64)) != 0)
    }

    /// Whether `date` is a business day: neither a weekend day nor a
    /// holiday's day off.
    pub fn is_business_day(&self, date: NaiveDate) -> Result<bool> {
        Ok(!self.is_holiday(date)? && !self.is_weekend(date))
    }

    /// Whether the data for `date` is confirmed or projected.
    pub fn status(&self, date: NaiveDate) -> Result<Status> {
        self.index(date)?;
        Ok(if date <= self.inner.confirmed_until { Status::Confirmed } else { Status::Projected })
    }

    /// The holidays whose day off is in `year`, in date order.
    pub fn holidays(&self, year: i32) -> impl Iterator<Item = &Holiday> {
        self.inner.holidays.iter().filter(move |h| h.observed.year() == year)
    }

    /// Every holiday whose day off is between `from` and `to`, inclusive.
    pub fn holidays_between(&self, from: NaiveDate, to: NaiveDate) -> impl Iterator<Item = &Holiday> {
        self.inner.holidays.iter().filter(move |h| (from..=to).contains(&h.observed))
    }
}
