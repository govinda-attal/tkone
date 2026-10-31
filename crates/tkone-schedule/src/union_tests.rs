//! Union specs (`A | B`): merged results, and resuming them from a stored
//! start with `resume_with_start`.

use crate::biz_day::WeekendSkipper;
use crate::{date, datetime, time, Error, IntervalMode, Occurrence, TimeAnchor};
use chrono::{DateTime, FixedOffset, TimeZone};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

const N: usize = 40;

fn ny(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Tz> {
    chrono_tz::America::New_York.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
}

fn stored(t: &DateTime<Tz>) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(&t.to_rfc3339()).unwrap()
}

fn fmt(t: &DateTime<Tz>) -> String {
    t.format("%m-%d %H:%M").to_string()
}

// ---------------------------------------------------------------------------
// Merged results
// ---------------------------------------------------------------------------

#[test]
fn test_date_union() {
    let got: Vec<String> = date::SpecIteratorBuilder::new_after("YY-MM-BD#3 | YY-MM-L~PB", WeekendSkipper::new(), ny(2025, 1, 1, 0, 0))
        .build()
        .unwrap()
        .take(4)
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    // 3rd business day and last business day of each month.
    assert_eq!(got, ["01-03", "01-31", "02-05", "02-28"]);
}

#[test]
fn test_union_keeps_one_occurrence_per_actual_time() {
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-15 | YY-MM-[01,15]", WeekendSkipper::new(), ny(2025, 1, 1, 0, 0))
        .build()
        .unwrap()
        .take(3)
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got, ["01-15", "02-01", "02-15"]);
    // Same actual time, different adjustment: the first listed part wins.
    let first = date::SpecIteratorBuilder::new_after("YY-MM-15~NB | YY-MM-15", WeekendSkipper::new(), ny(2025, 3, 1, 0, 0))
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(first, Occurrence::AdjustedLater(ny(2025, 3, 15, 0, 0), ny(2025, 3, 17, 0, 0)));
}

#[test]
fn test_time_union_covers_a_partial_hour_window() {
    // Every 15 minutes from 09:30 to 16:00.
    let spec = "09:[30,45]:00 | 10-15:15M:00 | 16:00:00";
    let got: Vec<String> = time::SpecIteratorBuilder::new_after(spec, ny(2025, 1, 6, 9, 0))
        .with_time_anchor(TimeAnchor::Midnight)
        .build()
        .unwrap()
        .take(28)
        .map(|t| Ok(fmt(&t)))
        .collect()
        .unwrap();
    assert_eq!(got[..3], ["01-06 09:30", "01-06 09:45", "01-06 10:00"]);
    assert_eq!(got[25..], ["01-06 15:45", "01-06 16:00", "01-07 09:30"]);
}

#[test]
fn test_datetime_union() {
    let spec = "YY-MM-[MON-FRI]T09:[30,45]:00 | YY-MM-[MON-FRI]T10-15:15M:00 | YY-MM-[MON-FRI]T16:00:00";
    let got: Vec<String> = datetime::SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), ny(2025, 1, 10, 15, 50))
        .with_time_anchor(TimeAnchor::Midnight)
        .build()
        .unwrap()
        .take(3)
        .map(|o| Ok(fmt(o.observed())))
        .collect()
        .unwrap();
    // Fri 15:50 → 16:00, then Monday 09:30.
    assert_eq!(got, ["01-10 16:00", "01-13 09:30", "01-13 09:45"]);

    // Different date rules per part.
    let got: Vec<String> = datetime::SpecIteratorBuilder::new_after("YY-MM-BD#1T09:00:00 | YY-MM-FRI#LT17:00:00", WeekendSkipper::new(), ny(2025, 1, 1, 12, 0))
        .build()
        .unwrap()
        .take(3)
        .map(|o| Ok(fmt(o.observed())))
        .collect()
        .unwrap();
    assert_eq!(got, ["01-31 17:00", "02-03 09:00", "02-28 17:00"]);
}

#[test]
fn test_union_with_end_and_end_spec() {
    let got: Vec<_> = time::SpecIteratorBuilder::new_with_start("HH:00:00 | HH:30:00", ny(2025, 1, 1, 9, 0))
        .with_end(ny(2025, 1, 1, 10, 30))
        .build()
        .unwrap()
        .map(|t| Ok(fmt(&t)))
        .collect()
        .unwrap();
    assert_eq!(got, ["01-01 09:00", "01-01 09:30", "01-01 10:00", "01-01 10:30"]);
    let got: Vec<_> = time::SpecIteratorBuilder::new_with_start("1H:00:00", ny(2025, 1, 1, 9, 0))
        .with_end_spec("12:00:00 | 11:00:00")
        .build()
        .unwrap()
        .map(|t| Ok(fmt(&t)))
        .collect()
        .unwrap();
    assert_eq!(got, ["01-01 09:00", "01-01 10:00", "01-01 11:00"]);
}

// ---------------------------------------------------------------------------
// Resuming with the stored start
// ---------------------------------------------------------------------------

fn check_date(spec: &str, start: DateTime<Tz>) {
    let bdp = WeekendSkipper::new();
    let continuous: Vec<_> = date::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start)
        .build()
        .unwrap()
        .take(N)
        .collect()
        .unwrap();
    for i in 0..continuous.len() - 1 {
        let next = date::SpecIteratorBuilder::resume_with_start(spec, bdp.clone(), start.timezone(), stored(&start), stored(continuous[i].actual()))
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), Some(&continuous[i + 1]), "date spec {spec} after #{i}");
    }
}

fn check_datetime(spec: &str, start: DateTime<Tz>, mode: IntervalMode, anchor: TimeAnchor) {
    let bdp = WeekendSkipper::new();
    let continuous: Vec<_> = datetime::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start)
        .with_interval_mode(mode)
        .with_time_anchor(anchor)
        .build()
        .unwrap()
        .take(N)
        .collect()
        .unwrap();
    for i in 0..continuous.len() - 1 {
        let next = datetime::SpecIteratorBuilder::resume_with_start(spec, bdp.clone(), start.timezone(), stored(&start), stored(continuous[i].actual()))
            .with_interval_mode(mode)
            .with_time_anchor(anchor)
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), Some(&continuous[i + 1]), "datetime spec {spec} ({mode:?}, {anchor:?}) after #{i}");
    }
}

fn check_time(spec: &str, start: DateTime<Tz>, mode: IntervalMode, anchor: TimeAnchor) {
    let continuous: Vec<_> = time::SpecIteratorBuilder::new_with_start(spec, start)
        .with_interval_mode(mode)
        .with_time_anchor(anchor)
        .build()
        .unwrap()
        .take(N)
        .collect()
        .unwrap();
    for i in 0..continuous.len() - 1 {
        let next = time::SpecIteratorBuilder::resume_with_start(spec, start.timezone(), stored(&start), stored(&continuous[i]))
            .with_interval_mode(mode)
            .with_time_anchor(anchor)
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), Some(&continuous[i + 1]), "time spec {spec} ({mode:?}, {anchor:?}) after #{i}");
    }
}

#[test]
fn test_resume_union_date_specs() {
    for (spec, start) in [
        ("YY-MM-BD#3 | YY-MM-L~PB", ny(2025, 1, 3, 0, 0)),
        ("YY-MM-7D | YY-MM-15", ny(2025, 1, 2, 0, 0)), // a step part and a calendar part
        ("YY-3M-15 | YY-MM-FRI#L", ny(2025, 2, 15, 0, 0)), // cadence counted from the start
        ("YY-MM-5BD | YY-MM-[01,20]~NB", ny(2025, 1, 6, 0, 0)),
        ("YY-MM-[MON-FRI]/4W[1,2] | YY-MM-SAT/4W[3]", ny(2025, 1, 6, 0, 0)),
        ("YY-MM-8D[1-4] | YY-MM-15", ny(2025, 1, 1, 0, 0)),
    ] {
        check_date(spec, start);
    }
}

#[test]
fn test_resume_union_time_specs() {
    use IntervalMode::WallClock;
    use TimeAnchor::{Midnight, Start};
    for (spec, start, anchor) in [
        ("HH:15M:00 | HH:20M:00", ny(2025, 1, 1, 9, 7), Start), // two grids counted from the start
        ("09:[30,45]:00 | 10-15:15M:00 | 16:00:00", ny(2025, 1, 6, 9, 30), Midnight),
        ("1H:00:00 | 02:30:00", ny(2025, 11, 1, 0, 0), Start), // fixed part recurs daily; fall-back
        ("2H:00:00 | 3H:00:00", ny(2025, 1, 1, 9, 0), Start),
    ] {
        check_time(spec, start, WallClock, anchor);
    }
}

#[test]
fn test_resume_union_datetime_specs() {
    use IntervalMode::WallClock;
    use TimeAnchor::{Midnight, Start};
    for (spec, start, anchor) in [
        ("YY-MM-[MON-FRI]T09:[30,45]:00 | YY-MM-[MON-FRI]T10-15:15M:00 | YY-MM-[MON-FRI]T16:00:00", ny(2025, 1, 6, 9, 30), Midnight),
        ("YY-MM-7DT09:00:00 | YY-MM-15T17:00:00", ny(2025, 1, 2, 9, 0), Start),
        ("YY-MM-DDT2H:15:00 | YY-MM-DDT3H:00:00", ny(2025, 1, 1, 9, 15), Start),
        ("YY-MM-DDT1H:00:00 | YY-MM-DDT02:30:00", ny(2025, 3, 8, 0, 0), Start), // spring-forward
        ("YY-MM-DD~PBT12H:00:00 | YY-MM-BD#1T09:00:00", ny(2025, 1, 9, 12, 0), Start),
    ] {
        check_datetime(spec, start, WallClock, anchor);
    }
}

/// For a plain spec, `resume_with_start` matches `resume_after`.
#[test]
fn test_resume_with_start_on_plain_specs() {
    use IntervalMode::{Elapsed, WallClock};
    use TimeAnchor::{Midnight, Start};
    for (spec, start) in [
        ("YY-3M-15", ny(2025, 1, 15, 9, 0)),
        ("YY-1M-7D", ny(2024, 12, 1, 0, 0)),
        ("YY-01-4D", ny(2025, 1, 1, 0, 0)),
        ("YY-MM-DD~PB", ny(2025, 1, 9, 0, 0)),
        ("YY-MM-BD#3", ny(2025, 1, 3, 0, 0)),
        // Cycles with positions: an occurrence can be anywhere in its cycle.
        ("YY-MM-[MON-FRI]/4W[1,2]~NB", ny(2025, 1, 6, 0, 0)),
        ("YY-MM-8D[1-4]", ny(2025, 1, 29, 0, 0)),
        ("YY-[01-02]-8D[1-4]", ny(2025, 1, 1, 0, 0)),
        ("YY-3M[1-2]-15", ny(2025, 1, 15, 0, 0)),
        ("4Y[1,3]-06-15", ny(2025, 6, 15, 0, 0)),
    ] {
        check_date(spec, start);
    }
    for (spec, start, mode, anchor) in [
        ("YY-MM-DDT2H:00:00", ny(2025, 1, 15, 19, 15), WallClock, Start),
        ("YY-1M-15T1H:MM:SS", ny(2025, 1, 15, 21, 22), WallClock, Start),
        ("YY-MM-DDT1H:00:00", ny(2025, 11, 2, 0, 0), Elapsed, Start),
        ("YY-MM-DDTHH:30M:00", ny(2025, 3, 9, 1, 0), Elapsed, Midnight),
        ("YY-MM-BD#3T[09,17]:00:00", ny(2025, 1, 3, 9, 0), WallClock, Start),
        ("YY-MM-8D[1-4]T07:00:00", ny(2025, 3, 6, 7, 0), WallClock, Start), // across spring-forward
        ("YY-MM-[MON-FRI]/4W[1,2]THH:30M:00", ny(2025, 1, 6, 9, 0), WallClock, Midnight),
    ] {
        check_datetime(spec, start, mode, anchor);
    }
    for (spec, start, mode) in [
        ("HH:30M:00", ny(2025, 3, 9, 1, 0), WallClock),
        ("1H:00:00", ny(2025, 11, 2, 0, 0), Elapsed),
        ("1H:MM:SS", ny(2025, 1, 1, 9, 22), WallClock),
    ] {
        check_time(spec, start, mode, Start);
    }
}

#[test]
fn test_union_resume_errors() {
    let bdp = WeekendSkipper::new();
    let tz = chrono_tz::America::New_York;
    let (start, last) = (stored(&ny(2025, 1, 1, 9, 0)), stored(&ny(2025, 1, 1, 10, 0)));
    let unsupported = |r: Result<(), Error>| matches!(r, Err(Error::UnsupportedResume(_)));

    assert!(unsupported(date::SpecIteratorBuilder::resume_after("YY-MM-01 | YY-MM-15", bdp.clone(), tz, last).build().map(|_| ())));
    assert!(unsupported(date::SpecIteratorBuilder::resume_after("YY-MM-8D[1-4]", bdp.clone(), tz, last).build().map(|_| ())));
    assert!(unsupported(
        datetime::SpecIteratorBuilder::resume_after("YY-MM-DD/4W[1,2]T09:00:00", bdp.clone(), tz, last).build().map(|_| ())
    ));
    // A cycle counted from its first position resumes from any occurrence.
    assert!(date::SpecIteratorBuilder::resume_after("YY-MM-[TUE,THU]/2W", bdp.clone(), tz, last).build().is_ok());
    assert!(unsupported(time::SpecIteratorBuilder::resume_after("HH:00:00 | HH:30:00", tz, last).build().map(|_| ())));
    assert!(unsupported(
        datetime::SpecIteratorBuilder::resume_after("YY-MM-DDT09:00:00 | YY-MM-DDT17:00:00", bdp.clone(), tz, last).build().map(|_| ())
    ));
    assert!(unsupported(
        time::SpecIteratorBuilder::resume_with_start("HH:00:00 | HH:30:00", tz, start, last)
            .with_interval_mode(IntervalMode::Elapsed)
            .build()
            .map(|_| ())
    ));
    assert!(unsupported(
        datetime::SpecIteratorBuilder::resume_with_start("YY-MM-DDT1H:00:00 | YY-MM-DDT02:30:00", bdp, tz, start, last)
            .with_interval_mode(IntervalMode::Elapsed)
            .build()
            .map(|_| ())
    ));
}
