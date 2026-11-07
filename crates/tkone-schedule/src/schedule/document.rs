//! Schedule documents: a [`Schedule`] as data, in YAML, JSON or any serde
//! format.
//!
//! ```rust
//! use tkone_schedule::schedule::document::{CalendarResolver, ScheduleDoc};
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use fallible_iterator::FallibleIterator;
//!
//! let doc: ScheduleDoc = serde_json::from_str(r#"{
//!     "version": 1,
//!     "time_zone": "Europe/London",
//!     "calendar": "WEEKDAYS",
//!     "start": "2026-05-01",
//!     "rules": [{ "id": "month-end", "rule": "YY-MM-L~PBT18:00:00" }]
//! }"#)?;
//!
//! // Turns calendar codes into business days; tkone-calendar's `Calendars`
//! // is one.
//! struct Weekdays;
//! impl CalendarResolver for Weekdays {
//!     type Bdp = WeekendSkipper;
//!     fn resolve(&self, code: &str) -> tkone_schedule::Result<WeekendSkipper> {
//!         match code {
//!             "WEEKDAYS" => Ok(WeekendSkipper::new()),
//!             _ => Err(tkone_schedule::Error::Calendar(format!("no calendar {code}"))),
//!         }
//!     }
//! }
//!
//! let schedule = doc.schedule(&Weekdays)?;
//! let slot = schedule.slots()?.next()?.unwrap();
//! assert_eq!(slot.at.to_string(), "2026-05-29 18:00:00 BST");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! See `docs/schedule-document.md` in the repository for the fields and the
//! decisions behind them.

use std::fmt;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{rule_uses_business_days, ExcludeMatch, Schedule, ScheduleBuilder, Selector, SelectorBuilder};
use crate::biz_day::{BizDayProcessor, Direction, WeekendSkipper};
use crate::utils::just_before;
use crate::utils::resolve_local;
use crate::{DstPolicy, Error, IntervalMode, Result, TimeAnchor};

/// The document format version this crate reads.
pub const VERSION: u32 = 1;

/// The calendar code for Saturday and Sunday off, and no holidays.
pub const WEEKENDS: &str = "WEEKENDS";

/// A schedule as data. See the [module documentation](self).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleDoc {
    /// The format version: [`VERSION`].
    pub version: u32,
    /// The IANA time zone everything is read in (`Europe/London`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// The business days: a calendar code (`GBLO`, `GBLO+USNY`) for the
    /// [`CalendarResolver`], or [`WEEKENDS`]. Required when a rule uses
    /// business days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<String>,
    /// The anchor and earliest slot.
    pub start: LocalTime,
    /// The last slot (inclusive); a date means the whole of that day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<LocalTime>,
    /// Stop after this many slots, counted after exclusions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    /// The rules, at least one.
    pub rules: Vec<RuleDoc>,
    /// Dates with no slots.
    #[serde(default, skip_serializing_if = "ExcludeDoc::is_empty")]
    pub exclude: ExcludeDoc,
    /// Iteration options.
    #[serde(default, skip_serializing_if = "OptionsDoc::is_empty")]
    pub options: OptionsDoc,
}

/// One rule of a [`ScheduleDoc`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDoc {
    /// Carried on every slot the rule produces; unique in the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// A datetime rule with nothing around it (`YY-MM-L~PBT18:00:00`).
    pub rule: String,
}

/// The exclusions of a [`ScheduleDoc`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludeDoc {
    /// Dates with no slots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dates: Vec<NaiveDate>,
    /// Date specs (`YY-12-24`) whose dates have no slots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    /// Which date exclusions are tested against.
    #[serde(default, rename = "match", skip_serializing_if = "is_default")]
    pub by: ExcludeMatch,
}

impl ExcludeDoc {
    fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// The iteration options of a [`ScheduleDoc`]; each defaults as on the
/// builder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionsDoc {
    /// See [`DstPolicy`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_policy: Option<DstPolicy>,
    /// See [`IntervalMode`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_mode: Option<IntervalMode>,
    /// See [`TimeAnchor`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_anchor: Option<TimeAnchor>,
}

impl OptionsDoc {
    fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

/// A local date (`2026-01-02`) or date and time (`2026-01-02T09:30:00`),
/// with no UTC offset: the document's `time_zone` says where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTime {
    /// A whole day.
    Date(NaiveDate),
    /// A wall-clock time.
    DateTime(NaiveDateTime),
}

impl LocalTime {
    fn parse(s: &str) -> std::result::Result<Self, String> {
        if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Ok(LocalTime::Date(date));
        }
        for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
            if let Ok(dtm) = NaiveDateTime::parse_from_str(s, format) {
                return Ok(LocalTime::DateTime(dtm));
            }
        }
        if DateTime::parse_from_rfc3339(s).is_ok() {
            return Err(format!("`{s}` has a UTC offset; write the local time, the zone is `time_zone`"));
        }
        Err(format!("`{s}` is not a date (2026-01-02) or a local date and time (2026-01-02T09:30:00)"))
    }

    /// The first instant of it in `zone`.
    fn first(self, zone: &Tz, policy: DstPolicy) -> Result<DateTime<Tz>> {
        let local = match self {
            LocalTime::Date(date) => date.and_time(NaiveTime::MIN),
            LocalTime::DateTime(dtm) => dtm,
        };
        resolve_local(zone, local, policy)
    }

    /// The last instant of it in `zone`: the end of a day, or the time itself.
    fn last(self, zone: &Tz, policy: DstPolicy) -> Result<DateTime<Tz>> {
        match self {
            LocalTime::Date(date) => {
                let next = date.succ_opt().ok_or(Error::IteratorNotConverged)?;
                Ok(just_before(&resolve_local(zone, next.and_time(NaiveTime::MIN), policy)?))
            }
            LocalTime::DateTime(dtm) => resolve_local(zone, dtm, policy),
        }
    }
}

impl fmt::Display for LocalTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocalTime::Date(date) => write!(f, "{}", date.format("%Y-%m-%d")),
            LocalTime::DateTime(dtm) => write!(f, "{}", dtm.format("%Y-%m-%dT%H:%M:%S")),
        }
    }
}

impl Serialize for LocalTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LocalTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        LocalTime::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Turns a document's calendar code into business days.
///
/// tkone-calendar's `Calendars` implements it for its calendars; an
/// application with calendars of its own implements it over them. Codes are
/// as written in the document (`GBLO`, `GBLO+USNY`); [`WEEKENDS`] never
/// reaches the resolver.
pub trait CalendarResolver {
    /// The business days a code resolves to.
    type Bdp: BizDayProcessor;

    /// The business days of `code`.
    ///
    /// # Errors
    ///
    /// When `code` names no calendar the resolver knows.
    fn resolve(&self, code: &str) -> Result<Self::Bdp>;
}

/// The business days of a schedule built from a document: a resolved
/// calendar, or Saturday and Sunday off.
#[derive(Debug, Clone)]
pub enum BusinessDays<B> {
    /// A calendar from the [`CalendarResolver`].
    Calendar(B),
    /// [`WEEKENDS`].
    Weekends(WeekendSkipper),
    /// No calendar: nothing in the document uses business days. An instance
    /// document read against it may not either.
    NoCalendar(WeekendSkipper),
}


impl<B: BizDayProcessor> BizDayProcessor for BusinessDays<B> {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool> {
        match self {
            BusinessDays::Calendar(bdp) => bdp.is_biz_day(dtm),
            BusinessDays::Weekends(bdp) | BusinessDays::NoCalendar(bdp) => bdp.is_biz_day(dtm),
        }
    }

    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime> {
        match self {
            BusinessDays::Calendar(bdp) => bdp.find_biz_day(dtm, direction),
            BusinessDays::Weekends(bdp) | BusinessDays::NoCalendar(bdp) => bdp.find_biz_day(dtm, direction),
        }
    }

    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        match self {
            BusinessDays::Calendar(bdp) => bdp.add(dtm, num),
            BusinessDays::Weekends(bdp) | BusinessDays::NoCalendar(bdp) => bdp.add(dtm, num),
        }
    }

    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        match self {
            BusinessDays::Calendar(bdp) => bdp.sub(dtm, num),
            BusinessDays::Weekends(bdp) | BusinessDays::NoCalendar(bdp) => bdp.sub(dtm, num),
        }
    }

    fn code(&self) -> Option<String> {
        match self {
            BusinessDays::Calendar(bdp) => bdp.code(),
            BusinessDays::Weekends(_) => Some(WEEKENDS.into()),
            BusinessDays::NoCalendar(_) => None,
        }
    }

    fn time_zone(&self) -> Option<Tz> {
        match self {
            BusinessDays::Calendar(bdp) => bdp.time_zone(),
            BusinessDays::Weekends(_) | BusinessDays::NoCalendar(_) => None,
        }
    }
}

impl ScheduleDoc {
    /// Build the schedule, resolving the calendar code with `calendars`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchedule`] for a document that is not version 1, has no
    /// or an unknown `time_zone`, uses business days without a `calendar`,
    /// or fails the [`ScheduleBuilder`]'s checks; the
    /// resolver's error for an unknown calendar.
    pub fn schedule<R: CalendarResolver>(&self, calendars: &R) -> Result<Schedule<BusinessDays<R::Bdp>>> {
        let invalid = |what: String| Error::InvalidSchedule(what);
        self.check_version()?;
        let zone: Tz = match &self.time_zone {
            None => return Err(invalid("`time_zone` is required".into())),
            Some(name) => name.parse().map_err(|_| invalid(format!("`time_zone`: {name} is not an IANA time zone")))?,
        };
        let bdp = match self.calendar.as_deref() {
            Some(WEEKENDS) => BusinessDays::Weekends(WeekendSkipper::new()),
            Some(code) => BusinessDays::Calendar(calendars.resolve(code)?),
            None => {
                self.needs_no_calendar("the document")?;
                BusinessDays::NoCalendar(WeekendSkipper::new())
            }
        };
        let policy = self.options.dst_policy.unwrap_or_default();
        let builder = Schedule::builder(self.start.first(&zone, policy)?, bdp)
            .dst_policy(policy)
            .interval_mode(self.options.interval_mode.unwrap_or_default())
            .time_anchor(self.options.time_anchor.unwrap_or_default());
        self.fill(builder, &zone, policy)?.build()
    }

    /// Read this document as an instance's selection from `definition`,
    /// a schedule built from the definition's document.
    ///
    /// An instance document has the definition's time zone, business days
    /// and options; it may not set `time_zone`, `calendar` or `options`. Its
    /// `count` becomes `until` (see [`SelectorBuilder::build`]).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchedule`] for a document that is not version 1, sets
    /// `time_zone`, `calendar` or `options`, uses business days when the
    /// definition has no calendar, or fails the builder's checks.
    pub fn selector<B: BizDayProcessor>(
        &self,
        definition: &Schedule<BusinessDays<B>>,
    ) -> Result<Selector<BusinessDays<B>>> {
        self.check_version()?;
        let own = [
            ("time_zone", self.time_zone.is_some()),
            ("calendar", self.calendar.is_some()),
            ("options", !self.options.is_empty()),
        ];
        if let Some((field, _)) = own.iter().find(|(_, set)| *set) {
            return Err(Error::InvalidSchedule(format!(
                "an instance document may not set `{field}`; it uses its definition's"
            )));
        }
        if matches!(definition.bdp, BusinessDays::NoCalendar(_)) {
            self.needs_no_calendar("its definition")?;
        }
        let zone = definition.zone();
        let policy = definition.options.0;
        let builder = definition.selector(self.start.first(&zone, policy)?);
        SelectorBuilder(self.fill(builder.0, &zone, policy)?).build()
    }

    fn check_version(&self) -> Result<()> {
        if self.version != VERSION {
            return Err(Error::InvalidSchedule(format!(
                "version {} is not supported; this reads version {VERSION}",
                self.version
            )));
        }
        Ok(())
    }

    /// Add the window, rules and exclusions to `builder`.
    fn fill<B: BizDayProcessor>(
        &self,
        mut builder: ScheduleBuilder<B>,
        zone: &Tz,
        policy: DstPolicy,
    ) -> Result<ScheduleBuilder<B>> {
        builder = builder.exclude_match(self.exclude.by);
        if let Some(until) = self.until {
            builder = builder.until(until.last(zone, policy)?);
        }
        if let Some(count) = self.count {
            builder = builder.count(count);
        }
        for rule in &self.rules {
            builder = match &rule.id {
                Some(id) => builder.named_rule(id, &rule.rule),
                None => builder.rule(&rule.rule),
            };
        }
        for date in &self.exclude.dates {
            builder = builder.exclude_date(*date);
        }
        for spec in &self.exclude.rules {
            builder = builder.exclude_rule(spec);
        }
        Ok(builder)
    }

    /// Fails when a rule or exclusion uses business days: without a
    /// `calendar`, they would silently be weekdays.
    /// `whose` is the document that would name the calendar.
    fn needs_no_calendar(&self, whose: &str) -> Result<()> {
        let uses = |what: &str, spec: &str| {
            Error::InvalidSchedule(format!(
                "{what} `{spec}` uses business days, but {whose} has no `calendar` \
                 (write `calendar: {WEEKENDS}` for Saturday and Sunday off)"
            ))
        };
        for rule in &self.rules {
            // A rule that does not parse fails in the builder, with its name.
            if rule_uses_business_days(&rule.rule).unwrap_or(false) {
                return Err(uses("rule", &rule.rule));
            }
        }
        for spec in &self.exclude.rules {
            if spec.parse::<crate::date::Spec>().is_ok_and(|s| s.uses_business_days()) {
                return Err(uses("exclusion", spec));
            }
        }
        Ok(())
    }
}
