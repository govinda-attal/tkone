//! A start, an end, a count and a calendar written in the spec:
//! `<spec> @<start> ..<end> x<count> [cal=<codes>]`.
//!
//! All are optional and apply to the whole spec, including every part of a
//! union and its exclusions. A start or end is a date or a datetime,
//! optionally with a UTC offset and an IANA time zone in RFC 9557 brackets:
//!
//! ```text
//! @2025-01-06
//! @2025-01-06T09:00:00
//! @2025-01-06T09:00:00+00:00
//! @2025-01-06T09:00:00[Europe/London]
//! ..2026-01-06                          (a date-only end includes the whole day)
//! ```
//!
//! An end can instead be a duration from the start, in years, months, weeks
//! and days: `..+30Y`, `..+5Y6M`, `..+2W`. It ends just before the start plus
//! the duration, so `@2025-01-31 ..+30Y` covers exactly 30 years. A schedule
//! with a duration needs a start, written or given to the builder.
//!
//! The count (`x36`) stops the schedule after that many results, counted from
//! the start; with an end too, whichever comes first stops it. Excluded
//! occurrences are not counted.
//!
//! The calendar (`[cal=GBLO]`, `[cal=GBLO+USNY]`) names the holiday calendar
//! whose business days the schedule counts and moves to. The builder checks
//! that the business-day processor it is given is that calendar.
//!
//! The start anchors the schedule (cadences, positions, steps and the time
//! phase count from it) and is an inclusive lower bound: results are the
//! occurrences at or after it. The end is an inclusive upper bound. A start,
//! end or count given to a builder takes precedence over the text.

use std::str::FromStr;
use std::sync::LazyLock;

use chrono::{DateTime, Days, Duration, FixedOffset, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use regex::Regex;

use crate::prelude::*;
use crate::utils::resolve_local;
use crate::DstPolicy;

/// ` x36`: a whitespace-separated token, so zone names such as
/// `America/Mexico_City` never match.
static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)x(\d+)(?:\s|$)").unwrap());

/// `[cal=GBLO+USNY]`. The `=` keeps it apart from lists such as `[2025-2027]`.
static CALENDAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[cal=([^\]]*)\]").unwrap());
/// One calendar code: capitals, digits and `-`.
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z0-9-]+$").unwrap());

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

/// `+5Y6M`: years, months, weeks and days, in that order.
static PERIOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\+(?:(\d+)Y)?(?:(\d+)M)?(?:(\d+)W)?(?:(\d+)D)?$").unwrap());

/// A duration written as an end (`..+5Y6M`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Period {
    years: u32,
    months: u32,
    weeks: u32,
    days: u32,
}

/// An end as written: a time, or a duration from the start.
#[derive(Debug, Clone, PartialEq)]
enum End {
    At(TextTime),
    After(Period),
}

/// The start, end and time zone written after a spec.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Bounds {
    start: Option<TextTime>,
    end: Option<End>,
    zone: Option<chrono_tz::Tz>,
    count: Option<usize>,
    /// The calendar codes, as written.
    calendar: Option<Vec<String>>,
}

/// Splits `spec @start ..end` into the spec and its bounds.
pub(crate) fn split_bounds(spec: &str) -> Result<(&str, Bounds)> {
    let at = spec.find('@');
    let dots = spec.find("..");
    let count = COUNT.captures(spec);
    let x = count.as_ref().map(|c| c.get(0).unwrap().start());
    let calendar = CALENDAR.captures(spec);
    let cal = calendar.as_ref().map(|c| c.get(0).unwrap().start());
    let body_end = [at, dots, x, cal].into_iter().flatten().min().unwrap_or(spec.len());
    let body = spec[..body_end].trim();
    let mut bounds = Bounds::default();
    let invalid = |reason: String| Error::InvalidSpecBounds(format!("{spec}: {reason}"));

    let token = |from: usize| {
        let next = [at, dots, x, cal].into_iter().flatten().filter(|&i| i > from).min().unwrap_or(spec.len());
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
    if let Some(caps) = &calendar {
        if CALENDAR.find_iter(spec).count() > 1 {
            return Err(invalid("more than one calendar".into()));
        }
        let codes: Vec<String> = caps[1].split('+').map(str::to_string).collect();
        if let Some(code) = codes.iter().find(|c| !CODE.is_match(c)) {
            return Err(invalid(format!("calendar code `{code}` must be capitals, digits and `-`")));
        }
        if let Some(code) = codes.iter().enumerate().find_map(|(i, c)| codes[..i].contains(c).then_some(c)) {
            return Err(invalid(format!("calendar code `{code}` is named twice")));
        }
        bounds.calendar = Some(codes);
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
        let text = token(i + 2);
        if text.starts_with('+') {
            bounds.end = Some(End::After(parse_period(text).map_err(invalid)?));
        } else {
            let (time, zone) = parse_time(text).map_err(invalid)?;
            match (bounds.zone, zone) {
                (Some(a), Some(b)) if a != b => return Err(invalid("the start and end name different time zones".into())),
                (None, zone) => bounds.zone = zone,
                _ => {}
            }
            bounds.end = Some(End::At(time));
        }
    }
    if let (Some(start), Some(End::At(end))) = (&bounds.start, &bounds.end) {
        if end.local < start.local && start.offset.is_none() && end.offset.is_none() {
            return Err(invalid("the end is before the start".into()));
        }
    }
    Ok((body, bounds))
}

/// The spec without its `@start ..end xN [cal=…]` suffix, and the suffix as
/// written.
pub(crate) fn split_suffix(spec: &str) -> (&str, &str) {
    let x = COUNT.find(spec).map(|m| m.start());
    let cal = CALENDAR.find(spec).map(|m| m.start());
    let i = [spec.find('@'), spec.find(".."), x, cal].into_iter().flatten().min().unwrap_or(spec.len());
    (spec[..i].trim_end(), spec[i..].trim_start())
}

fn parse_period(text: &str) -> core::result::Result<Period, String> {
    let caps = PERIOD
        .captures(text)
        .filter(|caps| caps.iter().skip(1).any(|c| c.is_some()))
        .ok_or_else(|| format!("`{text}` is not a duration (e.g. +30Y, +5Y6M, +2W, +90D)"))?;
    let field = |i: usize| -> core::result::Result<u32, String> {
        caps.get(i).map_or(Ok(0), |m| m.as_str().parse().map_err(|_| format!("`{text}` is too long")))
    };
    let period = Period { years: field(1)?, months: field(2)?, weeks: field(3)?, days: field(4)? };
    if period == (Period { years: 0, months: 0, weeks: 0, days: 0 }) {
        return Err(format!("the duration `{text}` must not be zero"));
    }
    Ok(period)
}

impl Period {
    /// `local` plus the period: months as `31L` counts them (the last day of a
    /// shorter month), then weeks and days.
    fn after(&self, local: NaiveDateTime) -> Option<NaiveDateTime> {
        let months = self.years.checked_mul(12)?.checked_add(self.months)?;
        let days = u64::from(self.weeks) * 7 + u64::from(self.days);
        local.checked_add_months(Months::new(months))?.checked_add_days(Days::new(days))
    }

    /// `"5 years and 6 months"`.
    fn describe(&self) -> String {
        let parts: Vec<String> = [(self.years, "year"), (self.months, "month"), (self.weeks, "week"), (self.days, "day")]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, unit)| format!("{n} {unit}{}", if n == 1 { "" } else { "s" }))
            .collect();
        match parts.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => parts.join(""),
        }
    }
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
    /// The time zone written in brackets after the start or end.
    pub(crate) fn zone(&self) -> Option<chrono_tz::Tz> {
        self.zone
    }

    /// The count written as `xN`.
    pub(crate) fn count(&self) -> Option<usize> {
        self.count
    }

    /// The calendar written as `[cal=…]`: its codes joined with `+`, as
    /// written.
    pub(crate) fn calendar(&self) -> Option<String> {
        self.calendar.as_ref().map(|codes| codes.join("+"))
    }

    /// Fails unless `bdp` is the calendar the spec names, if it names one.
    /// Codes compare as a set: `GBLO+USNY` is `USNY+GBLO`.
    pub(crate) fn check_calendar<BDP: crate::biz_day::BizDayProcessor>(&self, bdp: &BDP) -> Result<()> {
        let Some(named) = &self.calendar else {
            return Ok(());
        };
        let named_set: std::collections::BTreeSet<&str> = named.iter().map(String::as_str).collect();
        let given = bdp.code();
        if given.as_deref().is_some_and(|g| g.split('+').map(str::trim).collect::<std::collections::BTreeSet<_>>() == named_set) {
            return Ok(());
        }
        Err(Error::Calendar(match given {
            Some(given) => format!("the spec names calendar {}, but the business days given are {given}", named.join("+")),
            None => format!(
                "the spec names calendar {}, but the business days given are not a named calendar",
                named.join("+")
            ),
        }))
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
        match &self.end {
            Some(End::At(end)) => out += &format!("{} {}", if self.start.is_some() { " until" } else { ", until" }, when(end)),
            Some(End::After(period)) => out += &format!("{} {}", if self.start.is_some() { " for" } else { ", for" }, period.describe()),
            None => {}
        }
        if let Some(zone) = self.zone {
            out += &format!(" ({})", zone.name());
        }
        if let Some(codes) = &self.calendar {
            let (last, rest) = codes.split_last().expect("a calendar has a code");
            let names = if rest.is_empty() { last.clone() } else { format!("{} and {last}", rest.join(", ")) };
            out += &format!(", using the {names} calendar{}", if rest.is_empty() { "" } else { "s" });
        }
        out
    }

    /// The start as an instant in `tz`.
    pub(crate) fn start_in<Tz: TimeZone>(&self, tz: &Tz, policy: DstPolicy) -> Result<Option<DateTime<Tz>>> {
        self.start.as_ref().map(|t| self.resolve(t, t.local, tz, policy)).transpose()
    }

    /// The end as an instant in `tz`, the last instant results may be at: a
    /// date-only end is the end of that day, and a duration ends just before
    /// `start` plus the duration.
    ///
    /// `start` is the schedule's start, the builder's or the written one; a
    /// duration without one is an error.
    pub(crate) fn end_in<Tz: TimeZone>(
        &self,
        tz: &Tz,
        policy: DstPolicy,
        start: Option<&DateTime<Tz>>,
    ) -> Result<Option<DateTime<Tz>>> {
        match &self.end {
            None => Ok(None),
            Some(End::At(t)) => {
                let local = if t.date_only {
                    t.local.date().and_hms_nano_opt(23, 59, 59, 999_999_999).unwrap()
                } else {
                    t.local
                };
                self.resolve(t, local, tz, policy).map(Some)
            }
            Some(End::After(period)) => {
                let start = start.ok_or_else(|| {
                    Error::InvalidSpecBounds("a duration end (`..+…`) needs a start: write `@…`, or give the builder one".into())
                })?;
                // Add on the wall clock of the written zone, if any.
                let zone = self.zone.unwrap_or(chrono_tz::UTC);
                let local = match self.zone {
                    Some(_) => start.with_timezone(&zone).naive_local(),
                    None => start.naive_local(),
                };
                let end = period
                    .after(local)
                    .ok_or_else(|| Error::InvalidSpecBounds(format!("the duration {} is too long", period.describe())))?;
                let end = match self.zone {
                    Some(zone) => resolve_local(&zone, end, policy)?.with_timezone(tz),
                    None => resolve_local(tz, end, policy)?,
                };
                Ok(Some(just_before(&end)))
            }
        }
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
        assert!(matches!(&b.end, Some(End::At(t)) if t.date_only));

        let (body, b) = split_bounds("YY-MM-DDT09:00:00 @2025-01-06T09:00:00[Europe/London]").unwrap();
        assert_eq!(body, "YY-MM-DDT09:00:00");
        assert_eq!(b.zone, Some(chrono_tz::Europe::London));

        let (body, b) = split_bounds("1H:00:00 ..2025-01-06T17:00+01:00").unwrap();
        assert_eq!(body, "1H:00:00");
        assert!(b.start.is_none());
        assert!(matches!(&b.end, Some(End::At(t)) if t.offset == FixedOffset::east_opt(3600)));

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
        let end = b.end_in(&chrono_tz::Europe::London, DstPolicy::Adjust, None).unwrap().unwrap();
        assert_eq!(end.naive_local(), NaiveDate::from_ymd_opt(2025, 3, 31).unwrap().and_hms_nano_opt(23, 59, 59, 999_999_999).unwrap());
        let (_, b) = split_bounds("YY-MM-15 @2025-01-06").unwrap();
        assert_eq!(b.start_in(&tz, DstPolicy::Adjust).unwrap().unwrap(), tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap());
        assert_eq!(b.describe(), ", from 6 January 2025");
        let (_, b) = split_bounds("1H:00:00 @2025-01-06T09:00:00[Europe/London] ..2025-01-06T17:00").unwrap();
        assert_eq!(b.describe(), ", from 6 January 2025 09:00:00 until 6 January 2025 17:00:00 (Europe/London)");
    }

    #[test]
    fn test_duration() {
        let period = |text: &str| match split_bounds(text).unwrap().1.end {
            Some(End::After(p)) => (p.years, p.months, p.weeks, p.days),
            other => panic!("{text}: {other:?}"),
        };
        assert_eq!(period("YY-1M-31L @2025-01-31 ..+30Y"), (30, 0, 0, 0));
        assert_eq!(period("YY-3M-L ..+5Y6M @2025-03-31"), (5, 6, 0, 0));
        assert_eq!(period("YY-MM-FRI/2W ..+2W x3"), (0, 0, 2, 0));
        assert_eq!(period("1H:00:00 ..+1Y2M3W4D"), (1, 2, 3, 4));
        for bad in [
            "YY-MM-15 ..+",
            "YY-MM-15 ..+0Y",
            "YY-MM-15 ..+6M5Y",
            "YY-MM-15 ..+2H",
            "YY-MM-15 ..20Y",
            "YY-MM-15 ..-1Y",
            "YY-MM-15 ..+1Y ..2030-01-01",
            "YY-MM-15 ..+99999999999Y",
        ] {
            assert!(split_bounds(bad).is_err(), "{bad} should be rejected");
        }

        let tz = chrono_tz::Europe::London;
        let at = |y, m, d| tz.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap();
        // The end is just before the start plus the duration.
        let (_, b) = split_bounds("YY-1M-31L @2025-01-31 ..+30Y").unwrap();
        let end = b.end_in(&tz, DstPolicy::Adjust, Some(&at(2025, 1, 31))).unwrap().unwrap();
        assert_eq!(end, just_before(&at(2055, 1, 31)));
        // Months fall back to the last day of a shorter month.
        let (_, b) = split_bounds("YY-MM-DD ..+1Y").unwrap();
        let end = b.end_in(&tz, DstPolicy::Adjust, Some(&at(2024, 2, 29))).unwrap().unwrap();
        assert_eq!(end, just_before(&at(2025, 2, 28)));
        let (_, b) = split_bounds("YY-MM-DD ..+1M2W3D").unwrap();
        let end = b.end_in(&tz, DstPolicy::Adjust, Some(&at(2025, 1, 31))).unwrap().unwrap();
        assert_eq!(end, just_before(&at(2025, 3, 17)));
        // A duration needs a start.
        assert!(matches!(b.end_in(&tz, DstPolicy::Adjust, None), Err(Error::InvalidSpecBounds(_))));

        let describe = |text: &str| split_bounds(text).unwrap().1.describe();
        assert_eq!(describe("YY-1M-31L @2025-01-31 ..+30Y"), ", from 31 January 2025 for 30 years");
        assert_eq!(describe("YY-1M-31L @2025-01-31 ..+1Y x6"), ", 6 times, from 31 January 2025 for 1 year");
        assert_eq!(describe("YY-MM-DD ..+5Y6M"), ", for 5 years and 6 months");
        assert_eq!(describe("YY-MM-DD ..+1Y1M1W1D"), ", for 1 year, 1 month, 1 week and 1 day");
    }
}
