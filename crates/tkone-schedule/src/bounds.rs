//! A start, an end and a count written in the spec:
//! `<spec> @<start> ..<end> x<count>`.
//!
//! All are optional and apply to the whole spec, including every part of a
//! union and its exclusions. Each is a date or a datetime, optionally with a
//! UTC offset and an IANA time zone in RFC 9557 brackets:
//!
//! ```text
//! @2025-01-06
//! @2025-01-06T09:00:00
//! @2025-01-06T09:00:00+00:00
//! @2025-01-06T09:00:00[Europe/London]
//! ..2026-01-06                          (a date-only end includes the whole day)
//! ```
//!
//! The count (`x36`) stops the schedule after that many results, counted from
//! the start; with an end too, whichever comes first stops it. Excluded
//! occurrences are not counted.
//!
//! The start anchors the schedule (cadences, positions, steps and the time
//! phase count from it) and is an inclusive lower bound: results are the
//! occurrences at or after it. The end is an inclusive upper bound. A start,
//! end or count given to a builder takes precedence over the text.

use std::str::FromStr;
use std::sync::LazyLock;

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use regex::Regex;

use crate::prelude::*;
use crate::utils::resolve_local;
use crate::DstPolicy;

/// ` x36`: a whitespace-separated token, so zone names such as
/// `America/Mexico_City` never match.
static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)x(\d+)(?:\s|$)").unwrap());

static TEXT_TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\d{4}-\d{2}-\d{2})(?:T(\d{2}:\d{2}(?::\d{2})?))?(Z|[+-]\d{2}:\d{2})?(?:\[([^\]]+)\])?$",
    )
    .unwrap()
});

/// A start or end as written.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextTime {
    local: NaiveDateTime,
    offset: Option<FixedOffset>,
    date_only: bool,
}

/// The start, end and time zone written after a spec.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Bounds {
    start: Option<TextTime>,
    end: Option<TextTime>,
    zone: Option<chrono_tz::Tz>,
    count: Option<usize>,
}

/// Splits `spec @start ..end` into the spec and its bounds.
pub(crate) fn split_bounds(spec: &str) -> Result<(&str, Bounds)> {
    let at = spec.find('@');
    let dots = spec.find("..");
    let count = COUNT.captures(spec);
    let x = count.as_ref().map(|c| c.get(0).unwrap().start());
    let body_end = [at, dots, x].into_iter().flatten().min().unwrap_or(spec.len());
    let body = spec[..body_end].trim();
    let mut bounds = Bounds::default();
    let invalid = |reason: String| Error::InvalidSpecBounds(format!("{spec}: {reason}"));

    let token = |from: usize| {
        let next = [at, dots, x].into_iter().flatten().filter(|&i| i > from).min().unwrap_or(spec.len());
        spec[from..next].trim()
    };
    if let Some(caps) = &count {
        let n: usize = caps[1].parse().map_err(|_| invalid(format!("`x{}` is too large", &caps[1])))?;
        if n == 0 {
            return Err(invalid("the count `x0` must be at least 1".into()));
        }
        if COUNT.find_at(spec, caps.get(1).unwrap().end()).is_some() {
            return Err(invalid("more than one count".into()));
        }
        bounds.count = Some(n);
    }
    if let Some(i) = at {
        if spec[i + 1..].contains('@') {
            return Err(invalid("more than one start".into()));
        }
        let (time, zone) = parse_time(token(i + 1)).map_err(invalid)?;
        bounds.start = Some(time);
        bounds.zone = zone;
    }
    if let Some(i) = dots {
        if spec[i + 2..].contains("..") {
            return Err(invalid("more than one end".into()));
        }
        let (time, zone) = parse_time(token(i + 2)).map_err(invalid)?;
        match (bounds.zone, zone) {
            (Some(a), Some(b)) if a != b => return Err(invalid("the start and end name different time zones".into())),
            (None, zone) => bounds.zone = zone,
            _ => {}
        }
        bounds.end = Some(time);
    }
    if let (Some(start), Some(end)) = (&bounds.start, &bounds.end) {
        if end.local < start.local && start.offset.is_none() && end.offset.is_none() {
            return Err(invalid("the end is before the start".into()));
        }
    }
    Ok((body, bounds))
}

/// The spec without its `@start ..end xN` suffix, and the suffix as written.
pub(crate) fn split_suffix(spec: &str) -> (&str, &str) {
    let x = COUNT.find(spec).map(|m| m.start());
    let i = [spec.find('@'), spec.find(".."), x].into_iter().flatten().min().unwrap_or(spec.len());
    (spec[..i].trim_end(), spec[i..].trim_start())
}

fn parse_time(text: &str) -> core::result::Result<(TextTime, Option<chrono_tz::Tz>), String> {
    let caps = TEXT_TIME
        .captures(text)
        .ok_or_else(|| format!("`{text}` is not a date or datetime (e.g. 2025-01-06T09:00:00[Europe/London])"))?;
    let date = NaiveDate::parse_from_str(&caps[1], "%Y-%m-%d").map_err(|e| format!("`{text}`: {e}"))?;
    let time = match caps.get(2) {
        Some(t) => {
            let t = t.as_str();
            let format = if t.len() == 5 { "%H:%M" } else { "%H:%M:%S" };
            NaiveTime::parse_from_str(t, format).map_err(|e| format!("`{text}`: {e}"))?
        }
        None => NaiveTime::MIN,
    };
    let offset = match caps.get(3).map(|m| m.as_str()) {
        None => None,
        Some("Z") => Some(FixedOffset::east_opt(0).unwrap()),
        Some(o) => Some(DateTime::parse_from_rfc3339(&format!("2000-01-01T00:00:00{o}")).map_err(|e| format!("`{text}`: {e}"))?.offset().to_owned()),
    };
    let zone = match caps.get(4) {
        Some(z) => Some(chrono_tz::Tz::from_str(z.as_str()).map_err(|_| format!("`{}` is not an IANA time zone", z.as_str()))?),
        None => None,
    };
    Ok((TextTime { local: date.and_time(time), offset, date_only: caps.get(2).is_none() }, zone))
}

impl Bounds {
    /// The count written as `xN`.
    pub(crate) fn count(&self) -> Option<usize> {
        self.count
    }

    /// The cursor a schedule read from its own text starts from: just
    /// before its start, or now; in its time zone (UTC if it names none).
    pub(crate) fn cursor(&self) -> Result<DateTime<chrono_tz::Tz>> {
        let zone = self.zone.unwrap_or(chrono_tz::UTC);
        Ok(match self.start_in(&zone, DstPolicy::default())? {
            Some(start) => just_before(&start),
            None => chrono::Utc::now().with_timezone(&zone),
        })
    }

    /// `", from 6 January 2025 09:00:00 until 6 January 2026 (Europe/London)"`.
    pub(crate) fn describe(&self) -> String {
        let when = |t: &TextTime| {
            let mut out = if t.date_only {
                t.local.format("%-d %B %Y").to_string()
            } else {
                t.local.format("%-d %B %Y %H:%M:%S").to_string()
            };
            if let Some(offset) = t.offset {
                out += &format!(" {offset}");
            }
            out
        };
        let mut out = String::new();
        if let Some(n) = self.count {
            out += &format!(", {n} time{}", if n == 1 { "" } else { "s" });
        }
        if let Some(start) = &self.start {
            out += &format!(", from {}", when(start));
        }
        if let Some(end) = &self.end {
            out += &format!("{} {}", if self.start.is_some() { " until" } else { ", until" }, when(end));
        }
        if let Some(zone) = self.zone {
            out += &format!(" ({})", zone.name());
        }
        out
    }

    /// The start as an instant in `tz`.
    pub(crate) fn start_in<Tz: TimeZone>(&self, tz: &Tz, policy: DstPolicy) -> Result<Option<DateTime<Tz>>> {
        self.start.as_ref().map(|t| self.resolve(t, t.local, tz, policy)).transpose()
    }

    /// The end as an instant in `tz`; a date-only end is the end of that day.
    pub(crate) fn end_in<Tz: TimeZone>(&self, tz: &Tz, policy: DstPolicy) -> Result<Option<DateTime<Tz>>> {
        self.end
            .as_ref()
            .map(|t| {
                let local = if t.date_only {
                    t.local.date().and_hms_nano_opt(23, 59, 59, 999_999_999).unwrap()
                } else {
                    t.local
                };
                self.resolve(t, local, tz, policy)
            })
            .transpose()
    }

    fn resolve<Tz: TimeZone>(&self, t: &TextTime, local: NaiveDateTime, tz: &Tz, policy: DstPolicy) -> Result<DateTime<Tz>> {
        Ok(match (t.offset, self.zone) {
            (Some(offset), _) => offset
                .from_local_datetime(&local)
                .single()
                .ok_or(Error::IteratorNotConverged)?
                .with_timezone(tz),
            (None, Some(zone)) => resolve_local(&zone, local, policy)?.with_timezone(tz),
            (None, None) => resolve_local(tz, local, policy)?,
        })
    }
}

/// Just before `t`, so results "after" it include `t` itself.
pub(crate) fn just_before<Tz: TimeZone>(t: &DateTime<Tz>) -> DateTime<Tz> {
    t.clone() - Duration::nanoseconds(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_bounds() {
        let (body, b) = split_bounds("YY-MM-15 ! YY-12-15 @2025-01-06 ..2026-01-06").unwrap();
        assert_eq!(body, "YY-MM-15 ! YY-12-15");
        assert!(b.start.is_some());
        assert!(b.end.as_ref().unwrap().date_only);

        let (body, b) = split_bounds("YY-MM-DDT09:00:00 @2025-01-06T09:00:00[Europe/London]").unwrap();
        assert_eq!(body, "YY-MM-DDT09:00:00");
        assert_eq!(b.zone, Some(chrono_tz::Europe::London));

        let (body, b) = split_bounds("1H:00:00 ..2025-01-06T17:00+01:00").unwrap();
        assert_eq!(body, "1H:00:00");
        assert!(b.start.is_none());
        assert_eq!(b.end.as_ref().unwrap().offset, FixedOffset::east_opt(3600));

        let (body, b) = split_bounds("YY-MM-15").unwrap();
        assert_eq!((body, b), ("YY-MM-15", Bounds::default()));

        for bad in [
            "YY-MM-15 @2025-13-01",
            "YY-MM-15 @tomorrow",
            "YY-MM-15 @2025-01-06[Mars/Base]",
            "YY-MM-15 @2025-01-06 @2025-02-06",
            "YY-MM-15 @2025-02-06 ..2025-01-06",
            "YY-MM-15 @2025-01-06[Europe/London] ..2026-01-06[America/New_York]",
        ] {
            assert!(split_bounds(bad).is_err(), "{bad} should be rejected");
        }
        assert_eq!(split_suffix("YY-MM-15 ! YY-12-15 @2025-01-06"), ("YY-MM-15 ! YY-12-15", "@2025-01-06"));
        assert_eq!(split_suffix("YY-MM-15 x12"), ("YY-MM-15", "x12"));
    }

    #[test]
    fn test_count() {
        let (body, b) = split_bounds("YY-MM-01~MF @2025-04-01 x36").unwrap();
        assert_eq!((body, b.count()), ("YY-MM-01~MF", Some(36)));
        let (body, b) = split_bounds("YY-MM-01 x36 @2025-04-01 ..2028-03-31").unwrap();
        assert_eq!((body, b.count()), ("YY-MM-01", Some(36)));
        assert!(b.start.is_some() && b.end.is_some());
        let (body, b) = split_bounds("YY-MM-15 x12").unwrap();
        assert_eq!((body, b.count()), ("YY-MM-15", Some(12)));
        // An `x` inside a zone name is not a count.
        let (_, b) = split_bounds("YY-MM-DDT09:00:00 @2025-01-06[America/Mexico_City]").unwrap();
        assert_eq!((b.count(), b.zone), (None, Some(chrono_tz::America::Mexico_City)));
        assert_eq!(b.describe(), ", from 6 January 2025 (America/Mexico_City)");
        let (_, b) = split_bounds("YY-MM-01 @2025-04-01 x36").unwrap();
        assert_eq!(b.describe(), ", 36 times, from 1 April 2025");
        for bad in ["YY-MM-15 x0", "YY-MM-15 x3 x4", "YY-MM-15 x99999999999999999999999"] {
            assert!(split_bounds(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn test_resolve() {
        let tz = chrono_tz::America::New_York;
        let (_, b) = split_bounds("YY-MM-15 @2025-01-06T09:00:00[Europe/London] ..2025-03-31").unwrap();
        let start = b.start_in(&tz, DstPolicy::Adjust).unwrap().unwrap();
        assert_eq!(start, tz.with_ymd_and_hms(2025, 1, 6, 4, 0, 0).unwrap());
        // The end is read in the text's zone too, and covers the whole day.
        let end = b.end_in(&chrono_tz::Europe::London, DstPolicy::Adjust).unwrap().unwrap();
        assert_eq!(end.naive_local(), NaiveDate::from_ymd_opt(2025, 3, 31).unwrap().and_hms_nano_opt(23, 59, 59, 999_999_999).unwrap());
        let (_, b) = split_bounds("YY-MM-15 @2025-01-06").unwrap();
        assert_eq!(b.start_in(&tz, DstPolicy::Adjust).unwrap().unwrap(), tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap());
        assert_eq!(b.describe(), ", from 6 January 2025");
        let (_, b) = split_bounds("1H:00:00 @2025-01-06T09:00:00[Europe/London] ..2025-01-06T17:00").unwrap();
        assert_eq!(b.describe(), ", from 6 January 2025 09:00:00 until 6 January 2025 17:00:00 (Europe/London)");
    }
}
