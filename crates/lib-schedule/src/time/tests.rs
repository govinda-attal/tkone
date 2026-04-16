use crate::time::{Cycle, Spec, SpecIteratorBuilder};
use chrono::{DateTime, TimeZone, Utc};
use fallible_iterator::FallibleIterator;
use std::str::FromStr;

struct TestCase<Tz: TimeZone> {
    spec: &'static str,
    start: DateTime<Tz>,
    take: usize,
    expected: Vec<DateTime<Tz>>,
}

fn run_cases<Tz: TimeZone + Clone>(cases: Vec<TestCase<Tz>>) {
    for tc in cases {
        let iter = SpecIteratorBuilder::new_with_start(tc.spec, tc.start)
            .build()
            .unwrap();
        let results: Vec<DateTime<_>> = iter.take(tc.take).collect().unwrap();
        assert_eq!(tc.expected, results, "spec: {}", tc.spec);
    }
}

// ---------------------------------------------------------------------------
// Group 1: Every-hours cadences  (nH:00:00) — specs 1-4
// ---------------------------------------------------------------------------

#[test]
fn test_every_hours() {
    let tz = Utc;
    run_cases(vec![
        // spec 1: Every hour, on the hour
        TestCase {
            spec: "1H:00:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 15, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 17, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 18, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 19, 0, 0).unwrap(),
            ],
        },
        // spec 2: Every 2 hours — crosses midnight
        TestCase {
            spec: "2H:00:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 8, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 8, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 18, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 20, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 22, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap(), // next day
                tz.with_ymd_and_hms(2025, 1, 2, 2, 0, 0).unwrap(),
            ],
        },
        // spec 3: Every 4 hours — six ticks per day
        TestCase {
            spec: "4H:00:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 4, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 8, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 20, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 4, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 8, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 12, 0, 0).unwrap(),
            ],
        },
        // spec 4: Every 6 hours — four ticks per day
        TestCase {
            spec: "6H:00:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 6, 0, 0).unwrap(),
            take: 8,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 6, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 18, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 6, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 18, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap(),
            ],
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 2: Every-minutes cadences  (HH:nM:00) — specs 5-8
// ---------------------------------------------------------------------------

#[test]
fn test_every_minutes() {
    let tz = Utc;
    run_cases(vec![
        // spec 5: Every 30 minutes
        TestCase {
            spec: "HH:30M:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 30, 0).unwrap(),
            ],
        },
        // spec 6: Every 15 minutes
        TestCase {
            spec: "HH:15M:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 15, 0).unwrap(),
            ],
        },
        // spec 7: Every 10 minutes
        TestCase {
            spec: "HH:10M:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 10, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 20, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 40, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 50, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 10, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 20, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 30, 0).unwrap(),
            ],
        },
        // spec 8: Every 5 minutes
        TestCase {
            spec: "HH:5M:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 5, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 10, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 20, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 25, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 35, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 40, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 45, 0).unwrap(),
            ],
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 3: Every-seconds cadences  (HH:MM:nS) — specs 9-10
// ---------------------------------------------------------------------------

#[test]
fn test_every_seconds() {
    let tz = Utc;
    run_cases(vec![
        // spec 9: Every 30 seconds — Every(30) drives; HH and MM (ForEach) carry
        TestCase {
            spec: "HH:MM:30S",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 30).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 30).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 2, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 2, 30).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 3, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 3, 30).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 4, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 4, 30).unwrap(),
            ],
        },
        // spec 10: Every 15 seconds
        TestCase {
            spec: "HH:MM:15S",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 30).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 30).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 2, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 2, 15).unwrap(),
            ],
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 4: ForEach driver semantics — specs 11-13
// No Every component; finest ForEach drives by 1 of its unit.
// ---------------------------------------------------------------------------

#[test]
fn test_foreach_driver() {
    let tz = Utc;
    run_cases(vec![
        // spec 11: HH:MM:SS — all ForEach → SS is finest → every second
        TestCase {
            spec: "HH:MM:SS",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 1).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 2).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 3).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 4).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 5).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 6).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 7).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 8).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 9).unwrap(),
            ],
        },
        // spec 12: HH:MM:00 — SS is At(0), so MM (ForEach) is finest → every minute at :00
        TestCase {
            spec: "HH:MM:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 2, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 3, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 4, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 5, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 6, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 7, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 8, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 9, 0).unwrap(),
            ],
        },
        // spec 13: HH:00:00 — SS and MM are At(0), HH (ForEach) is only driver → every hour
        //   Equivalent to 1H:00:00
        TestCase {
            spec: "HH:00:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 8,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 15, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 0, 0).unwrap(),
            ],
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 5: Every-hours with pinned minutes and seconds — specs 14-16
// ---------------------------------------------------------------------------

#[test]
fn test_every_hours_pinned() {
    let tz = Utc;
    run_cases(vec![
        // spec 14: Every hour at :30 past
        TestCase {
            spec: "1H:30:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
            take: 8,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 15, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 30, 0).unwrap(),
            ],
        },
        // spec 15: Every 2 hours at :15 past — crosses midnight
        TestCase {
            spec: "2H:15:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 8, 15, 0).unwrap(),
            take: 10,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 8, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 18, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 20, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 22, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 0, 15, 0).unwrap(), // next day
                tz.with_ymd_and_hms(2025, 1, 2, 2, 15, 0).unwrap(),
            ],
        },
        // spec 16: Every 3 hours at :45 past — crosses midnight
        TestCase {
            spec: "3H:45:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 45, 0).unwrap(),
            take: 8,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 15, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 18, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 21, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 0, 45, 0).unwrap(), // next day
                tz.with_ymd_and_hms(2025, 1, 2, 3, 45, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 2, 6, 45, 0).unwrap(),
            ],
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 6: Every-hours with ForEach carry — specs 17-18
// ForEach on minutes/seconds carries the start value unchanged each tick.
// ---------------------------------------------------------------------------

#[test]
fn test_every_hours_foreach_carry() {
    let tz = Utc;
    run_cases(vec![
        // spec 17: 1H:MM:00 — hours advance, minute carries from start, seconds pinned to 0
        TestCase {
            spec: "1H:MM:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 15, 0).unwrap(),
            take: 8,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 15, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 15, 0).unwrap(),
            ],
        },
        // spec 18: 1H:MM:SS — hours advance; both minute and second carry from start
        TestCase {
            spec: "1H:MM:SS",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 22, 45).unwrap(),
            take: 8,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 22, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 22, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 22, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 22, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 22, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 22, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 15, 22, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 16, 22, 45).unwrap(),
            ],
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 7: Every-minutes with pinned seconds — spec 19
// ---------------------------------------------------------------------------

#[test]
fn test_every_minutes_pinned_seconds() {
    let tz = Utc;
    run_cases(vec![
        // spec 19: HH:30M:15 — minutes advance by 30; hours carry; seconds pinned to :15
        TestCase {
            spec: "HH:30M:15",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 15).unwrap(),
            take: 8,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 30, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 30, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 15).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 30, 15).unwrap(),
            ],
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 8: Bounded by end spec — specs 20-21
// ---------------------------------------------------------------------------

#[test]
fn test_with_end_spec() {
    let tz = Utc;

    // spec 20: 1H:00:00 bounded by end spec 13:00:00
    // End computed once via new_after from start 09:00: At(13) hours → 13:00:00.
    // Iterator produces 09:00, 10:00, 11:00, 12:00, 13:00 then terminates.
    {
        let start = tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let iter = SpecIteratorBuilder::new_with_start("1H:00:00", start)
            .with_end_spec("13:00:00")
            .build()
            .unwrap();
        let results: Vec<DateTime<_>> = iter.collect().unwrap();
        assert_eq!(
            results,
            vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 0, 0).unwrap(),
            ],
            "spec 20: 1H:00:00 with end spec 13:00:00"
        );
    }

    // spec 21: HH:30M:00 bounded by end spec 3H:30M:SS
    // End computed once: Every(3)H + Every(30)M + ForEach(carry) S from 09:00:00 → 12:30:00.
    // Iterator produces 09:00, 09:30, …, 12:30 then terminates.
    {
        let start = tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let iter = SpecIteratorBuilder::new_with_start("HH:30M:00", start)
            .with_end_spec("3H:30M:SS")
            .build()
            .unwrap();
        let results: Vec<DateTime<_>> = iter.collect().unwrap();
        assert_eq!(
            results,
            vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 30, 0).unwrap(),
            ],
            "spec 21: HH:30M:00 with end spec 3H:30M:SS"
        );
    }
}

// ---------------------------------------------------------------------------
// Group 9: AsIs variant — spec 22
// _ (AsIs) keeps the current component unchanged.
// Primary use is in combined datetime specs; here we verify parsing and the
// no-advance property directly.
// ---------------------------------------------------------------------------

#[test]
fn test_asis_parsing() {
    // spec 22: _:_:_ parses to all AsIs
    let spec = "_:_:_".parse::<Spec>().unwrap();
    assert_eq!(spec.hours, Cycle::AsIs);
    assert_eq!(spec.minutes, Cycle::AsIs);
    assert_eq!(spec.seconds, Cycle::AsIs);
    assert_eq!(spec.to_string(), "_:_:_");

    // Mixed: _:00:00 — hours AsIs (keep), minutes At(0), seconds At(0)
    let spec = "_:00:00".parse::<Spec>().unwrap();
    assert_eq!(spec.hours, Cycle::AsIs);
    assert_eq!(spec.minutes, Cycle::At(0));
    assert_eq!(spec.seconds, Cycle::At(0));
    assert_eq!(spec.to_string(), "_:00:00");

    // AsIs in minutes position only
    let spec = "HH:_:SS".parse::<Spec>().unwrap();
    assert_eq!(spec.hours, Cycle::ForEach);
    assert_eq!(spec.minutes, Cycle::AsIs);
    assert_eq!(spec.seconds, Cycle::ForEach);
    assert_eq!(spec.to_string(), "HH:_:SS");

    // The new_with_start start passthrough always returns start itself,
    // so take(1) on any spec (including all-AsIs) produces exactly the start.
    let start = Utc.with_ymd_and_hms(2025, 1, 1, 9, 22, 45).unwrap();
    let iter = SpecIteratorBuilder::new_with_start("_:00:00", start)
        .build()
        .unwrap();
    let results: Vec<DateTime<_>> = iter.take(1).collect().unwrap();
    assert_eq!(results, vec![start]);
}

// ---------------------------------------------------------------------------
// Group 10: AsIs recurrence — _ carries vs ForEach drives
//
// Key difference from ForEach:
//   ForEach can be the finest driver (advances by 1 of its unit when no Every).
//   AsIs is never a driver — it always carries the current value.
//
// This produces observable differences when _ replaces a ForEach in the
// finest position: instead of pinning (At) or driving (ForEach), the field
// keeps whatever value the cursor already has.
// ---------------------------------------------------------------------------

#[test]
fn test_asis_recurrence() {
    let tz = Utc;
    run_cases(vec![
        // HH:MM:_ — every minute, second carries unchanged from start.
        // Compare with HH:MM:00 which pins seconds to 0.
        // Start 09:00:45 → each tick adds 1 min (MM is finest ForEach driver)
        // and keeps second at 45.
        TestCase {
            spec: "HH:MM:_",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 45).unwrap(),
            take: 6,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 1, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 2, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 3, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 4, 45).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 5, 45).unwrap(),
            ],
        },
        // HH:_:00 — every hour, minute carries unchanged from start, seconds pinned to 0.
        // Compare with HH:00:00 which pins minutes to 0.
        // Start 09:22:00 → each tick adds 1 hour (HH is finest non-At driver)
        // and keeps minute at 22.
        TestCase {
            spec: "HH:_:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 22, 0).unwrap(),
            take: 6,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 22, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 22, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 22, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 22, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 22, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 22, 0).unwrap(),
            ],
        },
        // _:30M:00 — Every(30) drives; hours AsIs carries (identical observable behaviour
        // to HH:30M:00 since both carry when an Every is present).
        TestCase {
            spec: "_:30M:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
            take: 6,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 9, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 30, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 30, 0).unwrap(),
            ],
        },
        // 1H:_:00 — Every(1) hours drives; minutes AsIs carries from start;
        // seconds pinned to 0.  Same observable result as 1H:MM:00.
        TestCase {
            spec: "1H:_:00",
            start: tz.with_ymd_and_hms(2025, 1, 1, 9, 15, 0).unwrap(),
            take: 6,
            expected: vec![
                tz.with_ymd_and_hms(2025, 1, 1, 9, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 10, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 11, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 12, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 13, 15, 0).unwrap(),
                tz.with_ymd_and_hms(2025, 1, 1, 14, 15, 0).unwrap(),
            ],
        },
    ]);
}
