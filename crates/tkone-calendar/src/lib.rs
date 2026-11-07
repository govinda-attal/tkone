//! Holiday calendars for [`tkone_schedule`].
//!
//! A [`Calendar`] knows which dates are business days in one place, such as
//! London (`GBLO`) or the TARGET2 payment system (`EUTA`). It is a
//! [`BizDayProcessor`](tkone_schedule::biz_day::BizDayProcessor), so any
//! schedule can count and move to its business days:
//!
//! ```rust
//! use chrono::{NaiveDate, TimeZone, Utc};
//! use fallible_iterator::FallibleIterator;
//! use tkone_calendar::Calendars;
//! use tkone_schedule::date::SpecIteratorBuilder;
//!
//! let london = Calendars::builtin().get("GBLO").unwrap();
//!
//! // Christmas 2021 fell on a Saturday: the days off were Monday 27 and
//! // Tuesday 28 December.
//! let day = |m, d| NaiveDate::from_ymd_opt(2021, m, d).unwrap();
//! assert!(!london.is_business_day(day(12, 27)).unwrap());
//! assert!(!london.is_business_day(day(12, 28)).unwrap());
//!
//! // The last business day of December 2021, and the first of January 2022
//! // (3 January was the New Year's Day holiday).
//! let start = Utc.with_ymd_and_hms(2021, 12, 2, 0, 0, 0).unwrap();
//! let got: Vec<_> = SpecIteratorBuilder::new_after("YY-MM-BD#[1,L]", london, start)
//!     .build()
//!     .unwrap()
//!     .take(2)
//!     .map(|o| Ok(o.observed().date_naive()))
//!     .collect()
//!     .unwrap();
//! assert_eq!(got, [day(12, 31), NaiveDate::from_ymd_opt(2022, 1, 4).unwrap()]);
//! ```
//!
//! # Where the data comes from
//!
//! Each calendar is a YAML file in `data/`, compiled into this crate. A
//! holiday is either a **rule**, written as a tkone date spec
//! (`YY-05-MON#L`, `YY-MM-EAS-2`), or a list of **dates** with the source
//! that announced them (a coronation, a state funeral). Holidays that fall on
//! a weekend move to a day off by the calendar's `substitute` policy.
//! `data/README.md` describes the format.
//!
//! A calendar answers only for dates between its `valid_from` and
//! `valid_until`: outside them it returns [`Error::OutOfRange`] rather than
//! guess. After `confirmed_until` the rules still apply, but one-off holidays
//! not yet announced cannot be known, so [`Calendar::status`] reports those
//! dates as [`Status::Projected`].
//!
//! # Joint calendars
//!
//! `Calendars::builtin().get("GBLO+EUTA")` is a business day only when it is
//! one in both London and TARGET2.
//!
//! # Schedule documents
//!
//! [`Calendars`] resolves a schedule document's `calendar` code (`GBLO`,
//! `GBLO+USNY`): it implements
//! [`CalendarResolver`](tkone_schedule::schedule::document::CalendarResolver).

mod biz_day;
mod calendar;
mod calendars;
mod error;
pub mod model;
mod resolve;

#[cfg(test)]
mod tests;

pub use calendar::{Calendar, Holiday, Status};
pub use calendars::Calendars;
pub use error::{Error, Result};
pub use model::Kind;

/// Reads every `*.yaml` calendar file in `dir`, in name order. Each file must
/// be named after its code, and use no YAML anchors or aliases.
#[cfg(feature = "yaml")]
pub fn read_dir(dir: &std::path::Path) -> Result<Vec<model::CalendarFile>> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| Error::Parse(format!("{}: {e}", dir.display())))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    paths.sort();
    let mut files = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|e| Error::Parse(format!("{}: {e}", path.display())))?;
        if let Some(line) = anchor_line(&text) {
            return Err(Error::Parse(format!("{}:{line}: YAML anchors and aliases are not allowed", path.display())));
        }
        let file: model::CalendarFile =
            serde_yaml_ng::from_str(&text).map_err(|e| Error::Parse(format!("{}: {e}", path.display())))?;
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        if stem != file.code {
            return Err(Error::Parse(format!("{}: the file must be named {}.yaml", path.display(), file.code)));
        }
        files.push(file);
    }
    Ok(files)
}

/// The first line (1-based) with a YAML anchor (`&name`) or alias (`*name`)
/// outside quotes and comments.
#[cfg(feature = "yaml")]
fn anchor_line(text: &str) -> Option<usize> {
    use std::sync::LazyLock;
    static QUOTED: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r#""[^"]*"|'[^']*'"#).unwrap());
    static ANCHOR: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?:^|[\s\[{,:-])[&*][A-Za-z0-9_]").unwrap());
    text.lines().position(|line| {
        let line = QUOTED.replace_all(line, "\"\"");
        let line = line.split('#').next().unwrap_or_default();
        ANCHOR.is_match(line)
    })
    .map(|i| i + 1)
}

#[cfg(all(test, feature = "yaml"))]
mod yaml_tests {
    use super::*;

    #[test]
    fn test_anchor_line() {
        assert_eq!(anchor_line("a: 1\nb: &x 2\n"), Some(2));
        assert_eq!(anchor_line("a: *x\n"), Some(1));
        assert_eq!(anchor_line("a: \"&x\"\nb: 1 # *not\n"), None);
    }
}
