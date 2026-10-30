//! Plain-English description of a date spec, following the engine's rules
//! (see [`super::engine`]): eligible months from the year and month rules,
//! then the day rule, then the `~` adjustment.

use chrono::Weekday;

use super::spec::{
    BizDayAdjustment, BizDayCalendar, Cycle, DayCycle, LastDayOption, NextNthDayOption, Spec,
    WeekdayOption,
};
use crate::biz_day::Direction;
use crate::utils::join;

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];

fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn count(n: u32, unit: &str) -> String {
    if n == 1 { format!("1 {unit}") } else { format!("{n} {unit}s") }
}

/// Like [`join`], but three or more consecutive keys read as a range:
/// `"Monday to Friday"`, `"1st to 10th and 15th"`.
fn join_runs(items: impl IntoIterator<Item = (u32, String)>) -> String {
    let mut runs: Vec<Vec<(u32, String)>> = Vec::new();
    for (key, label) in items {
        match runs.last_mut() {
            Some(run) if run.last().is_some_and(|(k, _)| k + 1 == key) => run.push((key, label)),
            _ => runs.push(vec![(key, label)]),
        }
    }
    let parts: Vec<String> = runs
        .into_iter()
        .flat_map(|run| match run.as_slice() {
            [(_, first), .., (_, last)] if run.len() >= 3 => vec![format!("{first} to {last}")],
            _ => run.into_iter().map(|(_, label)| label).collect(),
        })
        .collect();
    join(&parts)
}

fn weekday(wd: Weekday) -> &'static str {
    match wd {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// Which months are eligible.
enum Months {
    All,
    In(String),
    /// Every k-th month from the start's month (`nM`, with any `nY` folded in).
    Stride(u32),
}

fn months(spec: &Spec) -> Months {
    match (&spec.years, &spec.months) {
        (Cycle::NextNth(ny), Cycle::NextNth(nm)) => Months::Stride(12 * ny + nm),
        (_, Cycle::NextNth(nm)) => Months::Stride(*nm),
        (_, Cycle::Values(ms)) => {
            Months::In(join_runs(ms.iter().map(|m| (*m, MONTHS[(*m - 1) as usize].to_string()))))
        }
        _ => Months::All,
    }
}

/// `", in 2025"`, `", in every 2nd year, counted from the start's year"`.
fn years(spec: &Spec) -> String {
    match (&spec.years, &spec.months) {
        (Cycle::Values(ys), _) => {
            format!(", in {}", join_runs(ys.iter().map(|y| (*y, y.to_string()))))
        }
        // With an `nM` cadence the year step is part of the month stride.
        (Cycle::NextNth(n), m) if *n > 1 && !matches!(m, Cycle::NextNth(_)) => {
            format!(", in every {} year, counted from the start's year", ordinal(*n))
        }
        _ => String::new(),
    }
}

fn stride(k: u32) -> String {
    format!("every {} month, counted from the start's month", ordinal(k))
}

/// A calendar day rule, either "every …" (every matching day of the eligible
/// months) or "on the …" (selected days of each eligible month), plus a note
/// on short months.
enum Days {
    Every(String),
    On(String, Option<&'static str>),
}

fn calendar_days(days: &DayCycle) -> Days {
    match days {
        DayCycle::ForEach => Days::Every("every day".into()),
        DayCycle::AsIs => Days::On(
            "on the start's day of the month".into(),
            Some("or the last day, in shorter months"),
        ),
        DayCycle::OnDays { days, .. } if days.is_empty() => Days::On("on the last day".into(), None),
        DayCycle::OnDays { days, option } => {
            let note = match option {
                LastDayOption::NA if days.iter().any(|d| *d > 28) => Some("skipped in shorter months"),
                LastDayOption::NA => None,
                LastDayOption::LastDay => Some("or the last day, in shorter months"),
                LastDayOption::NextMonthFirstDay => Some("or the 1st of the next month, in shorter months"),
                LastDayOption::NextMonthOverflow => Some("overflowing into the next month, in shorter months"),
            };
            Days::On(format!("on the {}", join_runs(days.iter().map(|d| (*d, ordinal(*d))))), note)
        }
        DayCycle::OnWeekDays { weekdays, option } => {
            let names = join_runs(
                weekdays.iter().map(|w| (w.0.num_days_from_monday(), weekday(w.0).to_string())),
            );
            match option {
                WeekdayOption::NA => Days::Every(format!("every {names}")),
                WeekdayOption::Starting(n) => {
                    Days::On(format!("on the {} {names}", ordinal(u32::from(n.unwrap_or(1)))), None)
                }
                WeekdayOption::Ending(None | Some(1)) => Days::On(format!("on the last {names}"), None),
                WeekdayOption::Ending(Some(n)) => {
                    Days::On(format!("on the {}-to-last {names}", ordinal(u32::from(*n))), None)
                }
            }
        }
        DayCycle::NthBizDay { calendar, option } => {
            let unit = match calendar {
                BizDayCalendar::BizDay => "business day",
                BizDayCalendar::WeekDay => "weekday",
            };
            let which = match option {
                WeekdayOption::Ending(None | Some(1)) => "last".to_string(),
                WeekdayOption::Ending(Some(n)) => format!("{}-to-last", ordinal(u32::from(*n))),
                WeekdayOption::Starting(n) => ordinal(u32::from(n.unwrap_or(1))),
                WeekdayOption::NA => ordinal(1),
            };
            Days::On(format!("on the {which} {unit}"), None)
        }
        DayCycle::NextNth(..) => unreachable!("step rules are described separately"),
    }
}

fn step_days(spec: &Spec, n: u32, unit: &NextNthDayOption) -> String {
    let (unit, first) = match unit {
        NextNthDayOption::Regular => ("day", "the 1st"),
        NextNthDayOption::BizDay => ("business day", "the first business day"),
        NextNthDayOption::WeekDay => ("weekday", "the first weekday"),
    };
    let years = years(spec);
    match months(spec) {
        // Month and day steps form one clock.
        Months::Stride(k) => {
            format!("every {} and {}, counted from the start{years}", count(k, "month"), count(n, unit))
        }
        months => {
            let mut out = if n == 1 { format!("every {unit}") } else { format!("every {}", count(n, unit)) };
            let filtered = !years.is_empty() || matches!(months, Months::In(_));
            if let Months::In(ms) = months {
                out += &format!(" in {ms}");
            }
            out += &years;
            if n > 1 {
                out += ", counted from the start";
            }
            if filtered {
                out += &format!(", restarting on {first} of the next eligible month after a gap");
            }
            out
        }
    }
}

fn adjustment(adj: &BizDayAdjustment) -> String {
    let dir = |d: &Direction| match d {
        Direction::Nearest => "nearest",
        Direction::Next => "next",
        Direction::Prev => "previous",
    };
    match adj {
        BizDayAdjustment::NA => String::new(),
        BizDayAdjustment::Weekday(d) => {
            format!(", moved to the {} weekday when it falls on a weekend", dir(d))
        }
        BizDayAdjustment::BizDay(d) => {
            format!(", moved to the {} business day when it is not one", dir(d))
        }
        BizDayAdjustment::ModifiedFollowing => ", moved to the next business day when it is not one, \
             or to the previous one if that is in the next month"
            .to_string(),
        BizDayAdjustment::ModifiedPreceding => ", moved to the previous business day when it is not one, \
             or to the next one if that is in the previous month"
            .to_string(),
        BizDayAdjustment::Prev(n) => format!(", moved {} earlier", count(*n, "business day")),
        BizDayAdjustment::Next(n) => format!(", moved {} later", count(*n, "business day")),
    }
}

pub(crate) fn describe(spec: &Spec) -> String {
    let mut out = match &spec.days {
        DayCycle::NextNth(n, unit) => step_days(spec, *n, unit),
        days => {
            // For a calendar rule, every month from the start is every month.
            let months = match months(spec) {
                Months::Stride(1) => Months::All,
                months => months,
            };
            let (mut out, note) = match (calendar_days(days), months) {
                (Days::Every(d), Months::All) => (d, None),
                (Days::Every(d), Months::In(ms)) => (format!("{d} in {ms}"), None),
                (Days::Every(d), Months::Stride(k)) => (format!("{d} in {}", stride(k)), None),
                (Days::On(d, note), Months::All) => (format!("{d} of every month"), note),
                (Days::On(d, note), Months::In(ms)) => (format!("{d} of {ms}"), note),
                (Days::On(d, note), Months::Stride(k)) => (format!("{d} of {}", stride(k)), note),
            };
            if let Some(note) = note {
                out += &format!(" ({note})");
            }
            out + &years(spec)
        }
    };
    if let Some(adj) = &spec.biz_day_adj {
        out += &adjustment(adj);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_describe() {
        for (spec, expected) in [
            ("YY-MM-DD", "every day"),
            ("YY-MM-15", "on the 15th of every month"),
            ("YY-1M-15", "on the 15th of every month"),
            ("YY-3M-15", "on the 15th of every 3rd month, counted from the start's month"),
            ("1Y-3M-15", "on the 15th of every 15th month, counted from the start's month"),
            ("2Y-MM-15", "on the 15th of every month, in every 2nd year, counted from the start's year"),
            ("2025-[01,07]-L", "on the last day of January and July, in 2025"),
            ("YY-MM-[01,15]", "on the 1st and 15th of every month"),
            ("YY-MM-31", "on the 31st of every month (skipped in shorter months)"),
            ("YY-MM-31L", "on the 31st of every month (or the last day, in shorter months)"),
            ("YY-MM-31N", "on the 31st of every month (or the 1st of the next month, in shorter months)"),
            ("YY-MM-31O", "on the 31st of every month (overflowing into the next month, in shorter months)"),
            ("YY-MM-MON", "every Monday"),
            ("YY-MM-[MON-FRI]", "every Monday to Friday"),
            ("YY-MM-[MON-WED,FRI]", "every Monday to Wednesday and Friday"),
            ("YY-MM-[SAT-SUN]", "every Saturday and Sunday"),
            ("YY-[01-06]-[01-10,15]", "on the 1st to 10th and 15th of January to June"),
            ("[2025-2027]-12-L", "on the last day of December, in 2025 to 2027"),
            ("YY-[03,09]-[MON,FRI]", "every Monday and Friday in March and September"),
            ("YY-MM-WED#2", "on the 2nd Wednesday of every month"),
            ("YY-MM-FRI#L", "on the last Friday of every month"),
            ("YY-MM-FRI#2L", "on the 2nd-to-last Friday of every month"),
            ("YY-MM-BD#3", "on the 3rd business day of every month"),
            ("YY-MM-BD#L", "on the last business day of every month"),
            ("YY-[03,06,09,12]-BD#2L", "on the 2nd-to-last business day of March, June, September and December"),
            ("YY-MM-WD#1", "on the 1st weekday of every month"),
            ("YY-MM-1D", "every day"),
            ("YY-MM-7D", "every 7 days, counted from the start"),
            ("YY-MM-5BD", "every 5 business days, counted from the start"),
            ("YY-MM-1WD", "every weekday"),
            ("YY-1M-7D", "every 1 month and 7 days, counted from the start"),
            (
                "YY-01-4D",
                "every 4 days in January, counted from the start, restarting on the 1st of the next eligible month after a gap",
            ),
            ("YY-MM-DD~NB", "every day, moved to the next business day when it is not one"),
            ("YY-1M-L~W", "on the last day of every month, moved to the nearest weekday when it falls on a weekend"),
            ("YY-MM-L~2P", "on the last day of every month, moved 2 business days earlier"),
            (
                "YY-MM-31L~MF",
                "on the 31st of every month (or the last day, in shorter months), moved to the next business day \
                 when it is not one, or to the previous one if that is in the next month",
            ),
            ("YY-MM-11~1N", "on the 11th of every month, moved 1 business day later"),
        ] {
            let parsed: Spec = spec.parse().unwrap();
            assert_eq!(describe(&parsed), expected, "{spec}");
        }
    }

    #[test]
    fn test_ordinal() {
        for (n, s) in [(1, "1st"), (2, "2nd"), (3, "3rd"), (4, "4th"), (11, "11th"), (12, "12th"), (13, "13th"), (21, "21st"), (22, "22nd"), (31, "31st")] {
            assert_eq!(ordinal(n), s);
        }
    }
}
