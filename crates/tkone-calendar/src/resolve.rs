//! From a [`CalendarFile`] to a [`Calendar`]: check the file, expand rules
//! and listed dates into holidays, then move weekend holidays to their days
//! off.
//!
//! Every problem in a file is collected and reported together, so one run
//! shows everything to fix.

use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

use chrono::{Datelike, Days, NaiveDate, Weekday};
use fallible_iterator::FallibleIterator;
use regex::Regex;
use tkone_schedule::biz_day::WeekendSkipper;
use tkone_schedule::date::NaiveSpecIterator;

use crate::calendar::{Calendar, Holiday, Weekend};
use crate::model::{CalendarFile, Dates, Substitute};
use crate::{Error, Result};

/// `~` and the adjustment after it.
static ADJUSTMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"~\s*([A-Z0-9]*)").unwrap());
/// A count (`x3`), standing alone as in a schedule's bounds.
static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)x\d").unwrap());

/// Most dates a rule may give in one year. A holiday rule gives one or two;
/// more is a mistake such as `YY-MM-DD`.
const MAX_PER_YEAR: usize = 4;

pub(crate) fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn parse_weekday(s: &str) -> Option<Weekday> {
    Some(match s {
        "MON" => Weekday::Mon,
        "TUE" => Weekday::Tue,
        "WED" => Weekday::Wed,
        "THU" => Weekday::Thu,
        "FRI" => Weekday::Fri,
        "SAT" => Weekday::Sat,
        "SUN" => Weekday::Sun,
        _ => return None,
    })
}

/// A holiday before weekend holidays are moved.
struct Draft {
    name: String,
    actual: NaiveDate,
    observed: NaiveDate,
    /// The rule's own adjustment moved it, or it is a listed date: it is the
    /// day off as it stands.
    fixed: bool,
    substitute: Substitute,
}

pub(crate) fn build(file: &CalendarFile) -> Result<Calendar> {
    let mut problems: Vec<String> = Vec::new();
    let invalid = |problems: Vec<String>| Error::Invalid { code: file.code.clone(), problems };

    if file.code.is_empty() || !file.code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-') {
        problems.push(format!("code `{}` must be capital letters, digits and `-`", file.code));
    }
    if file.zone.parse::<chrono_tz::Tz>().is_err() {
        problems.push(format!("zone `{}` is not an IANA time zone", file.zone));
    }
    let mut weekend = Weekend::default();
    for day in &file.weekend {
        match parse_weekday(day) {
            Some(wd) if weekend.contains(wd) => problems.push(format!("weekend lists {day} twice")),
            Some(wd) => weekend.insert(wd),
            None => problems.push(format!("weekend day `{day}` is not MON … SUN")),
        }
    }
    let date = |field: &str, value: &str, problems: &mut Vec<String>| {
        let parsed = parse_date(value);
        if parsed.is_none() {
            problems.push(format!("{field} `{value}` is not a date (YYYY-MM-DD)"));
        }
        parsed
    };
    let from = date("valid_from", &file.valid_from, &mut problems);
    let until = date("valid_until", &file.valid_until, &mut problems);
    let confirmed = date("confirmed_until", &file.confirmed_until, &mut problems);
    let (Some(from), Some(until), Some(confirmed)) = (from, until, confirmed) else {
        return Err(invalid(problems));
    };
    if !(from <= confirmed && confirmed <= until) {
        problems.push("dates must run valid_from ≤ confirmed_until ≤ valid_until".into());
    }
    if !problems.is_empty() {
        return Err(invalid(problems));
    }

    let mut drafts: Vec<Draft> = Vec::new();
    // (name, date) pairs, to find a date that is both excepted and listed.
    let mut excepted: HashSet<(String, NaiveDate)> = HashSet::new();
    let mut listed: HashSet<(String, NaiveDate)> = HashSet::new();

    for h in &file.holidays {
        let name = &h.name;
        if name.trim().is_empty() {
            problems.push("a holiday has no name".into());
            continue;
        }
        let mut problem = |p: String| problems.push(format!("`{name}`: {p}"));
        let substitute = h.substitute.unwrap_or(file.substitute);
        match (&h.rule, &h.dates) {
            (Some(rule), None) => {
                if rule.contains('@') || rule.contains("..") || COUNT.is_match(rule) {
                    problem(format!("rule `{rule}` has bounds; use `from` and `until`"));
                    continue;
                }
                if let Some(adj) = ADJUSTMENT
                    .captures_iter(rule)
                    .map(|c| c[1].to_string())
                    .find(|adj| !matches!(adj.as_str(), "W" | "NW" | "PW"))
                {
                    problem(format!(
                        "rule `{rule}` uses `~{adj}`; a holiday rule may only move off weekends (`~W`, `~NW`, `~PW`), \
                         since business days come from this calendar"
                    ));
                    continue;
                }
                if !h.except.is_empty() && h.source.is_none() {
                    problem("`except` needs a `source`".into());
                }
                if let (Some(a), Some(b)) = (h.from, h.until) {
                    if a > b {
                        problem(format!("from {a} is after until {b}"));
                        continue;
                    }
                }
                let mut except: BTreeSet<NaiveDate> = BTreeSet::new();
                for d in &h.except {
                    match parse_date(d) {
                        Some(d) => {
                            except.insert(d);
                            excepted.insert((name.clone(), d));
                        }
                        None => problem(format!("except `{d}` is not a date")),
                    }
                }
                // A year either side, for rules moved across the range's ends.
                let first = h.from.unwrap_or(i32::MIN).max(from.year() - 1);
                let last = h.until.unwrap_or(i32::MAX).min(until.year() + 1);
                if first > last {
                    problem(format!("from/until leave no year between {from} and {until}"));
                    continue;
                }
                let produced = match expand(rule, first, last) {
                    Ok(produced) => produced,
                    Err(p) => {
                        problem(p);
                        continue;
                    }
                };
                for (actual, observed) in &produced {
                    if except.remove(actual) {
                        continue;
                    }
                    drafts.push(Draft {
                        name: name.clone(),
                        actual: *actual,
                        observed: *observed,
                        fixed: actual != observed,
                        substitute,
                    });
                }
                if !produced.iter().any(|(a, _)| (from..=until).contains(a)) {
                    problem(format!("rule `{rule}` gives no date between {from} and {until}"));
                }
                for d in except {
                    problem(format!("except {d} is not a date the rule gives"));
                }
            }
            (None, Some(dates)) => {
                if h.from.is_some() || h.until.is_some() || !h.except.is_empty() {
                    problem("`from`, `until` and `except` are for rules; list only the dates".into());
                }
                if h.source.is_none() {
                    problem("`dates` needs a `source`".into());
                }
                if h.substitute.is_some() {
                    problem("listed dates are days off as they stand; `substitute` is for rules".into());
                }
                if let Dates::ByYear(years) = dates {
                    let years: Vec<i32> = years.keys().map(|y| y.0).collect();
                    if let Some(w) = years.windows(2).find(|w| w[1] != w[0] + 1) {
                        problem(format!("dates skip from {} to {}; list every year", w[0], w[1]));
                    }
                }
                for (year, d) in dates.iter() {
                    let Some(date) = parse_date(d) else {
                        problem(format!("date `{d}` is not a date"));
                        continue;
                    };
                    if year.is_some_and(|y| y != date.year()) {
                        problem(format!("{d} is listed under {}", year.unwrap()));
                    }
                    if !(from..=until).contains(&date) {
                        problem(format!("{d} is outside {from} to {until}"));
                    }
                    if weekend.contains(date.weekday()) {
                        problem(format!("{d} is a weekend day; list the day off instead"));
                    }
                    if !listed.insert((name.clone(), date)) {
                        problem(format!("{d} is listed twice"));
                    }
                    drafts.push(Draft { name: name.clone(), actual: date, observed: date, fixed: true, substitute });
                }
            }
            _ => problem("give either a `rule` or `dates`".into()),
        }
    }

    for (name, date) in excepted.intersection(&listed) {
        problems.push(format!("`{name}`: {date} is both excepted and listed"));
    }
    let mut seen: HashSet<(&str, NaiveDate)> = HashSet::new();
    for d in &drafts {
        if !seen.insert((&d.name, d.actual)) {
            problems.push(format!("`{}`: {} appears twice (two entries give it)", d.name, d.actual));
        }
    }
    if !problems.is_empty() {
        problems.sort();
        problems.dedup();
        return Err(invalid(problems));
    }

    let holidays = substitute(drafts, &weekend)
        .into_iter()
        .filter(|h| (from..=until).contains(&h.observed))
        .collect();
    Ok(Calendar::new(
        file.code.clone(),
        file.name.clone(),
        Some(file.kind),
        Some(file.zone.clone()),
        weekend,
        (from, until, confirmed),
        holidays,
    ))
}

/// The (actual, observed) dates of `rule` in years `first..=last`.
fn expand(rule: &str, first: i32, last: i32) -> std::result::Result<Vec<(NaiveDate, NaiveDate)>, String> {
    let start = NaiveDate::from_ymd_opt(first, 1, 1)
        .and_then(|d| d.pred_opt())
        .ok_or_else(|| format!("year {first} is out of range"))?
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let mut iter = NaiveSpecIterator::new_after(rule, WeekendSkipper::new(), start).map_err(|e| format!("rule `{rule}`: {e}"))?;
    let mut out: Vec<(NaiveDate, NaiveDate)> = Vec::new();
    loop {
        match iter.next() {
            Ok(Some(o)) => {
                let actual = o.actual().date();
                if actual.year() > last {
                    break;
                }
                if out.iter().filter(|(a, _)| a.year() == actual.year()).count() >= MAX_PER_YEAR {
                    return Err(format!("rule `{rule}` gives more than {MAX_PER_YEAR} dates in {}", actual.year()));
                }
                out.push((actual, o.observed().date()));
            }
            Ok(None) => break,
            Err(e) => return Err(format!("rule `{rule}`: {e}")),
        }
    }
    Ok(out)
}

/// Move each weekend holiday to its day off, in date order, so an earlier
/// holiday takes the first free day.
fn substitute(mut drafts: Vec<Draft>, weekend: &Weekend) -> Vec<Holiday> {
    drafts.sort_by_key(|d| d.actual);
    let mut taken: HashSet<NaiveDate> =
        drafts.iter().map(|d| d.observed).filter(|d| !weekend.contains(d.weekday())).collect();
    let step = |d: NaiveDate, forward: bool| if forward { d.checked_add_days(Days::new(1)) } else { d.checked_sub_days(Days::new(1)) };
    drafts
        .into_iter()
        .map(|d| {
            let observed = if d.fixed || !weekend.contains(d.observed.weekday()) {
                d.observed
            } else {
                match d.substitute {
                    Substitute::None => d.observed,
                    Substitute::NextFreeWeekday => {
                        let mut day = d.observed;
                        while let Some(next) = step(day, true) {
                            day = next;
                            if !weekend.contains(day.weekday()) && !taken.contains(&day) {
                                break;
                            }
                        }
                        day
                    }
                    Substitute::SundayToMonday if d.observed.weekday() != Weekday::Sun => d.observed,
                    Substitute::NextWeekday | Substitute::SundayToMonday => {
                        let mut day = d.observed;
                        while let Some(next) = step(day, true) {
                            day = next;
                            if !weekend.contains(day.weekday()) {
                                break;
                            }
                        }
                        day
                    }
                    // The closest weekday; forwards on a tie.
                    Substitute::NearestWeekday => (1..7)
                        .find_map(|k| {
                            let fwd = d.observed.checked_add_days(Days::new(k))?;
                            let back = d.observed.checked_sub_days(Days::new(k))?;
                            if !weekend.contains(fwd.weekday()) {
                                Some(fwd)
                            } else if !weekend.contains(back.weekday()) {
                                Some(back)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(d.observed),
                }
            };
            taken.insert(observed);
            Holiday { name: d.name, actual: d.actual, observed }
        })
        .collect()
}
