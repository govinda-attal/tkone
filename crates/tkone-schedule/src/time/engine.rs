//! Next-occurrence search for time specs.
//!
//! A time spec describes a regular grid of times:
//!
//! - **Step** — the sum of the `Every` fields (`1H:30M:00` = 90 minutes), or
//!   one unit of the finest `ForEach` field when there is no `Every`
//!   (`HH:MM:00` = 1 minute). The unit of that field is the *driver*.
//! - **Origin** — an anchor time with the fixed (`At`) fields finer than the
//!   driver applied. Fields finer than the driver that are `ForEach` or `AsIs`
//!   keep the anchor's value, so `1H:MM:SS` from 09:22:45 carries `:22:45`.
//! - **Filter** — `At` fields coarser than the driver restrict the grid to a
//!   window: `09:30M:00` is every 30 minutes, but only during hour 9.
//!
//! The next occurrence is the first grid point after the cursor that passes
//! the filter. A spec without a driver (`09:30:00`, `_:_:_`) names a single
//! time of day.
//!
//! Specs have whole-second resolution: the origin drops any sub-second part of
//! the anchor.

use chrono::{Duration, NaiveDateTime, Timelike};

use super::spec::{Cycle, Spec};

/// Upper bound on filter windows skipped while searching; guards grids that
/// never enter a window.
const MAX_SCAN: u32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Unit {
    Second,
    Minute,
    Hour,
}

impl Unit {
    fn duration(self, n: i64) -> Duration {
        match self {
            Unit::Second => Duration::seconds(n),
            Unit::Minute => Duration::minutes(n),
            Unit::Hour => Duration::hours(n),
        }
    }
}

fn fields(spec: &Spec) -> [(Unit, &Cycle); 3] {
    [
        (Unit::Second, &spec.seconds),
        (Unit::Minute, &spec.minutes),
        (Unit::Hour, &spec.hours),
    ]
}

/// The finest unit with an `Every`, or failing that the finest `ForEach`.
fn driver(spec: &Spec) -> Option<Unit> {
    let finest = |pred: fn(&Cycle) -> bool| fields(spec).into_iter().find(|(_, c)| pred(c)).map(|(u, _)| u);
    finest(|c| matches!(c, Cycle::Every(_))).or_else(|| finest(|c| matches!(c, Cycle::ForEach)))
}

/// Whether the spec repeats within a day (`false` for `09:30:00`, `_:_:_`).
pub(crate) fn has_step(spec: &Spec) -> bool {
    driver(spec).is_some()
}

fn step(spec: &Spec, driver: Unit) -> Duration {
    let every: Duration = fields(spec)
        .into_iter()
        .filter_map(|(u, c)| match c {
            Cycle::Every(n) => Some(u.duration(i64::from(*n))),
            _ => None,
        })
        .fold(Duration::zero(), |acc, d| acc + d);
    if every.is_zero() { driver.duration(1) } else { every }
}

/// The earliest time strictly after `after` that `spec` produces, taking the
/// grid's phase and carried fields from `anchor`.
///
/// For a spec without a driver this is the named time of day on `after`'s
/// date, or the next day's if it has passed.
pub(crate) fn next_after(spec: &Spec, anchor: NaiveDateTime, after: NaiveDateTime) -> Option<NaiveDateTime> {
    let anchor = anchor.with_nanosecond(0)?;
    let Some(driver) = driver(spec) else {
        return fixed_time(spec, anchor, after);
    };
    let step = step(spec, driver).num_nanoseconds()?;

    let mut origin = anchor;
    for (unit, cycle) in fields(spec) {
        if let (true, Cycle::At(v)) = (unit < driver, cycle) {
            origin = set(origin, unit, u32::from(*v))?;
        }
    }

    // Window fields: `At` coarser than the driver.
    let pinned = |unit: Unit| match fields(spec).into_iter().find(|(u, _)| *u == unit) {
        Some((u, Cycle::At(v))) if u > driver => Some(u32::from(*v)),
        _ => None,
    };
    let (hour, minute) = (pinned(Unit::Hour), pinned(Unit::Minute));

    let mut t = grid_after(origin, step, after, false)?;
    for _ in 0..MAX_SCAN {
        match window_start(hour, minute, t)? {
            None => return Some(t),
            Some(window) => t = grid_after(origin, step, window, true)?,
        }
    }
    None
}

/// First point of `origin + k·step` after `bound` (or at it, if `inclusive`).
fn grid_after(origin: NaiveDateTime, step: i64, bound: NaiveDateTime, inclusive: bool) -> Option<NaiveDateTime> {
    let offset = (bound - origin).num_nanoseconds()?;
    let k = if inclusive {
        offset.div_euclid(step) + i64::from(offset.rem_euclid(step) != 0)
    } else {
        offset.div_euclid(step) + 1
    };
    origin.checked_add_signed(Duration::nanoseconds(k.checked_mul(step)?))
}

/// `None` when `t` is inside the window the pinned hour / minute allow;
/// otherwise `Some(start of the next window at or after t)`.
fn window_start(hour: Option<u32>, minute: Option<u32>, t: NaiveDateTime) -> Option<Option<NaiveDateTime>> {
    if hour.is_none_or(|h| t.hour() == h) && minute.is_none_or(|m| t.minute() == m) {
        return Some(None);
    }
    let hours = hour.map_or(0..=23, |h| h..=h);
    for date in [t.date(), t.date().succ_opt()?] {
        for h in hours.clone() {
            let start = date.and_hms_opt(h, minute.unwrap_or(0), 0)?;
            if start >= t {
                return Some(Some(start));
            }
        }
    }
    None
}

/// The single time of day named by a spec without a driver: `At` fields as
/// given, `AsIs` fields from `anchor`.
fn fixed_time(spec: &Spec, anchor: NaiveDateTime, after: NaiveDateTime) -> Option<NaiveDateTime> {
    let mut time = anchor.time();
    for (unit, cycle) in fields(spec) {
        if let Cycle::At(v) = cycle {
            time = set(after.date().and_time(time), unit, u32::from(*v))?.time();
        }
    }
    let today = after.date().and_time(time);
    if today > after {
        Some(today)
    } else {
        Some(after.date().succ_opt()?.and_time(time))
    }
}

fn set(t: NaiveDateTime, unit: Unit, value: u32) -> Option<NaiveDateTime> {
    match unit {
        Unit::Second => t.with_second(value),
        Unit::Minute => t.with_minute(value),
        Unit::Hour => t.with_hour(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap().and_hms_opt(h, m, s).unwrap()
    }

    fn next(spec: &str, anchor: NaiveDateTime, after: NaiveDateTime) -> Option<NaiveDateTime> {
        next_after(&spec.parse().unwrap(), anchor, after)
    }

    #[test]
    fn test_pinned_finer_field_does_not_skip_current_slot() {
        assert_eq!(next("HH:30:00", at(10, 0, 0), at(10, 0, 0)), Some(at(10, 30, 0)));
        assert_eq!(next("HH:MM:30", at(10, 0, 0), at(10, 0, 0)), Some(at(10, 0, 30)));
    }

    #[test]
    fn test_pinned_coarser_field_is_a_daily_window() {
        let spec = "09:30M:00";
        assert_eq!(next(spec, at(8, 0, 0), at(8, 0, 0)), Some(at(9, 0, 0)));
        assert_eq!(next(spec, at(9, 0, 0), at(9, 0, 0)), Some(at(9, 30, 0)));
        // after the window: tomorrow 09:00
        assert_eq!(next(spec, at(9, 30, 0), at(9, 30, 0)), Some(at(9, 0, 0) + Duration::days(1)));
    }

    #[test]
    fn test_unaligned_cursor_snaps_to_grid() {
        assert_eq!(next("1H:00:00", at(9, 30, 0), at(9, 30, 0)), Some(at(10, 0, 0)));
        assert_eq!(next("2H:00:00", at(9, 15, 0), at(9, 15, 0)), Some(at(11, 0, 0)));
    }

    #[test]
    fn test_fixed_time_rolls_to_next_day() {
        assert_eq!(next("13:00:00", at(9, 0, 0), at(9, 0, 0)), Some(at(13, 0, 0)));
        assert_eq!(next("13:00:00", at(14, 0, 0), at(14, 0, 0)), Some(at(13, 0, 0) + Duration::days(1)));
    }
}
