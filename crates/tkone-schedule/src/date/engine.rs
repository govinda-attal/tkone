//! Occurrence engine for date specs.
//!
//! A date spec is evaluated in two layers:
//!
//! 1. **Periods** — the year and month rules together decide which calendar
//!    months `(year, month)` are eligible.
//! 2. **Days** — the day rule picks dates inside eligible periods.
//!
//! Day rules come in two kinds:
//!
//! - *Calendar* rules (`DD`, `15`, `[01,15]`, `L`, `31L`, `31N`, `31O`, `MON`,
//!   `[MON,FRI]`, `WED#2`, `FRI#L`) are a pure function of the period: list the
//!   matching days and take the first one after the cursor.
//! - *Step* rules (`nD`, `nBD`, `nWD`) advance from the previous occurrence.
//!   With an `nM` month cadence the month step and the day step compose into a
//!   single clock (`YY-1M-7D` = +1 month and +7 days per tick). Otherwise the
//!   step rolls forward while it stays inside eligible periods, and restarts at
//!   the first day of the next eligible period once it leaves them
//!   (`YY-01-4D` = Jan 1, 5, …, 29, then Jan 1 of the next year).
//!
//! `nY` and `nM` cadences are aligned to [`IterContext::anchor`]. When both are
//! present they fold into one stride of `12·n + m` months (`1Y-3M` = every 15
//! months).

use chrono::{Datelike, Days, Months, NaiveDate, NaiveDateTime, NaiveTime};

use crate::biz_day::{BizDayProcessor, WeekendSkipper};
use crate::date::spec::{
    BizDayAdjustment, Cycle, DayCycle, LastDayOption, NextNthDayOption, Spec, WeekdayOption,
};
use crate::utils::WeekdayStartingMonday;
use crate::{prelude::*, Occurrence};

/// Upper bound on loop passes that make no progress towards a result: eligible
/// periods with no matching day, years without an eligible month, or clock
/// ticks outside the year rule. Specs that can never match (e.g. `YY-02-30`)
/// fail with [`Error::IteratorNotConverged`] instead of looping forever.
const MAX_SCAN: u32 = 10_000;

#[derive(Clone, Debug)]
pub(super) struct IterContext<BDP: BizDayProcessor> {
    /// Origin for `nY` / `nM` cadences.
    pub anchor: NaiveDateTime,
    pub bd_processor: BDP,
}

/// The next raw occurrence strictly after `after`, before any `~` adjustment.
///
/// Occurrences keep the time-of-day of `after`.
pub(super) fn next_after<BDP: BizDayProcessor>(
    spec: &Spec,
    after: NaiveDateTime,
    ctx: &IterContext<BDP>,
) -> Result<Option<Occurrence<NaiveDateTime>>> {
    match &spec.days {
        DayCycle::NextNth(n, unit) => next_step(spec, *n, unit, after, ctx),
        days => next_calendar(spec, days, after, ctx),
    }
}

/// Apply the spec's `~` business day adjustment to a raw occurrence.
///
/// `~nP` / `~nN` shift unconditionally. The directional variants only shift
/// when the observed date is not already a weekday (`~W`, `~NW`, `~PW`) or a
/// business day (`~B`, `~NB`, `~PB`).
pub(super) fn adjust<BDP: BizDayProcessor>(
    spec: &Spec,
    raw: Occurrence<NaiveDateTime>,
    ctx: &IterContext<BDP>,
) -> Result<Occurrence<NaiveDateTime>> {
    let Some(adj) = &spec.biz_day_adj else {
        return Ok(raw);
    };
    let (actual, observed) = raw.as_tuple();
    let (actual, observed) = (*actual, *observed);
    let bdp = &ctx.bd_processor;
    let adjusted = match adj {
        BizDayAdjustment::NA => return Ok(raw),
        BizDayAdjustment::Prev(n) => {
            return Ok(Occurrence::AdjustedEarlier(actual, bdp.sub(&observed, *n)?))
        }
        BizDayAdjustment::Next(n) => {
            return Ok(Occurrence::AdjustedLater(actual, bdp.add(&observed, *n)?))
        }
        // Weekday rules only look at Sat/Sun, so a holiday on a weekday stays put.
        BizDayAdjustment::Weekday(dir) => {
            let ws = WeekendSkipper::new();
            if ws.is_biz_day(&observed)? {
                return Ok(raw);
            }
            ws.find_biz_day(&observed, dir.clone())?
        }
        BizDayAdjustment::BizDay(dir) => {
            if bdp.is_biz_day(&observed)? {
                return Ok(raw);
            }
            bdp.find_biz_day(&observed, dir.clone())?
        }
    };
    Ok(if adjusted > actual {
        Occurrence::AdjustedLater(actual, adjusted)
    } else if adjusted < actual {
        Occurrence::AdjustedEarlier(actual, adjusted)
    } else {
        Occurrence::Exact(adjusted)
    })
}

// ---------------------------------------------------------------------------
// Periods
// ---------------------------------------------------------------------------

/// A calendar month.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Period {
    year: i32,
    month: u32,
}

impl Period {
    fn of(date: NaiveDate) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
        }
    }

    /// Months since year 0; cadence arithmetic works on this index.
    fn index(self) -> i64 {
        i64::from(self.year) * 12 + i64::from(self.month) - 1
    }

    fn from_index(index: i64) -> Option<Self> {
        Some(Self {
            year: i32::try_from(index.div_euclid(12)).ok()?,
            month: index.rem_euclid(12) as u32 + 1,
        })
    }

    fn succ(self) -> Option<Self> {
        Self::from_index(self.index() + 1)
    }

    fn first_day(self) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(self.year, self.month, 1)
    }

    fn last_day(self) -> Option<NaiveDate> {
        self.succ()?.first_day()?.pred_opt()
    }
}

/// The month stride when the month rule is an `nM` cadence. An `nY` year rule
/// folds into the same stride.
fn month_stride(spec: &Spec) -> Option<i64> {
    match (&spec.years, &spec.months) {
        (Cycle::NextNth(ny), Cycle::NextNth(nm)) => Some(12 * i64::from(*ny) + i64::from(*nm)),
        (_, Cycle::NextNth(nm)) => Some(i64::from(*nm)),
        _ => None,
    }
}

/// Whether `year` passes the year rule. With a month stride an `nY` rule is
/// already part of the stride, so it passes every year.
fn year_matches(spec: &Spec, year: i32, anchor: Period) -> bool {
    match &spec.years {
        Cycle::AsIs | Cycle::ForEach => true,
        Cycle::NextNth(_) if month_stride(spec).is_some() => true,
        Cycle::Values(years) => u32::try_from(year).is_ok_and(|y| years.contains(&y)),
        Cycle::NextNth(n) => {
            (i64::from(year) - i64::from(anchor.year)).rem_euclid(i64::from(*n)) == 0
        }
    }
}

/// True once `year` is beyond every year an explicit year list allows.
fn past_last_year(years: &Cycle, year: i32) -> bool {
    match years {
        Cycle::Values(years) => years
            .last()
            .is_none_or(|&last| i64::from(year) > i64::from(last)),
        _ => false,
    }
}

fn period_matches(spec: &Spec, p: Period, anchor: Period) -> bool {
    let month_ok = match (&spec.months, month_stride(spec)) {
        (_, Some(stride)) => (p.index() - anchor.index()).rem_euclid(stride) == 0,
        (Cycle::Values(months), None) => months.contains(&p.month),
        _ => true,
    };
    month_ok && year_matches(spec, p.year, anchor)
}

/// The first eligible period at or after `from`.
fn next_period(spec: &Spec, from: Period, anchor: Period) -> Result<Option<Period>> {
    if let Some(stride) = month_stride(spec) {
        let offset = (from.index() - anchor.index()).rem_euclid(stride);
        let mut index = from.index() + if offset == 0 { 0 } else { stride - offset };
        for _ in 0..MAX_SCAN {
            let Some(p) = Period::from_index(index) else {
                return Ok(None);
            };
            if past_last_year(&spec.years, p.year) {
                return Ok(None);
            }
            if year_matches(spec, p.year, anchor) {
                return Ok(Some(p));
            }
            index += stride;
        }
        return Err(Error::IteratorNotConverged);
    }

    let mut p = from;
    for _ in 0..MAX_SCAN {
        let Some(year) = next_year(&spec.years, p.year, anchor) else {
            return Ok(None);
        };
        let first_month = if year == p.year { p.month } else { 1 };
        let month = match &spec.months {
            Cycle::Values(months) => months.range(first_month..=12).next().copied(),
            _ => Some(first_month),
        };
        if let Some(month) = month {
            return Ok(Some(Period { year, month }));
        }
        let Some(next_year) = year.checked_add(1) else {
            return Ok(None);
        };
        p = Period {
            year: next_year,
            month: 1,
        };
    }
    Err(Error::IteratorNotConverged)
}

/// The first year at or after `year` that passes the year rule (ignoring any
/// month stride).
fn next_year(years: &Cycle, year: i32, anchor: Period) -> Option<i32> {
    match years {
        Cycle::AsIs | Cycle::ForEach => Some(year),
        Cycle::Values(years) => {
            let from = u32::try_from(year).unwrap_or(0);
            years
                .range(from..)
                .next()
                .and_then(|&y| i32::try_from(y).ok())
        }
        Cycle::NextNth(n) => {
            let n = i64::from(*n);
            let offset = (i64::from(year) - i64::from(anchor.year)).rem_euclid(n);
            i32::try_from(i64::from(year) + if offset == 0 { 0 } else { n - offset }).ok()
        }
    }
}

// ---------------------------------------------------------------------------
// Calendar day rules
// ---------------------------------------------------------------------------

fn next_calendar<BDP: BizDayProcessor>(
    spec: &Spec,
    days: &DayCycle,
    after: NaiveDateTime,
    ctx: &IterContext<BDP>,
) -> Result<Option<Occurrence<NaiveDateTime>>> {
    let anchor = Period::of(ctx.anchor.date());
    let mut from = Period::of(after.date());
    for _ in 0..MAX_SCAN {
        let Some(p) = next_period(spec, from, anchor)? else {
            return Ok(None);
        };
        let found = calendar_days(days, p, ctx.anchor.date())
            .into_iter()
            .find(|occ| *occ.actual() > after.date());
        if let Some(occ) = found {
            return Ok(Some(at_time(occ, after.time())));
        }
        let Some(succ) = p.succ() else {
            return Ok(None);
        };
        from = succ;
    }
    Err(Error::IteratorNotConverged)
}

/// Dates matched by a calendar day rule inside `p`, ascending by actual date.
///
/// A day beyond the end of the month is dropped (no suffix), clamped to the
/// last day (`L`), or reported as `AdjustedLater(last_day, observed)` with
/// `observed` in the next month (`N`, `O`).
fn calendar_days(days: &DayCycle, p: Period, anchor: NaiveDate) -> Vec<Occurrence<NaiveDate>> {
    let (Some(first), Some(last)) = (p.first_day(), p.last_day()) else {
        return Vec::new();
    };
    let month = || first.iter_days().take_while(move |d| *d <= last);

    match days {
        DayCycle::ForEach => month().map(Occurrence::Exact).collect(),
        DayCycle::AsIs => vec![Occurrence::Exact(
            first.with_day(anchor.day()).unwrap_or(last),
        )],
        DayCycle::OnDays { days, option } if days.is_empty() => match option {
            LastDayOption::LastDay => vec![Occurrence::Exact(last)],
            _ => Vec::new(),
        },
        DayCycle::OnDays { days, option } => {
            let mut out: Vec<Occurrence<NaiveDate>> = Vec::new();
            for &day in days {
                let occ = if day <= last.day() {
                    first.with_day(day).map(Occurrence::Exact)
                } else {
                    match option {
                        LastDayOption::NA => None,
                        LastDayOption::LastDay => Some(Occurrence::Exact(last)),
                        LastDayOption::NextMonthFirstDay => last
                            .succ_opt()
                            .map(|observed| Occurrence::AdjustedLater(last, observed)),
                        LastDayOption::NextMonthOverflow => last
                            .checked_add_days(Days::new(u64::from(day - last.day())))
                            .map(|observed| Occurrence::AdjustedLater(last, observed)),
                    }
                };
                // Every overflowing day lands on the last day; keep the first.
                if let Some(occ) = occ {
                    if out.last().is_none_or(|prev| prev.actual() != occ.actual()) {
                        out.push(occ);
                    }
                }
            }
            out
        }
        DayCycle::OnWeekDays { weekdays, option } => {
            let mut dates: Vec<NaiveDate> = match option {
                WeekdayOption::NA => month()
                    .filter(|d| weekdays.contains(&WeekdayStartingMonday(d.weekday())))
                    .collect(),
                WeekdayOption::Starting(n) => weekdays
                    .iter()
                    .filter_map(|wd| {
                        NaiveDate::from_weekday_of_month_opt(p.year, p.month, wd.0, n.unwrap_or(1))
                    })
                    .collect(),
                WeekdayOption::Ending(n) => weekdays
                    .iter()
                    .filter_map(|wd| nth_last_weekday(first, last, wd.0, n.unwrap_or(1)))
                    .collect(),
            };
            dates.sort();
            dates.into_iter().map(Occurrence::Exact).collect()
        }
        DayCycle::NextNth(..) => Vec::new(),
    }
}

/// The `n`th-to-last `weekday` between `first` and `last` (`n = 1` is the last).
fn nth_last_weekday(
    first: NaiveDate,
    last: NaiveDate,
    weekday: chrono::Weekday,
    n: u8,
) -> Option<NaiveDate> {
    if n == 0 {
        return None;
    }
    let back = (7 + last.weekday().num_days_from_monday() - weekday.num_days_from_monday()) % 7;
    let date = last.checked_sub_days(Days::new(u64::from(back) + 7 * (u64::from(n) - 1)))?;
    (date >= first).then_some(date)
}

fn at_time(occ: Occurrence<NaiveDate>, time: NaiveTime) -> Occurrence<NaiveDateTime> {
    match occ {
        Occurrence::Exact(d) => Occurrence::Exact(d.and_time(time)),
        Occurrence::AdjustedLater(a, o) => {
            Occurrence::AdjustedLater(a.and_time(time), o.and_time(time))
        }
        Occurrence::AdjustedEarlier(a, o) => {
            Occurrence::AdjustedEarlier(a.and_time(time), o.and_time(time))
        }
    }
}

// ---------------------------------------------------------------------------
// Step day rules
// ---------------------------------------------------------------------------

fn next_step<BDP: BizDayProcessor>(
    spec: &Spec,
    n: u32,
    unit: &NextNthDayOption,
    after: NaiveDateTime,
    ctx: &IterContext<BDP>,
) -> Result<Option<Occurrence<NaiveDateTime>>> {
    let anchor = Period::of(ctx.anchor.date());

    // Month cadence: month step and day step form one clock.
    if let Some(stride) = month_stride(spec) {
        let months = Months::new(u32::try_from(stride).map_err(|_| Error::IteratorNotConverged)?);
        let mut cur = after;
        for _ in 0..MAX_SCAN {
            let Some(moved) = cur.checked_add_months(months) else {
                return Ok(None);
            };
            let Some(next) = step(unit, n, moved, ctx)? else {
                return Ok(None);
            };
            if past_last_year(&spec.years, next.year()) {
                return Ok(None);
            }
            if year_matches(spec, next.year(), anchor) {
                return Ok(Some(Occurrence::Exact(next)));
            }
            cur = next;
        }
        return Err(Error::IteratorNotConverged);
    }

    // Rolling step: keep it while it stays in eligible periods.
    let Some(next) = step(unit, n, after, ctx)? else {
        return Ok(None);
    };
    if next <= after {
        return Err(Error::IteratorNotConverged);
    }
    if period_matches(spec, Period::of(next.date()), anchor) {
        return Ok(Some(Occurrence::Exact(next)));
    }

    // Left the eligible periods: restart at the next eligible one.
    let Some(from) = Period::of(after.date()).succ() else {
        return Ok(None);
    };
    let Some(p) = next_period(spec, from, anchor)? else {
        return Ok(None);
    };
    Ok(period_start(unit, p, after.time(), ctx)?.map(Occurrence::Exact))
}

fn step<BDP: BizDayProcessor>(
    unit: &NextNthDayOption,
    n: u32,
    from: NaiveDateTime,
    ctx: &IterContext<BDP>,
) -> Result<Option<NaiveDateTime>> {
    match unit {
        NextNthDayOption::Regular => Ok(from.checked_add_days(Days::new(u64::from(n)))),
        NextNthDayOption::BizDay => ctx.bd_processor.add(&from, n).map(Some),
        NextNthDayOption::WeekDay => WeekendSkipper::new().add(&from, n).map(Some),
    }
}

/// Where a step sequence restarts inside period `p`: the 1st, or for `nBD` /
/// `nWD` the first business day / weekday on or after it.
fn period_start<BDP: BizDayProcessor>(
    unit: &NextNthDayOption,
    p: Period,
    time: NaiveTime,
    ctx: &IterContext<BDP>,
) -> Result<Option<NaiveDateTime>> {
    let Some(first) = p.first_day().map(|d| d.and_time(time)) else {
        return Ok(None);
    };
    let start = match unit {
        NextNthDayOption::Regular => first,
        NextNthDayOption::BizDay if ctx.bd_processor.is_biz_day(&first)? => first,
        NextNthDayOption::BizDay => ctx.bd_processor.add(&first, 1)?,
        NextNthDayOption::WeekDay => {
            let ws = WeekendSkipper::new();
            if ws.is_biz_day(&first)? {
                first
            } else {
                ws.add(&first, 1)?
            }
        }
    };
    Ok(Some(start))
}
