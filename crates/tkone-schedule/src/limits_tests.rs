//! `with_exclusions` and `with_count`, including resuming counted schedules.

use crate::biz_day::WeekendSkipper;
use crate::{date, datetime, time, Error, IntervalMode, Occurrence};
use chrono::{DateTime, FixedOffset, TimeZone};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

fn ny(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Tz> {
    chrono_tz::America::New_York.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
}

fn stored(t: &DateTime<Tz>) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(&t.to_rfc3339()).unwrap()
}

fn days(occurrences: &[Occurrence<DateTime<Tz>>]) -> Vec<String> {
    occurrences.iter().map(|o| o.observed().format("%m-%d").to_string()).collect()
}

#[test]
fn test_date_exclusions() {
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-DD", WeekendSkipper::new(), ny(2025, 12, 23, 0, 0))
        .with_exclusions("YY-12-25 | YY-01-01")
        .build()
        .unwrap()
        .take(10)
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["12-24", "12-26", "12-27", "12-28", "12-29", "12-30", "12-31", "01-02", "01-03", "01-04"]);

    // Matched on the actual (scheduled) date, not the adjusted one.
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-25~NB", WeekendSkipper::new(), ny(2025, 10, 1, 0, 0))
        .with_exclusions("YY-12-25")
        .build()
        .unwrap()
        .take(3)
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["10-27", "11-25", "01-26"]);

    // Day steps are not allowed in an exclusion spec.
    let err = date::SpecIteratorBuilder::new_after("YY-MM-DD", WeekendSkipper::new(), ny(2025, 1, 1, 0, 0))
        .with_exclusions("YY-MM-01 | YY-MM-7D")
        .build()
        .unwrap_err();
    assert!(matches!(err, Error::InvalidDateSpec(_)), "{err:?}");
}

#[test]
fn test_time_exclusions() {
    let got: Vec<String> = time::SpecIteratorBuilder::new_after("HH:30M:00", ny(2025, 1, 1, 10, 45))
        .with_time_anchor(crate::TimeAnchor::Midnight)
        .with_exclusions("12:MM:SS")
        .build()
        .unwrap()
        .take(4)
        .map(|t| Ok(t.format("%H:%M").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got, ["11:00", "11:30", "13:00", "13:30"]);
}

#[test]
fn test_datetime_exclusions() {
    let got: Vec<_> = datetime::SpecIteratorBuilder::new_after("YY-MM-[MON-FRI]T09:00:00", WeekendSkipper::new(), ny(2025, 12, 23, 12, 0))
        .with_exclusions("YY-12-25 | YY-12-26")
        .build()
        .unwrap()
        .take(3)
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["12-24", "12-29", "12-30"]);
}

#[test]
fn test_count() {
    let bdp = WeekendSkipper::new();
    // The start counts as the first result.
    let got: Vec<_> = date::SpecIteratorBuilder::new_with_start("YY-MM-15", bdp.clone(), ny(2025, 1, 15, 0, 0))
        .with_count(3)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["01-15", "02-15", "03-15"]);
    // Excluded dates are not counted.
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-15", bdp.clone(), ny(2025, 1, 1, 0, 0))
        .with_exclusions("YY-02-15")
        .with_count(3)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["01-15", "03-15", "04-15"]);
    // A union counts merged results; an end can stop it earlier.
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-01 | YY-MM-15", bdp.clone(), ny(2025, 1, 1, 0, 0))
        .with_count(5)
        .with_end(ny(2025, 2, 1, 0, 0))
        .build()
        .unwrap()
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["01-15", "02-01"]);

    let got: Vec<_> = time::SpecIteratorBuilder::new_with_start("1H:00:00", ny(2025, 1, 1, 9, 0))
        .with_count(3)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    assert_eq!(got, [ny(2025, 1, 1, 9, 0), ny(2025, 1, 1, 10, 0), ny(2025, 1, 1, 11, 0)]);

    let got: Vec<_> = datetime::SpecIteratorBuilder::new_after("YY-MM-BD#1T[09,17]:00:00", bdp, ny(2025, 1, 1, 12, 0))
        .with_count(3)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(*got[2].observed(), ny(2025, 2, 3, 17, 0));
}

/// Resuming a counted schedule with `resume_with_start` after every result
/// gives the same results as one iterator, and then `None`.
#[test]
fn test_resume_counted_schedules() {
    let bdp = WeekendSkipper::new();
    let tz = chrono_tz::America::New_York;

    let start = ny(2025, 1, 2, 0, 0);
    let continuous: Vec<_> = date::SpecIteratorBuilder::new_with_start("YY-MM-7D | YY-MM-15", bdp.clone(), start)
        .with_exclusions("YY-02-DD")
        .with_count(8)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    assert_eq!(continuous.len(), 8);
    for i in 0..continuous.len() {
        let next = date::SpecIteratorBuilder::resume_with_start("YY-MM-7D | YY-MM-15", bdp.clone(), tz, stored(&start), stored(continuous[i].actual()))
            .with_exclusions("YY-02-DD")
            .with_count(8)
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), continuous.get(i + 1), "date after #{i}");
    }

    // A union in elapsed mode can be resumed when counted: it is replayed.
    let start = ny(2025, 3, 8, 22, 0);
    let spec = "YY-MM-DDT3H:00:00 | YY-MM-DDT02:30:00";
    let continuous: Vec<_> = datetime::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start)
        .with_interval_mode(IntervalMode::Elapsed)
        .with_count(10)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    for i in 0..continuous.len() {
        let next = datetime::SpecIteratorBuilder::resume_with_start(spec, bdp.clone(), tz, stored(&start), stored(continuous[i].actual()))
            .with_interval_mode(IntervalMode::Elapsed)
            .with_count(10)
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), continuous.get(i + 1), "datetime after #{i}");
    }

    let start = ny(2025, 1, 1, 9, 0);
    let continuous: Vec<_> = time::SpecIteratorBuilder::new_with_start("HH:15M:00 | HH:20M:00", start)
        .with_exclusions("12:MM:SS")
        .with_count(12)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    for i in 0..continuous.len() {
        let next = time::SpecIteratorBuilder::resume_with_start("HH:15M:00 | HH:20M:00", tz, stored(&start), stored(&continuous[i]))
            .with_exclusions("12:MM:SS")
            .with_count(12)
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), continuous.get(i + 1), "time after #{i}");
    }
}

#[test]
fn test_count_needs_resume_with_start() {
    let tz = chrono_tz::America::New_York;
    let last = stored(&ny(2025, 1, 15, 0, 0));
    let unsupported = |r: Result<(), Error>| matches!(r, Err(Error::UnsupportedResume(_)));
    assert!(unsupported(
        date::SpecIteratorBuilder::resume_after("YY-MM-15", WeekendSkipper::new(), tz, last).with_count(3).build().map(|_| ())
    ));
    assert!(unsupported(time::SpecIteratorBuilder::resume_after("1H:00:00", tz, last).with_count(3).build().map(|_| ())));
    assert!(unsupported(
        datetime::SpecIteratorBuilder::resume_after("YY-MM-15T09:00:00", WeekendSkipper::new(), tz, last)
            .with_count(3)
            .build()
            .map(|_| ())
    ));
}

// ---------------------------------------------------------------------------
// Exclusions written in the spec: `A ! X`
// ---------------------------------------------------------------------------

#[test]
fn test_exclusions_in_the_spec() {
    let bdp = WeekendSkipper::new();
    let take = |spec: &str, n| -> Vec<String> {
        let got: Vec<_> = date::SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), ny(2025, 12, 23, 0, 0))
            .build()
            .unwrap()
            .take(n)
            .collect()
            .unwrap();
        days(&got)
    };
    assert_eq!(take("YY-MM-DD ! YY-12-25 | YY-01-01", 4), ["12-24", "12-26", "12-27", "12-28"]);
    // The exclusions apply to the whole union before them.
    assert_eq!(take("YY-MM-25 | YY-MM-01 ! YY-12-25 | YY-01-01", 3), ["01-25", "02-01", "02-25"]);
    // Written in the spec and with the builder: both apply.
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-DD ! YY-12-25", bdp.clone(), ny(2025, 12, 23, 0, 0))
        .with_exclusions("YY-12-26")
        .build()
        .unwrap()
        .take(2)
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["12-24", "12-27"]);
    // The naive iterators read it too.
    let naive: Vec<_> = date::NaiveSpecIterator::new_after("YY-MM-DD ! YY-12-25", bdp.clone(), ny(2025, 12, 24, 0, 0).naive_local())
        .unwrap()
        .take(1)
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(naive, ["12-26"]);
    let naive: Vec<_> = time::NaiveSpecIterator::new_after("1H:00:00 ! 12:MM:SS", ny(2025, 1, 1, 10, 30).naive_local())
        .unwrap()
        .take(2)
        .map(|t| Ok(t.format("%H:%M").to_string()))
        .collect()
        .unwrap();
    assert_eq!(naive, ["11:00", "13:00"]);

    let got: Vec<_> = datetime::SpecIteratorBuilder::new_after("YY-MM-[MON-FRI]T09:00:00 ! YY-12-25 | YY-12-26", bdp.clone(), ny(2025, 12, 23, 12, 0))
        .build()
        .unwrap()
        .take(2)
        .collect()
        .unwrap();
    assert_eq!(days(&got), ["12-24", "12-29"]);

    // An exclusion spec cannot have its own exclusions, or day steps.
    assert!(date::SpecIteratorBuilder::new_after("YY-MM-DD ! YY-12-DD ! YY-12-25", bdp.clone(), ny(2025, 1, 1, 0, 0)).build().is_err());
    assert!(datetime::SpecIteratorBuilder::new_after("YY-MM-DDT09:00:00 ! YY-MM-7D", bdp, ny(2025, 1, 1, 0, 0)).build().is_err());
}

#[test]
fn test_resume_with_exclusions_in_the_spec() {
    let bdp = WeekendSkipper::new();
    let tz = chrono_tz::America::New_York;
    // A plain spec whose exclusions are a union is still resumed with `resume_after`.
    let spec = "YY-MM-DD ! YY-12-25 | YY-01-01";
    let next = date::SpecIteratorBuilder::resume_after(spec, bdp.clone(), tz, stored(&ny(2025, 12, 24, 0, 0)))
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(*next.observed(), ny(2025, 12, 26, 0, 0));
    let spec = "YY-MM-DDT09:00:00 ! YY-12-25";
    let next = datetime::SpecIteratorBuilder::resume_after(spec, bdp.clone(), tz, stored(&ny(2025, 12, 24, 9, 0)))
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(*next.observed(), ny(2025, 12, 26, 9, 0));
    let next = time::SpecIteratorBuilder::resume_after("1H:00:00 ! 12:MM:SS", tz, stored(&ny(2025, 1, 1, 11, 0)))
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(next, ny(2025, 1, 1, 13, 0));
    // A union with exclusions is resumed with its start.
    let start = ny(2025, 12, 1, 0, 0);
    let spec = "YY-MM-7D | YY-MM-25 ! YY-12-25";
    let continuous: Vec<_> = date::SpecIteratorBuilder::new_with_start(spec, bdp.clone(), start)
        .build()
        .unwrap()
        .take(10)
        .collect()
        .unwrap();
    assert!(days(&continuous).iter().all(|d| d != "12-25"));
    for i in 0..continuous.len() - 1 {
        let next = date::SpecIteratorBuilder::resume_with_start(spec, bdp.clone(), tz, stored(&start), stored(continuous[i].actual()))
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), Some(&continuous[i + 1]), "after #{i}");
    }
}

#[test]
fn test_describe_exclusions() {
    assert_eq!(
        date::describe("YY-MM-DD ! YY-12-25 | YY-01-01").unwrap(),
        "every day, except on the 25th of December or on the 1st of January"
    );
    assert_eq!(
        datetime::describe("YY-MM-[MON-FRI]T09:00:00 ! YY-12-25").unwrap(),
        "every Monday to Friday; at 09:00:00, except on the 25th of December"
    );
    assert_eq!(
        time::describe("1H:00:00 ! 12:MM:SS").unwrap(),
        "every hour, at minute 00 and second 00, except during 12:00–12:59"
    );
}
