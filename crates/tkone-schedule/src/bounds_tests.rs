//! A start, end and time zone written in the spec: `<spec> @start ..end`.

use crate::biz_day::WeekendSkipper;
use crate::{date, datetime, time, IntervalMode, Occurrence};
use chrono::{DateTime, FixedOffset, TimeZone};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

fn ny(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Tz> {
    chrono_tz::America::New_York.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
}

fn stored(t: &DateTime<Tz>) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(&t.to_rfc3339()).unwrap()
}

fn dates(spec: &str, after: DateTime<Tz>, n: usize) -> Vec<String> {
    date::SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), after)
        .build()
        .unwrap()
        .take(n)
        .map(|o| Ok(o.observed().format("%Y-%m-%d").to_string()))
        .collect()
        .unwrap()
}

#[test]
fn test_text_start_anchors_and_bounds() {
    let early = ny(2025, 1, 1, 0, 0);
    // The start is inclusive: a matching start is the first result.
    assert_eq!(dates("YY-MM-15 @2025-01-15", early, 2), ["2025-01-15", "2025-02-15"]);
    assert_eq!(dates("YY-MM-15 @2025-01-10", early, 1), ["2025-01-15"]);
    // Cadences count from it, even when iterating from a later cursor.
    assert_eq!(dates("YY-3M-15 @2025-02-15", early, 2), ["2025-02-15", "2025-05-15"]);
    assert_eq!(dates("YY-3M-15 @2025-02-15", ny(2025, 6, 1, 0, 0), 1), ["2025-08-15"]);
    // Without a written start the cursor anchors the cadence instead.
    assert_eq!(dates("YY-3M-15", ny(2025, 6, 1, 0, 0), 1), ["2025-06-15"]);
    // The end is inclusive; a date-only end covers the whole day.
    assert_eq!(dates("YY-MM-15 @2025-01-01 ..2025-03-15", early, 9), ["2025-01-15", "2025-02-15", "2025-03-15"]);
    assert_eq!(dates("YY-MM-15 ..2025-02-15", early, 9), ["2025-01-15", "2025-02-15"]);
    // Week cycles and positions count from the written start.
    assert_eq!(dates("YY-MM-[TUE,THU]/2W @2025-01-07", early, 4), ["2025-01-07", "2025-01-09", "2025-01-21", "2025-01-23"]);
    assert_eq!(dates("YY-MM-8D[1-2] @2025-01-03", early, 3), ["2025-01-03", "2025-01-04", "2025-01-11"]);
}

#[test]
fn test_text_start_with_step_rules() {
    let early = ny(2025, 1, 1, 0, 0);
    let bdp = WeekendSkipper::new();
    // A step starts on the written start (Sat 4 Jan) …
    assert_eq!(dates("YY-MM-7D @2025-01-04", early, 2), ["2025-01-04", "2025-01-11"]);
    // … or on the next business day for business-day steps …
    assert_eq!(dates("YY-MM-5BD @2025-01-04", early, 2), ["2025-01-06", "2025-01-13"]);
    // … and the start is adjusted like any other occurrence.
    let first = date::SpecIteratorBuilder::new_after("YY-MM-7D~NB @2025-01-04", bdp, early)
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(first, Occurrence::AdjustedLater(ny(2025, 1, 4, 0, 0), ny(2025, 1, 6, 0, 0)));
    // A start outside the eligible months: steps begin in the next one.
    assert_eq!(dates("YY-01-4D @2024-12-30", ny(2024, 12, 1, 0, 0), 2), ["2025-01-01", "2025-01-05"]);
}

#[test]
fn test_builder_start_and_end_take_precedence() {
    let bdp = WeekendSkipper::new();
    // `new_with_start` replaces the written start (and is yielded as-is).
    let got: Vec<_> = date::SpecIteratorBuilder::new_with_start("YY-3M-15 @2025-02-15", bdp.clone(), ny(2025, 3, 15, 0, 0))
        .build()
        .unwrap()
        .take(2)
        .map(|o| Ok(o.observed().format("%Y-%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got, ["2025-03-15", "2025-06-15"]);
    // `with_end` replaces the written end.
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-15 ..2025-02-15", bdp, ny(2025, 1, 1, 0, 0))
        .with_end(ny(2025, 4, 15, 0, 0))
        .build()
        .unwrap()
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got, ["01-15", "02-15", "03-15", "04-15"]);
}

#[test]
fn test_resume_after_with_written_start() {
    // With the start in the text, `resume_after` is enough for unions,
    // positions and counts.
    let bdp = WeekendSkipper::new();
    let tz = chrono_tz::America::New_York;
    for spec in [
        "YY-MM-8D[1-4] @2025-01-01",
        "YY-MM-[MON-FRI]/4W[1,2] | YY-MM-SAT/4W[3] @2025-01-06",
        "YY-MM-7D | YY-MM-15 ! YY-02-DD @2025-01-02 ..2025-06-30",
    ] {
        let continuous: Vec<_> = date::SpecIteratorBuilder::new_after(spec, bdp.clone(), ny(2024, 1, 1, 0, 0))
            .build()
            .unwrap()
            .take(30)
            .collect()
            .unwrap();
        // An unbounded spec continues past the 30 collected; a bounded one ends.
        let checked = if spec.contains("..") { continuous.len() } else { continuous.len() - 1 };
        for i in 0..checked {
            let next = date::SpecIteratorBuilder::resume_after(spec, bdp.clone(), tz, stored(continuous[i].actual()))
                .build()
                .unwrap()
                .next()
                .unwrap();
            assert_eq!(next.as_ref(), continuous.get(i + 1), "{spec} after #{i}");
        }
    }
    // A counted schedule with a written start.
    let spec = "YY-MM-15 @2025-01-15";
    let continuous: Vec<_> = date::SpecIteratorBuilder::new_after(spec, bdp.clone(), ny(2024, 1, 1, 0, 0))
        .with_count(3)
        .build()
        .unwrap()
        .collect()
        .unwrap();
    assert_eq!(continuous.len(), 3);
    for i in 0..continuous.len() {
        let next = date::SpecIteratorBuilder::resume_after(spec, bdp.clone(), tz, stored(continuous[i].actual()))
            .with_count(3)
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), continuous.get(i + 1), "counted after #{i}");
    }
}

#[test]
fn test_time_with_written_start() {
    let take = |spec: &str, after, n| -> Vec<String> {
        time::SpecIteratorBuilder::new_after(spec, after)
            .build()
            .unwrap()
            .take(n)
            .map(|t| Ok(t.format("%d %H:%M").to_string()))
            .collect()
            .unwrap()
    };
    let early = ny(2025, 1, 6, 0, 0);
    // The grid is counted from the written start, and includes it.
    assert_eq!(take("HH:15M:00 @2025-01-06T09:07:00", early, 2), ["06 09:07", "06 09:22"]);
    assert_eq!(take("1H:00:00 @2025-01-06T09:30:00", early, 2), ["06 10:00", "06 11:00"]);
    // Fixed times are yielded on the start's day.
    assert_eq!(take("[09,17]:00:00 @2025-01-06", early, 5), ["06 09:00", "06 17:00"]);
    // The written end stops it.
    assert_eq!(take("2H:00:00 @2025-01-06T08:00 ..2025-01-06T12:00", early, 9), ["06 08:00", "06 10:00", "06 12:00"]);
    // Elapsed mode: the first time comes from the start, later ones are real-time steps.
    let got: Vec<_> = time::SpecIteratorBuilder::new_after("6H:00:00 @2025-03-08T22:00", ny(2025, 3, 8, 0, 0))
        .with_interval_mode(IntervalMode::Elapsed)
        .build()
        .unwrap()
        .take(3)
        .map(|t| Ok(t.format("%d %H:%M %Z").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got, ["08 22:00 EST", "09 05:00 EDT", "09 11:00 EDT"]);
}

#[test]
fn test_datetime_with_written_start_and_zone() {
    let bdp = WeekendSkipper::new();
    // A start later in the day than the time: that day is skipped.
    let got: Vec<_> = datetime::SpecIteratorBuilder::new_after("YY-MM-DDT09:00:00 @2025-01-06T10:00:00", bdp.clone(), ny(2025, 1, 1, 0, 0))
        .build()
        .unwrap()
        .take(1)
        .collect()
        .unwrap();
    assert_eq!(*got[0].observed(), ny(2025, 1, 7, 9, 0));
    // The written zone resolves the start; times are read in the builder's zone.
    let got: Vec<_> = datetime::SpecIteratorBuilder::new_after(
        "YY-MM-DDT09:00:00 @2025-01-06T09:00:00[Europe/London]",
        bdp.clone(),
        ny(2025, 1, 1, 0, 0),
    )
    .build()
    .unwrap()
    .take(1)
    .collect()
    .unwrap();
    assert_eq!(*got[0].observed(), ny(2025, 1, 6, 9, 0)); // 04:00 New York < 09:00
    // `from_spec` reads everything, including the zone, from the text.
    let got: Vec<_> = datetime::SpecIteratorBuilder::from_spec("YY-MM-DDT09:00:00 @2025-03-29[Europe/London] ..2025-03-31", bdp.clone())
        .unwrap()
        .build()
        .unwrap()
        .map(|o| Ok(o.observed().to_rfc3339()))
        .collect()
        .unwrap();
    assert_eq!(got, ["2025-03-29T09:00:00+00:00", "2025-03-30T09:00:00+01:00", "2025-03-31T09:00:00+01:00"]);
    // Resuming a self-contained union.
    let spec = "YY-MM-[TUE,THU]/2WT18:00:00 | YY-MM-8D[1]T07:00:00 @2025-01-07[Europe/London] ..2025-04-30";
    let continuous: Vec<_> = datetime::SpecIteratorBuilder::from_spec(spec, bdp.clone()).unwrap().build().unwrap().collect().unwrap();
    assert!(continuous.len() > 10);
    for i in 0..continuous.len() {
        let next = datetime::SpecIteratorBuilder::resume_after(spec, bdp.clone(), chrono_tz::Europe::London, stored_tz(continuous[i].actual()))
            .build()
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(next.as_ref(), continuous.get(i + 1), "after #{i}");
    }
}

fn stored_tz(t: &DateTime<Tz>) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(&t.to_rfc3339()).unwrap()
}

#[test]
fn test_exclusions_and_describe_with_bounds() {
    // `with_exclusions` goes before the written start and end.
    let got: Vec<_> = date::SpecIteratorBuilder::new_after("YY-MM-DD @2025-12-24 ..2025-12-27", WeekendSkipper::new(), ny(2025, 1, 1, 0, 0))
        .with_exclusions("YY-12-25")
        .build()
        .unwrap()
        .map(|o| Ok(o.observed().format("%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got, ["24", "26", "27"]);
    assert_eq!(
        date::describe("YY-MM-15~NB ! YY-12-15 @2025-01-01 ..2026-12-31").unwrap(),
        "on the 15th of every month, moved to the next business day when it is not one, except on the 15th of December, from 1 January 2025 until 31 December 2026"
    );
    assert_eq!(
        datetime::describe("YY-MM-DDT09:00:00 @2025-01-06T09:00:00[Europe/London]").unwrap(),
        "every day; at 09:00:00, from 6 January 2025 09:00:00 (Europe/London)"
    );
    // Malformed bounds are errors.
    assert!(date::SpecIteratorBuilder::new_after("YY-MM-15 @soon", WeekendSkipper::new(), ny(2025, 1, 1, 0, 0)).build().is_err());
    assert!(time::SpecIteratorBuilder::from_spec("1H:00:00 @2025-01-06[Nowhere/City]").is_err());
}
