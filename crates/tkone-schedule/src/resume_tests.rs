//! Resuming from a persisted occurrence must give the same series as one
//! continuous iterator.
//!
//! An application that stores each occurrence and later computes the next one
//! stores `actual()` as RFC 3339 and calls `resume_after(spec, …, tz, stored)`.
//! Each check here runs a spec both ways — continuously, and by resuming after
//! every occurrence from its stored `actual` — and compares.

use crate::biz_day::WeekendSkipper;
use crate::{date, datetime, time, IntervalMode, Occurrence, TimeAnchor};
use chrono::{DateTime, FixedOffset, TimeZone};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

const N: usize = 40;

/// What a database round trip of `actual().to_rfc3339()` gives back.
fn stored(t: &DateTime<Tz>) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(&t.to_rfc3339()).unwrap()
}

fn check_date(spec: &str, start: DateTime<Tz>) {
    let bdp = WeekendSkipper::new();
    let continuous: Vec<_> = date::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start)
        .build()
        .unwrap()
        .take(N)
        .collect()
        .unwrap();
    let mut resumed = vec![continuous[0].clone()];
    while resumed.len() < continuous.len() {
        let cursor = stored(resumed.last().unwrap().actual());
        let next = date::SpecIteratorBuilder::resume_after(spec, bdp.clone(), start.timezone(), cursor)
            .build()
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        resumed.push(next);
    }
    assert_eq!(resumed, continuous, "date spec {spec}");
}

fn check_datetime(spec: &str, start: DateTime<Tz>, mode: IntervalMode, anchor: TimeAnchor) {
    let bdp = WeekendSkipper::new();
    let build = |b: datetime::SpecIteratorBuilder<Tz, WeekendSkipper, _, _, _>| b;
    let continuous: Vec<Occurrence<DateTime<Tz>>> =
        build(datetime::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start))
            .with_interval_mode(mode)
            .with_time_anchor(anchor)
            .build()
            .unwrap()
            .take(N)
            .collect()
            .unwrap();
    let mut resumed = vec![continuous[0].clone()];
    while resumed.len() < continuous.len() {
        let cursor = stored(resumed.last().unwrap().actual());
        let next = datetime::SpecIteratorBuilder::resume_after(spec, bdp.clone(), start.timezone(), cursor)
            .with_interval_mode(mode)
            .with_time_anchor(anchor)
            .build()
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        resumed.push(next);
    }
    assert_eq!(resumed, continuous, "datetime spec {spec} ({mode:?}, {anchor:?})");
}

fn check_time(spec: &str, start: DateTime<Tz>, mode: IntervalMode) {
    let continuous: Vec<DateTime<Tz>> = time::SpecIteratorBuilder::new_with_start(spec, start)
        .with_interval_mode(mode)
        .build()
        .unwrap()
        .take(N)
        .collect()
        .unwrap();
    let mut resumed = vec![continuous[0]];
    while resumed.len() < continuous.len() {
        let next = time::SpecIteratorBuilder::resume_after(spec, start.timezone(), stored(resumed.last().unwrap()))
            .with_interval_mode(mode)
            .build()
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        resumed.push(next);
    }
    assert_eq!(resumed, continuous, "time spec {spec} ({mode:?})");
}

fn ny(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Tz> {
    chrono_tz::America::New_York.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
}

#[test]
fn test_resume_date_specs() {
    for (spec, start) in [
        ("YY-3M-15", ny(2025, 1, 15, 9, 0)),
        ("2Y-MM-15", ny(2025, 11, 15, 0, 0)),
        ("1Y-3M-15", ny(2025, 6, 15, 0, 0)),
        ("2Y-[03,09]-01", ny(2025, 3, 1, 0, 0)),
        ("YY-01-4D", ny(2025, 1, 1, 0, 0)),
        ("YY-1M-7D", ny(2024, 12, 1, 0, 0)),
        ("YY-MM-5BD", ny(2025, 1, 1, 0, 0)),
        ("YY-MM-DD~NB", ny(2025, 1, 9, 0, 0)),
        ("YY-MM-DD~PB", ny(2025, 1, 9, 0, 0)),
        ("YY-1M-31N", ny(2024, 12, 31, 0, 0)),
        ("YY-1M-31O", ny(2024, 12, 31, 0, 0)),
        ("YY-1M-L~2N", ny(2025, 1, 31, 0, 0)),
        ("YY-MM-FRI#L", ny(2025, 1, 31, 0, 0)),
    ] {
        check_date(spec, start);
    }
    // a skipped day
    let apia = chrono_tz::Pacific::Apia.with_ymd_and_hms(2011, 12, 27, 0, 0, 0).unwrap();
    check_date("YY-MM-DD", apia);
}

/// An occurrence moved by a DST gap is stored as `02:30-05:00` (its scheduled
/// wall-clock time), so resuming continues at 02:30, not at the shifted 03:30.
#[test]
fn test_resume_across_dst_gap_keeps_wall_clock_time() {
    check_date("YY-MM-DD", ny(2025, 3, 7, 2, 30));
    let start = ny(2025, 3, 7, 14, 30);
    check_datetime("YY-MM-DDT02:30:00", start, IntervalMode::WallClock, TimeAnchor::Start);
    check_datetime("YY-MM-DDT_:_:_", ny(2025, 3, 7, 2, 30), IntervalMode::WallClock, TimeAnchor::Start);
}

#[test]
fn test_resume_datetime_specs() {
    use IntervalMode::{Elapsed, WallClock};
    use TimeAnchor::{Midnight, Start};
    for (spec, start, mode, anchor) in [
        ("YY-MM-DDT2H:00:00", ny(2025, 1, 15, 19, 15), WallClock, Start),
        ("YY-MM-DDT2H:00:00", ny(2025, 1, 15, 19, 15), WallClock, Midnight),
        ("YY-1M-15T1H:MM:SS", ny(2025, 1, 15, 21, 22), WallClock, Start),
        ("YY-MM-DDT09:30M:00", ny(2025, 1, 15, 9, 0), WallClock, Start),
        ("YY-MM-DD~PBT12H:00:00", ny(2025, 1, 9, 12, 0), WallClock, Start),
        ("YY-1M-31L~NBT6H:00:00", ny(2025, 4, 30, 0, 0), WallClock, Start),
        ("YY-MM-DDT1H:00:00", ny(2025, 3, 9, 0, 0), WallClock, Start), // spring-forward
        ("YY-MM-DDT1H:00:00", ny(2025, 11, 2, 0, 0), WallClock, Start), // fall-back
        ("YY-MM-DDT1H:00:00", ny(2025, 11, 2, 0, 0), Elapsed, Start),
        ("YY-MM-DDTHH:30M:00", ny(2025, 3, 9, 1, 0), Elapsed, Midnight),
    ] {
        check_datetime(spec, start, mode, anchor);
    }
}

#[test]
fn test_resume_time_specs() {
    use IntervalMode::{Elapsed, WallClock};
    for (spec, start, mode) in [
        ("1H:00:00", ny(2025, 3, 9, 0, 0), WallClock),
        ("HH:30M:00", ny(2025, 3, 9, 1, 0), WallClock),
        ("1H:00:00", ny(2025, 11, 2, 0, 0), WallClock),
        ("1H:00:00", ny(2025, 11, 2, 0, 0), Elapsed),
        ("6H:00:00", ny(2025, 3, 8, 6, 0), Elapsed),
        ("1H:MM:SS", ny(2025, 1, 1, 9, 22), WallClock),
        ("09:30M:00", ny(2025, 1, 1, 9, 0), WallClock),
    ] {
        check_time(spec, start, mode);
    }
}

/// A stored schedule with an end: resuming with `.with_end(end)` until
/// `Ok(None)` gives the same bounded series as one continuous iterator.
#[test]
fn test_resume_with_end() {
    let bdp = WeekendSkipper::new();

    // date
    let (spec, start, end) = ("YY-MM-DD~NB", ny(2025, 1, 9, 0, 0), ny(2025, 1, 21, 0, 0));
    let continuous: Vec<_> = date::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start)
        .with_end(end)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    let mut resumed = vec![continuous[0].clone()];
    while let Some(next) = date::SpecIteratorBuilder::resume_after(
        spec,
        bdp.clone(),
        start.timezone(),
        stored(resumed.last().unwrap().actual()),
    )
    .with_end(end)
    .build()
    .unwrap()
    .next()
    .unwrap()
    {
        resumed.push(next);
    }
    assert_eq!(resumed, continuous, "date");

    // datetime
    let (spec, start, end) = ("YY-MM-DDT6H:00:00", ny(2025, 3, 8, 6, 0), ny(2025, 3, 10, 12, 0));
    let continuous: Vec<_> =
        datetime::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start)
            .with_end(end)
            .build()
            .unwrap()
            .collect()
            .unwrap();
    let mut resumed = vec![continuous[0].clone()];
    while let Some(next) = datetime::SpecIteratorBuilder::resume_after(
        spec,
        bdp.clone(),
        start.timezone(),
        stored(resumed.last().unwrap().actual()),
    )
    .with_end(end)
    .build()
    .unwrap()
    .next()
    .unwrap()
    {
        resumed.push(next);
    }
    assert_eq!(resumed, continuous, "datetime");
    assert_eq!(*resumed.last().unwrap().observed(), end, "end is inclusive");

    // time
    let (spec, start, end) = ("HH:30M:00", ny(2025, 3, 9, 0, 0), ny(2025, 3, 9, 5, 0));
    let continuous: Vec<_> = time::SpecIteratorBuilder::new_with_start(spec, start)
        .with_end(end)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    let mut resumed = vec![continuous[0]];
    while let Some(next) =
        time::SpecIteratorBuilder::resume_after(spec, start.timezone(), stored(resumed.last().unwrap()))
            .with_end(end)
            .build()
            .unwrap()
            .next()
            .unwrap()
    {
        resumed.push(next);
    }
    assert_eq!(resumed, continuous, "time");
}
