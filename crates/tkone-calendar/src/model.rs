//! The calendar file format, as written in `data/*.yaml` and embedded as
//! `compiled/calendars.json`. See `data/README.md` for a guide.
//!
//! Dates are strings (`"2025-12-25"`) parsed by this crate, never by the
//! YAML parser, so no parser can turn them into something else.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One calendar file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarFile {
    /// The calendar's code: an FpML / ISDA business centre (`GBLO`) or an
    /// exchange's MIC (`XNYS`). The file is named after it.
    pub code: String,
    /// A readable name.
    pub name: String,
    /// What kind of calendar it is.
    pub kind: Kind,
    /// The IANA time zone of the place.
    pub zone: String,
    /// The days of the week that are never business days: `[SAT, SUN]`.
    pub weekend: Vec<String>,
    /// The first date the calendar answers for.
    pub valid_from: String,
    /// The last date the calendar answers for.
    pub valid_until: String,
    /// The last date the data has been checked against published sources.
    /// Later dates come from the rules alone and are reported as projected.
    pub confirmed_until: String,
    /// What happens to a holiday that falls on a weekend, unless the holiday
    /// says otherwise.
    #[serde(default, skip_serializing_if = "Substitute::is_none")]
    pub substitute: Substitute,
    /// Where the rules come from: laws, the publisher's holiday pages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// The holidays.
    pub holidays: Vec<HolidayEntry>,
}

/// What kind of calendar a file describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Public holidays of a country or region.
    Public,
    /// The days banks settle payments in a business centre.
    Settlement,
    /// The days an exchange trades.
    Exchange,
}

/// How a holiday that falls on a weekend is moved to a day off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Substitute {
    /// It is not moved: the day off is lost. Default.
    #[default]
    None,
    /// The next weekday that is not already a day off (UK, Australia, New
    /// Zealand). Two holidays on one weekend give Monday and Tuesday.
    NextFreeWeekday,
    /// The nearest weekday: Saturday to Friday, Sunday to Monday (US federal
    /// holidays, NYSE).
    NearestWeekday,
    /// The next weekday, even when it is already a day off: two holidays can
    /// share it (New Zealand's Waitangi Day and Anzac Day).
    NextWeekday,
    /// Sunday to Monday; a holiday on Saturday is not moved (the Federal
    /// Reserve, South Africa).
    SundayToMonday,
}

impl Substitute {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

/// One holiday: a rule, or a list of dates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HolidayEntry {
    /// The holiday's name.
    pub name: String,
    /// A tkone date spec, such as `YY-12-25`, `YY-05-MON#L` or `YY-MM-EAS-2`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// Dates: a list, or a map from year to date(s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dates: Option<Dates>,
    /// The first year a rule applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<i32>,
    /// The last year a rule applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i32>,
    /// Dates the rule does not produce (the holiday moved, or was cancelled).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub except: Vec<String>,
    /// Overrides the calendar's `substitute` for this holiday.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substitute: Option<Substitute>,
    /// Where `dates` and `except` come from. Required for both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Anything a reader should know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The dates of a listed holiday.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dates {
    /// `["2023-05-08"]`.
    List(Vec<String>),
    /// `{2025: "2025-02-11", 2026: "2026-02-01"}`: one entry per year, so a
    /// year cannot be listed twice and a missing year shows.
    ByYear(BTreeMap<Year, OneOrMany>),
}

/// A year used as a key. Written as a number in YAML (`2025:`) and as a
/// string in JSON (`"2025":`); both read back the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Year(pub i32);

impl<'de> Deserialize<'de> for Year {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Year;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a year")
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Year, E> {
                i32::try_from(v).map(Year).map_err(E::custom)
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Year, E> {
                i32::try_from(v).map(Year).map_err(E::custom)
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Year, E> {
                v.parse().map(Year).map_err(|_| E::custom(format!("`{v}` is not a year")))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// One date, or several in the same year.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    /// `"2025-02-11"`.
    One(String),
    /// `["2033-01-02", "2033-12-23"]`.
    Many(Vec<String>),
}

impl Dates {
    /// Every date, with the year it is listed under (if any).
    pub(crate) fn iter(&self) -> Vec<(Option<i32>, &str)> {
        match self {
            Dates::List(dates) => dates.iter().map(|d| (None, d.as_str())).collect(),
            Dates::ByYear(years) => years
                .iter()
                .flat_map(|(year, dates)| {
                    let dates: Vec<&str> = match dates {
                        OneOrMany::One(d) => vec![d.as_str()],
                        OneOrMany::Many(ds) => ds.iter().map(String::as_str).collect(),
                    };
                    dates.into_iter().map(move |d| (Some(year.0), d))
                })
                .collect(),
        }
    }
}
