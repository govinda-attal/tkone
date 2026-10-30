//! Next-occurrence search for time specs.
//!
//! A time spec describes a regular grid of times:
//!
//! - **Step** — the sum of the `Every` fields (`1H:30M:00` = 90 minutes), or
//!   one unit of the finest `ForEach` field when there is no `Every`
//!   (`HH:MM:00` = 1 minute). The unit of that field is the *driver*.
//! - **Origin** — an anchor time with the fixed (`At` / `Values`) fields finer
//!   than the driver applied. Fields finer than the driver that are `ForEach`
//!   or `AsIs` keep the anchor's value, so `1H:MM:SS` from 09:22:45 carries
//!   `:22:45`. A list there gives one grid per value: `HH:[00,30]:00` runs at
//!   :00 and :30.
//! - **Filter** — fixed fields coarser than the driver restrict the grid to a
//!   window: `09:30M:00` is every 30 minutes, but only during hour 9;
//!   `09-16:15M:00` is every 15 minutes from 09:00 to 16:45.
//!
//! The next occurrence is the earliest grid point after the cursor that passes
//! the filter. A spec without a driver (`09:30:00`, `[09,12,17]:00:00`,
//! `_:_:_`) names fixed times of day.
//!
//! Specs have whole-second resolution: the origin drops any sub-second part of
//! the anchor.

use chrono::{Duration, NaiveDateTime, Timelike};

use super::spec::{Cycle, Spec};

/// Upper bound on filter windows skipped while searching; guards grids that
/// never enter a window.
const MAX_SCAN: u32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Unit {
    Second,
    Minute,
    Hour,
}

impl Unit {
    pub(super) fn duration(self, n: i64) -> Duration {
        match self {
            Unit::Second => Duration::seconds(n),
            Unit::Minute => Duration::minutes(n),
            Unit::Hour => Duration::hours(n),
        }
    }
}

pub(super) fn fields(spec: &Spec) -> [(Unit, &Cycle); 3] {
    [
        (Unit::Second, &spec.seconds),
        (Unit::Minute, &spec.minutes),
        (Unit::Hour, &spec.hours),
    ]
}

/// The finest unit with an `Every`, or failing that the finest `ForEach`.
pub(super) fn driver(spec: &Spec) -> Option<Unit> {
    let finest = |pred: fn(&Cycle) -> bool| fields(spec).into_iter().find(|(_, c)| pred(c)).map(|(u, _)| u);
    finest(|c| matches!(c, Cycle::Every(_))).or_else(|| finest(|c| matches!(c, Cycle::ForEach)))
}

/// Whether the spec repeats within a day (`false` for `09:30:00`, `_:_:_`).
pub(crate) fn has_step(spec: &Spec) -> bool {
    driver(spec).is_some()
}

pub(super) fn step(spec: &Spec, driver: Unit) -> Duration {
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
/// For a spec without a driver this is the first named time of day after
/// `after` on its date, or the next day's first if they have all passed.
pub(crate) fn next_after(spec: &Spec, anchor: NaiveDateTime, after: NaiveDateTime) -> Option<NaiveDateTime> {
    let anchor = anchor.with_nanosecond(0)?;
    let Some(driver) = driver(spec) else {
        return fixed_time(spec, anchor, after);
    };
    let step = step(spec, driver).num_nanoseconds()?;

    // One grid per combination of fixed values finer than the driver
    // (`HH:[00,30]:00` is an hourly grid at :00 and another at :30).
    let mut origins = vec![anchor];
    for (unit, cycle) in fields(spec) {
        if let (true, Some(values)) = (unit < driver, cycle.pinned()) {
            origins = origins
                .iter()
                .flat_map(|o| values.iter().map(move |v| set(*o, unit, u32::from(*v))))
                .collect::<Option<_>>()?;
        }
    }

    // Window fields: fixed fields coarser than the driver.
    let window = |unit: Unit| {
        fields(spec)
            .into_iter()
            .find(|(u, _)| *u == unit && *u > driver)
            .and_then(|(_, c)| c.pinned())
            .map(|vs| vs.into_iter().map(u32::from).collect::<Vec<_>>())
    };
    let (hours, minutes) = (window(Unit::Hour), window(Unit::Minute));

    origins
        .into_iter()
        .filter_map(|origin| next_on_grid(origin, step, after, hours.as_deref(), minutes.as_deref()))
        .min()
}

/// The first point of `origin + k·step` after `after` inside the window.
fn next_on_grid(
    origin: NaiveDateTime,
    step: i64,
    after: NaiveDateTime,
    hours: Option<&[u32]>,
    minutes: Option<&[u32]>,
) -> Option<NaiveDateTime> {
    let mut t = grid_after(origin, step, after, false)?;
    for _ in 0..MAX_SCAN {
        match window_start(hours, minutes, t)? {
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

/// `None` when `t` is inside the window the fixed hours / minutes allow;
/// otherwise `Some(start of the next window at or after t)`.
fn window_start(hours: Option<&[u32]>, minutes: Option<&[u32]>, t: NaiveDateTime) -> Option<Option<NaiveDateTime>> {
    let allows = |values: Option<&[u32]>, v: u32| values.is_none_or(|vs| vs.contains(&v));
    if allows(hours, t.hour()) && allows(minutes, t.minute()) {
        return Some(None);
    }
    let every_hour: Vec<u32> = (0..24).collect();
    let hours = hours.unwrap_or(&every_hour);
    let minutes = minutes.unwrap_or(&[0]);
    for date in [t.date(), t.date().succ_opt()?] {
        for h in hours {
            for m in minutes {
                let start = date.and_hms_opt(*h, *m, 0)?;
                if start >= t {
                    return Some(Some(start));
                }
            }
        }
    }
    None
}

/// The times of day named by a spec without a driver: every combination of
/// the fixed fields' values, with `AsIs` fields from `anchor`.
fn fixed_time(spec: &Spec, anchor: NaiveDateTime, after: NaiveDateTime) -> Option<NaiveDateTime> {
    let mut times = vec![after.date().and_time(anchor.time())];
    for (unit, cycle) in fields(spec) {
        if let Some(values) = cycle.pinned() {
            times = times
                .iter()
                .flat_map(|t| values.iter().map(move |v| set(*t, unit, u32::from(*v))))
                .collect::<Option<_>>()?;
        }
    }
    times.sort();
    match times.iter().find(|t| **t > after) {
        Some(today) => Some(*today),
        None => Some(after.date().succ_opt()?.and_time(times.first()?.time())),
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

    fn run(spec: &str, from: NaiveDateTime, n: usize) -> Vec<NaiveDateTime> {
        let spec: Spec = spec.parse().unwrap();
        let mut out = Vec::new();
        let mut cur = from;
        for _ in 0..n {
            cur = next_after(&spec, from, cur).unwrap();
            out.push(cur);
        }
        out
    }

    #[test]
    fn test_hour_list_and_range_windows() {
        let day2 = |h, m| at(h, m, 0) + Duration::days(1);
        assert_eq!(
            run("[09,12,17]:00:00", at(10, 0, 0), 4),
            vec![at(12, 0, 0), at(17, 0, 0), day2(9, 0), day2(12, 0)]
        );
        let t = run("09-16:15M:00", at(8, 0, 0), 33);
        assert_eq!(t[0], at(9, 0, 0));
        assert_eq!(t[31], at(16, 45, 0));
        assert_eq!(t[32], day2(9, 0));
        assert_eq!(
            run("[09,14]:30M:00", at(9, 0, 0), 4),
            vec![at(9, 30, 0), at(14, 0, 0), at(14, 30, 0), day2(9, 0)]
        );
    }

    #[test]
    fn test_finer_list_gives_one_grid_per_value() {
        assert_eq!(
            run("HH:[00,30]:00", at(9, 10, 0), 3),
            vec![at(9, 30, 0), at(10, 0, 0), at(10, 30, 0)]
        );
        assert_eq!(
            run("09-10:[15,45]:00", at(0, 0, 0), 5),
            vec![at(9, 15, 0), at(9, 45, 0), at(10, 15, 0), at(10, 45, 0), at(9, 15, 0) + Duration::days(1)]
        );
        // minute window when seconds drive
        assert_eq!(
            run("HH:[00,30]:20S", at(9, 0, 0), 4),
            vec![at(9, 0, 20), at(9, 0, 40), at(9, 30, 0), at(9, 30, 20)]
        );
    }

    #[test]
    fn test_fixed_time_rolls_to_next_day() {
        assert_eq!(next("13:00:00", at(9, 0, 0), at(9, 0, 0)), Some(at(13, 0, 0)));
        assert_eq!(next("13:00:00", at(14, 0, 0), at(14, 0, 0)), Some(at(13, 0, 0) + Duration::days(1)));
    }
}
