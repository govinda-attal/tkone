use crate::biz_day::WeekendSkipper;
use crate::date::SpecIteratorBuilder;
use crate::prelude::*;
use crate::Occurrence;
use chrono::DateTime;
use chrono::TimeZone;
use fallible_iterator::FallibleIterator;

struct TestCase<Tz: TimeZone> {
    spec: &'static str,
    start: DateTime<Tz>,
    take: usize,
    expected: Result<Vec<Occurrence<DateTime<Tz>>>>,
}

fn run_cases<Tz: TimeZone + Clone>(cases: Vec<TestCase<Tz>>) {
    let bdp = WeekendSkipper::new();
    for tc in cases {
        let iter = SpecIteratorBuilder::new_with_start(tc.spec, bdp.clone(), tc.start).build().unwrap();
        let results: Vec<Occurrence<DateTime<_>>> = iter.take(tc.take).collect().unwrap();
        assert_eq!(tc.expected, Ok(results), "spec: {}", tc.spec);
    }
}

// ---------------------------------------------------------------------------
// Group 1: Rolling calendar-day cadences  (DD, nD)
// ---------------------------------------------------------------------------

#[test]
fn test_rolling_day_cadences() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // Every calendar day
        TestCase {
            spec: "YY-MM-DD",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 4, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 7, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap()),
            ]),
        },
        // Every 4 calendar days – rolls across month boundaries
        TestCase {
            spec: "YY-MM-4D",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 17, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 21, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 25, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 2, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 6, 0, 0, 0).unwrap()),
            ]),
        },
        // Every 7 calendar days – weekly, crosses month boundaries
        TestCase {
            spec: "YY-MM-7D",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 12, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 19, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 26, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 5, 0, 0, 0).unwrap()),
            ]),
        },
        // Every 14 calendar days – bi-weekly
        TestCase {
            spec: "YY-MM-14D",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 12, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 26, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 12, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 26, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 9, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 23, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 7, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 2: Business-day / weekday cadences  (nBD, nWD)
// ---------------------------------------------------------------------------

#[test]
fn test_biz_day_cadences() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // Every business day (WeekendSkipper)
        TestCase {
            spec: "YY-MM-1BD",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(), // Wednesday
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()), // Mon (skip Sat/Sun)
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 7, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 14, 0, 0, 0).unwrap()),
            ]),
        },
        // Every 5 business days (~weekly)
        TestCase {
            spec: "YY-MM-5BD",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 12, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 19, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 26, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 5, 0, 0, 0).unwrap()),
            ]),
        },
        // Every weekday – identical to 1BD when no custom holidays
        TestCase {
            spec: "YY-MM-1WD",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 7, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 14, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 3: Fixed day of month (monthly cadence)
// ---------------------------------------------------------------------------

#[test]
fn test_fixed_day_monthly() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // 15th of every month
        TestCase {
            spec: "YY-1M-15",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // 14th of every month (non-trivial start time preserved)
        TestCase {
            spec: "YY-1M-14",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 14, 23, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 14, 23, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 14, 23, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 14, 23, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 14, 23, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 14, 23, 0, 0).unwrap()),
            ]),
        },
        // Last day of every month
        TestCase {
            spec: "YY-1M-L",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 31, 0, 0, 0).unwrap()),
            ]),
        },
        // 31st clamped to last day of month (L suffix)
        TestCase {
            spec: "YY-1M-31L",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap()), // clamped
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap()), // clamped
            ]),
        },
        // 31st with roll-to-first-of-next-month on overflow (N suffix)
        TestCase {
            spec: "YY-1M-31N",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap(),
                ),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 5, 1, 0, 0, 0).unwrap(),
                ),
            ]),
        },
        // 31st with overflow-remainder into next month (O suffix)
        // Feb 2025 overflows by 3 days → Mar 3; 30-day months overflow by 1 → next month's 1st
        TestCase {
            spec: "YY-1M-31O",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 3, 3, 0, 0, 0).unwrap(),
                ),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 5, 1, 0, 0, 0).unwrap(),
                ),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 4: Periodic month cadences (quarterly, half-yearly, specific months)
// ---------------------------------------------------------------------------

#[test]
fn test_periodic_month_cadences() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // 15th every quarter
        TestCase {
            spec: "YY-3M-15",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 7, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 10, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 4, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // Last day of each quarter
        TestCase {
            spec: "YY-3M-L",
            take: 8,
            start: tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 4, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 7, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 10, 31, 0, 0, 0).unwrap()),
            ]),
        },
        // 1st of every half-year
        TestCase {
            spec: "YY-6M-01",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2028, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2028, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2029, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2029, 7, 1, 0, 0, 0).unwrap()),
            ]),
        },
        // Last day of each quarter in a specific year
        TestCase {
            spec: "2025-3M-L",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 12, 31, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 5: Weekday specs
// ---------------------------------------------------------------------------

#[test]
fn test_weekday_specs() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // Every Monday (start on a Wednesday – first result is start itself)
        TestCase {
            spec: "YY-MM-MON",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 20, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 27, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 10, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 17, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 24, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 3, 0, 0, 0).unwrap()),
            ]),
        },
        // Every Monday (start exactly on a Monday)
        TestCase {
            spec: "YY-MM-MON",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 20, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 27, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 10, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 17, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 24, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 10, 0, 0, 0).unwrap()),
            ]),
        },
        // Monday, Wednesday, and Friday every week
        TestCase {
            spec: "YY-MM-[MON,WED,FRI]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap(), // Monday
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 17, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 20, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 24, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 27, 0, 0, 0).unwrap()),
            ]),
        },
        // First Monday of every month
        TestCase {
            spec: "YY-1M-MON#1",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 3, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 7, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 2, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 7, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 4, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 6, 0, 0, 0).unwrap()),
            ]),
        },
        // First Wednesday of every month
        TestCase {
            spec: "YY-1M-WED#1",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 2, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 7, 0, 0, 0).unwrap()),
            ]),
        },
        // Last Friday of every month
        TestCase {
            spec: "YY-1M-FRI#L",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 28, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 25, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 27, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 25, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 26, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 31, 0, 0, 0).unwrap()),
            ]),
        },
        // Last Wednesday of every month
        TestCase {
            spec: "YY-1M-WED#L",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 26, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 26, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 28, 0, 0, 0).unwrap()),
            ]),
        },
        // 2nd-to-last Wednesday of every month
        TestCase {
            spec: "YY-1M-WED#2L",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 19, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 19, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 23, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 21, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 18, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 23, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 20, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 17, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 22, 0, 0, 0).unwrap()),
            ]),
        },
        // 2nd-to-last Wednesday every quarter (specific year)
        TestCase {
            spec: "2025-3M-WED#2L",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 23, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 23, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 22, 0, 0, 0).unwrap()),
            ]),
        },
        // Last Sunday of every December (annual)
        TestCase {
            spec: "1Y-12-SUN#L",
            take: 3,
            start: tz.with_ymd_and_hms(2025, 12, 28, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 12, 28, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 12, 27, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 12, 26, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 6: Enumerated days / months
// ---------------------------------------------------------------------------

#[test]
fn test_enumerated_days() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // 1st and 15th of every month
        TestCase {
            spec: "YY-MM-[01,15]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // 1st, 10th, 20th and 25th of every month
        TestCase {
            spec: "YY-MM-[01,10,20,25]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 20, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 25, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 10, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 20, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 25, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 10, 0, 0, 0).unwrap()),
            ]),
        },
        // 1st and 15th of Jan, Apr, Jul and Oct (quarterly, two days per quarter)
        TestCase {
            spec: "YY-[01,04,07,10]-[01,15]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 7: nth-year specs
// ---------------------------------------------------------------------------

#[test]
fn test_nth_year_specs() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // June 15th every year
        TestCase {
            spec: "1Y-06-15",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2028, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2029, 6, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // June 15th every 2 years
        TestCase {
            spec: "2Y-06-15",
            take: 8,
            start: tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2029, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2031, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2033, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2035, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2037, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2039, 6, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // Combined year+month clock: each tick = +1 year AND +3 months
        // Dec+3 wraps into Mar of the following year, adding an extra year to the year counter
        TestCase {
            spec: "1Y-3M-15",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 9, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 12, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2029, 3, 15, 0, 0, 0).unwrap()), // Dec+3 → Mar, +1 extra year
                Occurrence::Exact(tz.with_ymd_and_hms(2030, 6, 15, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 8: Finite / terminating specs
// ---------------------------------------------------------------------------

#[test]
fn test_finite_specs() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // 1st of every month in 2025 only – terminates after December
        TestCase {
            spec: "2025-MM-01",
            take: 12,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 11, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 12, 1, 0, 0, 0).unwrap()),
            ]),
        },
        // 1st and 15th of Jan and Jul across 2025 and 2026 – terminates after 8 results
        TestCase {
            spec: "[2025,2026]-[01,07]-[01,15]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 7, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // 1st and 15th of every month across 2025 and 2026 (start mid-year)
        TestCase {
            spec: "[2025,2026]-MM-[01,15]",
            take: 12,
            start: tz.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 11, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 11, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 12, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 12, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 2, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

/// Verify that `2025-MM-01` yields exactly 12 results before exhausting.
#[test]
fn test_finite_spec_terminates() {
    let tz = chrono_tz::America::New_York;
    let bdp = WeekendSkipper::new();
    let iter =
        SpecIteratorBuilder::new_with_start("2025-MM-01", bdp, tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap())
            .build()
            .unwrap();
    // collect all – should be exactly 12
    let results: Vec<_> = iter.collect().unwrap();
    assert_eq!(results.len(), 12, "2025-MM-01 should produce exactly 12 results");
}

// ---------------------------------------------------------------------------
// Group 9: Constrained-month + relative-day  (sequence restarts per period)
// ---------------------------------------------------------------------------

#[test]
fn test_constrained_month_relative_day() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // Every 4 days in January only; after Jan 29 the sequence restarts from Jan 1 of the next year
        TestCase {
            spec: "YY-01-4D",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 17, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 21, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 25, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                // After exhausting January, sequence restarts from Jan 1 of the next year
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap()),
            ]),
        },
        // Every 7 days with 1M month cadence: combined clock – day carries across months
        TestCase {
            spec: "YY-1M-7D",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 29, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 10: Values-months + relative-day regression
// ---------------------------------------------------------------------------

/// Regression tests for `NextNth year + Values months`.
///
/// The 7-day sequence restarts from day 1 of each constrained month period.
/// Year advancement resets back to the first month in the set.
#[test]
fn test_values_months_with_relative_days() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // Every year, January and June, every 7 days – sequence restarts from the 1st of each month
        TestCase {
            spec: "1Y-[01,06]-7D",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 8, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 22, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 29, 0, 0, 0).unwrap()),
            ]),
        },
        // Every year, January and June, on the 15th
        TestCase {
            spec: "1Y-[01,06]-15",
            take: 6,
            start: tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 6, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 6, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // Every 2 years, March and September, on the 1st
        TestCase {
            spec: "2Y-[03,09]-01",
            take: 6,
            start: tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 3, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2027, 9, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2029, 3, 1, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2029, 9, 1, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 11: Business-day adjustments (~NW, ~PW, ~nP, ~nN)
// ---------------------------------------------------------------------------

#[test]
fn test_biz_day_adjustments() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // 15th monthly; advance to next weekday if weekend (~NW is conditional)
        TestCase {
            spec: "YY-1M-15~NW",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()), // Wed
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 2, 15, 0, 0, 0).unwrap(), // Sat
                    tz.with_ymd_and_hms(2025, 2, 17, 0, 0, 0).unwrap(), // Mon
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 3, 15, 0, 0, 0).unwrap(), // Sat
                    tz.with_ymd_and_hms(2025, 3, 17, 0, 0, 0).unwrap(), // Mon
                ),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap()), // Tue
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 15, 0, 0, 0).unwrap()), // Thu
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(), // Sun
                    tz.with_ymd_and_hms(2025, 6, 16, 0, 0, 0).unwrap(), // Mon
                ),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()), // Tue
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 15, 0, 0, 0).unwrap()), // Fri
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 15, 0, 0, 0).unwrap()), // Mon
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()), // Wed
            ]),
        },
        // 15th monthly; move back to previous weekday if weekend (~PW is conditional)
        TestCase {
            spec: "YY-1M-15~PW",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()), // Wed
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 2, 15, 0, 0, 0).unwrap(), // Sat
                    tz.with_ymd_and_hms(2025, 2, 14, 0, 0, 0).unwrap(), // Fri
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 3, 15, 0, 0, 0).unwrap(), // Sat
                    tz.with_ymd_and_hms(2025, 3, 14, 0, 0, 0).unwrap(), // Fri
                ),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap()), // Tue
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 5, 15, 0, 0, 0).unwrap()), // Thu
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(), // Sun
                    tz.with_ymd_and_hms(2025, 6, 13, 0, 0, 0).unwrap(), // Fri
                ),
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()), // Tue
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 8, 15, 0, 0, 0).unwrap()), // Fri
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 9, 15, 0, 0, 0).unwrap()), // Mon
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()), // Wed
            ]),
        },
        // 15th monthly; unconditionally subtract 3 business days (~3P)
        // new_with_start always returns the start date as Exact  (no adjustment applied).
        TestCase {
            spec: "YY-1M-15~3P",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()), // start: no adj
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 2, 15, 0, 0, 0).unwrap(), // Sat
                    tz.with_ymd_and_hms(2025, 2, 12, 0, 0, 0).unwrap(), // Wed (−3BD from Fri 14)
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 3, 15, 0, 0, 0).unwrap(), // Sat
                    tz.with_ymd_and_hms(2025, 3, 12, 0, 0, 0).unwrap(), // Wed
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap(), // Tue
                    tz.with_ymd_and_hms(2025, 4, 10, 0, 0, 0).unwrap(), // Thu
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 5, 15, 0, 0, 0).unwrap(), // Thu
                    tz.with_ymd_and_hms(2025, 5, 12, 0, 0, 0).unwrap(), // Mon
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(), // Sun
                    tz.with_ymd_and_hms(2025, 6, 11, 0, 0, 0).unwrap(), // Wed (−3BD from Fri 13)
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap(), // Tue
                    tz.with_ymd_and_hms(2025, 7, 10, 0, 0, 0).unwrap(), // Thu
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 8, 15, 0, 0, 0).unwrap(), // Fri
                    tz.with_ymd_and_hms(2025, 8, 12, 0, 0, 0).unwrap(), // Tue
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 9, 15, 0, 0, 0).unwrap(), // Mon
                    tz.with_ymd_and_hms(2025, 9, 10, 0, 0, 0).unwrap(), // Wed
                ),
                Occurrence::AdjustedEarlier(
                    tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap(), // Wed
                    tz.with_ymd_and_hms(2025, 10, 10, 0, 0, 0).unwrap(), // Fri
                ),
            ]),
        },
        // Last day of month; unconditionally add 2 business days (~2N)
        // new_with_start returns the start date as Exact; adjustment applies from result 2 onward.
        TestCase {
            spec: "YY-1M-L~2N",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()), // start: no adj
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap(), // Fri
                    tz.with_ymd_and_hms(2025, 3, 4, 0, 0, 0).unwrap(),  // Tue (+2BD)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap(), // Mon
                    tz.with_ymd_and_hms(2025, 4, 2, 0, 0, 0).unwrap(),  // Wed (+2BD)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap(), // Wed
                    tz.with_ymd_and_hms(2025, 5, 2, 0, 0, 0).unwrap(),  // Fri (+2BD)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 5, 31, 0, 0, 0).unwrap(), // Sat
                    tz.with_ymd_and_hms(2025, 6, 3, 0, 0, 0).unwrap(),  // Tue (+2BD from Mon 2)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 6, 30, 0, 0, 0).unwrap(), // Mon
                    tz.with_ymd_and_hms(2025, 7, 2, 0, 0, 0).unwrap(),  // Wed (+2BD)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 7, 31, 0, 0, 0).unwrap(), // Thu
                    tz.with_ymd_and_hms(2025, 8, 4, 0, 0, 0).unwrap(),  // Mon (+2BD)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 8, 31, 0, 0, 0).unwrap(), // Sun
                    tz.with_ymd_and_hms(2025, 9, 2, 0, 0, 0).unwrap(),  // Tue (+2BD from Mon 1)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 9, 30, 0, 0, 0).unwrap(), // Tue
                    tz.with_ymd_and_hms(2025, 10, 2, 0, 0, 0).unwrap(), // Thu (+2BD)
                ),
                Occurrence::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 10, 31, 0, 0, 0).unwrap(), // Fri
                    tz.with_ymd_and_hms(2025, 11, 4, 0, 0, 0).unwrap(),  // Tue (+2BD)
                ),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 12: Year cadence with every month  (nY-MM-…)
// ---------------------------------------------------------------------------

/// `nY` with `MM` visits every month of each eligible year, not just the
/// anchor month.
#[test]
fn test_year_cadence_every_month() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![
        // 15th of every month, every 2 years: 2025 is skipped
        TestCase {
            spec: "2Y-MM-15",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 10, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 10, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 11, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 2, 15, 0, 0, 0).unwrap()),
            ]),
        },
        // Every day, every 2 years
        TestCase {
            spec: "2Y-MM-DD",
            take: 3,
            start: tz.with_ymd_and_hms(2024, 12, 30, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 30, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
            ]),
        },
        // Last day of every month, every year
        TestCase {
            spec: "1Y-MM-L",
            take: 3,
            start: tz.with_ymd_and_hms(2024, 1, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 1, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 3, 31, 0, 0, 0).unwrap()),
            ]),
        },
        // Every Friday, every 2 years
        TestCase {
            spec: "2Y-MM-FRI",
            take: 3,
            start: tz.with_ymd_and_hms(2024, 12, 27, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 27, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 9, 0, 0, 0).unwrap()),
            ]),
        },
        // Rolling 7-day step over every month of each eligible year; restarts
        // on Jan 1 of the next eligible year
        TestCase {
            spec: "2Y-MM-7D",
            take: 3,
            start: tz.with_ymd_and_hms(2024, 12, 24, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 24, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                Occurrence::Exact(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
            ]),
        },
    ]);
}

// ---------------------------------------------------------------------------
// Group 13: Step rules over adjacent eligible months
// ---------------------------------------------------------------------------

/// A step keeps rolling while it lands in an eligible month, and restarts at
/// the 1st only after leaving the eligible months.
#[test]
fn test_step_rolls_across_adjacent_eligible_months() {
    let tz = chrono_tz::America::New_York;
    run_cases(vec![TestCase {
        spec: "YY-[01,02]-7D",
        take: 4,
        start: tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap(),
        expected: Ok(vec![
            Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
            Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 5, 0, 0, 0).unwrap()),
            Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 12, 0, 0, 0).unwrap()),
            Occurrence::Exact(tz.with_ymd_and_hms(2025, 2, 19, 0, 0, 0).unwrap()),
        ]),
    }]);
}

// ---------------------------------------------------------------------------
// Group 14: Specs that can never match fail instead of looping
// ---------------------------------------------------------------------------

#[test]
fn test_never_matching_specs_error() {
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    for spec in ["YY-02-30", "[2025,2026]-02-29"] {
        let mut iter = SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), after).build().unwrap();
        let result = iter.next();
        assert!(matches!(result, Err(Error::IteratorNotConverged) | Ok(None)), "spec: {spec}, got {result:?}");
    }
}

// ---------------------------------------------------------------------------
// Group 15: Out-of-range values are rejected when parsing
// ---------------------------------------------------------------------------

#[test]
fn test_out_of_range_specs_rejected() {
    for spec in [
        "YY-13-01",
        "YY-00-01",
        "YY-[01,13]-01",
        "YY-0M-01",
        "0Y-MM-01",
        "YY-MM-0D",
        "YY-MM-0BD",
        "YY-MM-00",
        "YY-MM-32",
        "YY-MM-[01,32]",
        "YY-MM-WED#0",
        "YY-MM-WED#6",
        "YY-MM-WED#6L",
        // silent fallbacks that used to mean `#1` / `1`
        "YY-MM-WED#",
        "YY-MM-WED#300",
        "YY-MM-15~99999999999P",
        // a step in the wrong slot
        "YY-3Y-15",
        "2M-MM-15",
    ] {
        assert!(
            matches!(spec.parse::<crate::date::Spec>(), Err(Error::InvalidDateSpec(_))),
            "spec should be rejected: {spec}"
        );
    }
    for spec in [
        "YY-12-31",
        "YY-MM-31L",
        "YY-MM-WED#5",
        "YY-MM-FRI#5L",
        "YY-MM-FRI#L",
        "YY-1M-1D",
        "_-_-15",
        "2Y-3M-15",
        "YY-MM-15~P",
        "YY-MM-15~2N",
    ] {
        assert!(spec.parse::<crate::date::Spec>().is_ok(), "spec should parse: {spec}");
    }
}

// ---------------------------------------------------------------------------
// Group 16: Bounds
// ---------------------------------------------------------------------------

/// The end spec resolves to its first occurrence *after* start.
#[test]
fn test_with_end_spec_resolves_after_start() {
    let tz = chrono_tz::America::New_York;
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-15",
        WeekendSkipper::new(),
        tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap(),
    )
    .with_end_spec("YY-04-30")
    .build()
    .unwrap();
    let results: Vec<_> = iter.collect().unwrap();
    // End = 2025-04-30 (inclusive); the 15th of Jan–Apr fall inside it
    let observed: Vec<_> = results.iter().map(|r| r.observed().date_naive()).collect();
    assert_eq!(
        observed,
        vec![
            chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 4, 15).unwrap(),
        ]
    );
}

/// An end given in another timezone bounds by instant, not by wall clock.
#[test]
fn test_with_end_in_other_timezone() {
    let ny: chrono_tz::Tz = chrono_tz::America::New_York;
    let london: chrono_tz::Tz = chrono_tz::Europe::London;
    let start = ny.with_ymd_and_hms(2025, 1, 10, 20, 0, 0).unwrap();
    // 2025-01-13 00:30 London = 2025-01-12 19:30 New York
    let end = london.with_ymd_and_hms(2025, 1, 13, 0, 30, 0).unwrap();
    let iter =
        SpecIteratorBuilder::new_with_start("YY-MM-DD", WeekendSkipper::new(), start).with_end(end).build().unwrap();
    let results: Vec<_> = iter.collect().unwrap();
    assert!(results.iter().all(|r| *r.observed() <= end), "{results:?}");
    assert_eq!(results.len(), 2, "{results:?}"); // Jan 10, Jan 11
}

// ---------------------------------------------------------------------------
// Group 17: Weekday adjustments ignore the business-day calendar
// ---------------------------------------------------------------------------

/// A calendar where Wednesday 2025-01-15 is a holiday.
#[derive(Debug, Clone)]
struct HolidayOn15Jan;

impl crate::biz_day::BizDayProcessor for HolidayOn15Jan {
    fn is_biz_day(&self, dtm: &chrono::NaiveDateTime) -> Result<bool> {
        let holiday = chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        Ok(WeekendSkipper::new().is_biz_day(dtm)? && dtm.date() != holiday)
    }
    fn find_biz_day(
        &self,
        dtm: &chrono::NaiveDateTime,
        direction: crate::biz_day::Direction,
    ) -> Result<chrono::NaiveDateTime> {
        match direction {
            crate::biz_day::Direction::Prev => self.sub(dtm, 1),
            _ => self.add(dtm, 1),
        }
    }
    fn add(&self, dtm: &chrono::NaiveDateTime, num: u32) -> Result<chrono::NaiveDateTime> {
        let mut cur = *dtm;
        let mut n = 0;
        while n < num {
            cur += chrono::Duration::days(1);
            if self.is_biz_day(&cur)? {
                n += 1;
            }
        }
        Ok(cur)
    }
    fn sub(&self, dtm: &chrono::NaiveDateTime, num: u32) -> Result<chrono::NaiveDateTime> {
        let mut cur = *dtm;
        let mut n = 0;
        while n < num {
            cur -= chrono::Duration::days(1);
            if self.is_biz_day(&cur)? {
                n += 1;
            }
        }
        Ok(cur)
    }
}

#[test]
fn test_weekday_adjustment_ignores_holidays() {
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let holiday = tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap();
    let next =
        |spec| SpecIteratorBuilder::new_after(spec, HolidayOn15Jan, after).build().unwrap().next().unwrap().unwrap();
    // ~NW: a weekday holiday is still a weekday
    assert_eq!(next("YY-MM-15~NW"), Occurrence::Exact(holiday));
    // ~NB: the holiday is not a business day
    assert_eq!(
        next("YY-MM-15~NB"),
        Occurrence::AdjustedLater(holiday, tz.with_ymd_and_hms(2025, 1, 16, 0, 0, 0).unwrap())
    );
}

// ---------------------------------------------------------------------------
// Group 18: A skipped calendar day is reported as shifted, not duplicated
// ---------------------------------------------------------------------------

/// Pacific/Apia skipped 2011-12-30 entirely (-10:00 → +14:00).
#[test]
fn test_skipped_day_reported_as_adjusted_later() {
    let apia: chrono_tz::Tz = chrono_tz::Pacific::Apia;
    let start = apia.with_ymd_and_hms(2011, 12, 29, 0, 0, 0).unwrap();
    let results: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-DD", WeekendSkipper::new(), start)
        .build()
        .unwrap()
        .take(3)
        .collect()
        .unwrap();

    assert_eq!(results[0], Occurrence::Exact(start));
    let Occurrence::AdjustedLater(actual, observed) = &results[1] else {
        panic!("expected AdjustedLater, got {:?}", results[1]);
    };
    assert_eq!(actual.to_rfc3339(), "2011-12-30T00:00:00-10:00");
    assert_eq!(observed.to_rfc3339(), "2011-12-31T00:00:00+14:00");
    assert_eq!(actual, observed, "label and shift are the same instant");
    // Dec 31's own occurrence collides with the shift and is dropped
    assert_eq!(results[2], Occurrence::Exact(apia.with_ymd_and_hms(2012, 1, 1, 0, 0, 0).unwrap()));
}

// ---------------------------------------------------------------------------
// Group 19: Conditional adjustments keep every raw date
// ---------------------------------------------------------------------------

#[test]
fn test_conditional_adjustments_keep_every_raw_date() {
    let tz = chrono_tz::America::New_York;
    let thu = tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap();
    let day = |d| tz.with_ymd_and_hms(2025, 1, d, 0, 0, 0).unwrap();
    let take = |spec| -> Vec<_> {
        SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), thu).build().unwrap().take(4).collect().unwrap()
    };
    assert_eq!(
        take("YY-MM-DD~NB"),
        vec![
            Occurrence::Exact(day(10)),
            Occurrence::AdjustedLater(day(11), day(13)),
            Occurrence::AdjustedLater(day(12), day(13)),
            Occurrence::Exact(day(13)),
        ]
    );
    assert_eq!(
        take("YY-MM-DD~PB"),
        vec![
            Occurrence::Exact(day(10)),
            Occurrence::AdjustedEarlier(day(11), day(10)),
            Occurrence::AdjustedEarlier(day(12), day(10)),
            Occurrence::Exact(day(13)),
        ]
    );
}

// ---------------------------------------------------------------------------
// Group 20: nth business day of the month (BD#n, BD#L, BD#nL, WD#…)
// ---------------------------------------------------------------------------

#[test]
fn test_nth_biz_day_of_month() {
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap() - chrono::Duration::days(1);
    let day = |m, d| tz.with_ymd_and_hms(2025, m, d, 0, 0, 0).unwrap();
    let take = |spec: &str, bdp| -> Vec<_> {
        SpecIteratorBuilder::new_after(spec, bdp, after)
            .build()
            .unwrap()
            .take(3)
            .map(|o| Ok(*o.observed()))
            .collect()
            .unwrap()
    };
    // Feb 1 and Mar 1 2025 are Saturdays.
    assert_eq!(take("YY-MM-BD#3", WeekendSkipper::new()), vec![day(1, 3), day(2, 5), day(3, 5)]);
    assert_eq!(take("YY-MM-BD#1", WeekendSkipper::new()), vec![day(1, 1), day(2, 3), day(3, 3)]);
    assert_eq!(take("YY-MM-BD#L", WeekendSkipper::new()), vec![day(1, 31), day(2, 28), day(3, 31)]);
    assert_eq!(take("YY-MM-BD#2L", WeekendSkipper::new()), vec![day(1, 30), day(2, 27), day(3, 28)]);
    assert_eq!(
        take("YY-[03,06]-BD#1", WeekendSkipper::new()),
        vec![day(3, 3), day(6, 2), tz.with_ymd_and_hms(2026, 3, 2, 0, 0, 0).unwrap()]
    );
}

#[test]
fn test_nth_biz_day_uses_the_holiday_calendar() {
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap();
    let first = |spec: &str| -> DateTime<chrono_tz::Tz> {
        *SpecIteratorBuilder::new_after(spec, HolidayOn15Jan, after)
            .build()
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .observed()
    };
    let jan = |d| tz.with_ymd_and_hms(2025, 1, d, 0, 0, 0).unwrap();
    // Business days: 1, 2, 3, 6, 7, 8, 9, 10, 13, 14, (15 holiday), 16 …
    assert_eq!(first("YY-MM-BD#10"), jan(14));
    assert_eq!(first("YY-MM-BD#11"), jan(16));
    // WD# counts weekdays only, so the holiday still counts.
    assert_eq!(first("YY-MM-WD#11"), jan(15));
}

#[test]
fn test_nth_biz_day_parsing() {
    for s in ["YY-MM-BD#3", "YY-MM-BD#L", "YY-MM-BD#2L", "YY-MM-WD#1", "YY-MM-WD#L~1N"] {
        assert_eq!(s.parse::<crate::date::Spec>().unwrap().to_string(), s, "round trip {s}");
    }
    for s in ["YY-MM-BD#0", "YY-MM-BD#24", "YY-MM-BD#", "YY-MM-BD3", "YY-MM-XD#1"] {
        assert!(s.parse::<crate::date::Spec>().is_err(), "{s} should be rejected");
    }
    // A month with fewer business days than asked for is skipped.
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap();
    let next = SpecIteratorBuilder::new_after("YY-MM-BD#23", WeekendSkipper::new(), after)
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    // Feb 2025 has 20 weekdays, March 21; May 2025 has 22, July 23.
    assert_eq!(*next.observed(), tz.with_ymd_and_hms(2025, 7, 31, 0, 0, 0).unwrap());
}

// ---------------------------------------------------------------------------
// Group 21: Modified Following / Modified Preceding (~MF, ~MP)
// ---------------------------------------------------------------------------

#[test]
fn test_modified_following_and_preceding() {
    let tz = chrono_tz::America::New_York;
    let day = |m, d| tz.with_ymd_and_hms(2025, m, d, 0, 0, 0).unwrap();
    let first = |spec: &str, after| {
        SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), after).build().unwrap().next().unwrap().unwrap()
    };
    // Sat 2025-05-31: following is Mon Jun 2 (next month), so roll back to Fri May 30.
    assert_eq!(first("YY-MM-31L~MF", day(5, 1)), Occurrence::AdjustedEarlier(day(5, 31), day(5, 30)));
    assert_eq!(first("YY-MM-31L~NB", day(5, 1)), Occurrence::AdjustedLater(day(5, 31), day(6, 2)));
    // Sat 2025-03-15: following stays in March.
    assert_eq!(first("YY-MM-15~MF", day(3, 1)), Occurrence::AdjustedLater(day(3, 15), day(3, 17)));
    // Sat 2025-03-01: preceding is Fri Feb 28 (previous month), so roll forward to Mon Mar 3.
    assert_eq!(first("YY-MM-01~MP", day(2, 15)), Occurrence::AdjustedLater(day(3, 1), day(3, 3)));
    assert_eq!(first("YY-MM-15~MP", day(3, 1)), Occurrence::AdjustedEarlier(day(3, 15), day(3, 14)));
    // A business day is left alone.
    assert_eq!(first("YY-MM-14~MF", day(3, 1)), Occurrence::Exact(day(3, 14)));
    for s in ["YY-MM-31L~MF", "YY-MM-01~MP", "YY-MM-31L~MFT11:00:00"] {
        let date_part = s.split('T').next().unwrap();
        assert_eq!(date_part.parse::<crate::date::Spec>().unwrap().to_string(), date_part);
        assert!(s.parse::<crate::datetime::Spec>().is_ok() || !s.contains('T'));
    }
}

// ---------------------------------------------------------------------------
// Group 22: Ranges in date lists
// ---------------------------------------------------------------------------

#[test]
fn test_ranges_in_date_lists() {
    use crate::date::{Cycle, DayCycle, Spec};
    use std::collections::BTreeSet;
    let spec: Spec = "[2025-2027]-[01-03,12]-[01-05,15]".parse().unwrap();
    assert_eq!(spec.years, Cycle::Values(BTreeSet::from([2025, 2026, 2027])));
    assert_eq!(spec.months, Cycle::Values(BTreeSet::from([1, 2, 3, 12])));
    assert!(matches!(&spec.days, DayCycle::OnDays { days, .. } if *days == BTreeSet::from([1, 2, 3, 4, 5, 15])));

    let weekdays = |s: &str| match s.parse::<Spec>().unwrap().days {
        DayCycle::OnWeekDays { weekdays, .. } => weekdays.into_iter().map(|w| w.0).collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    use chrono::Weekday::*;
    assert_eq!(weekdays("YY-MM-[MON-FRI]"), vec![Mon, Tue, Wed, Thu, Fri]);
    assert_eq!(weekdays("YY-MM-[MON-TUE,THU-SAT]"), vec![Mon, Tue, Thu, Fri, Sat]);
    // A range is the same as listing its values; runs display as ranges.
    for (input, shown) in [
        ("YY-MM-[MON-FRI]", "YY-MM-[MON-FRI]"),
        ("YY-MM-[MON,TUE,WED,THU,FRI]", "YY-MM-[MON-FRI]"),
        ("YY-MM-[MON,WED,FRI]", "YY-MM-[MON,WED,FRI]"),
        ("YY-MM-[MON-WED,FRI,SAT]", "YY-MM-[MON-WED,FRI-SAT]"),
        ("YY-[01-03,12]-[01-05,15]", "YY-[01-03,12]-[01-05,15]"),
        ("YY-[03,06,09,12]-15", "YY-[03,06,09,12]-15"),
        ("[2025-2027,2030]-12-L", "[2025-2027,2030]-12-L"),
        ("YY-[06]-[15]", "YY-06-15"),
        ("YY-MM-[15,01]", "YY-MM-[01,15]"),
    ] {
        let spec: Spec = input.parse().unwrap();
        assert_eq!(spec.to_string(), shown, "display of {input}");
        let again: Spec = shown.parse().unwrap();
        assert_eq!(again.to_string(), shown, "round trip of {shown}");
    }

    for s in [
        "YY-MM-[FRI-MON]",
        "YY-[06-01]-01",
        "YY-MM-[15-01]",
        "[1-4000000000]-MM-01",
        "YY-[01-13]-01",
        "YY-MM-[00-05]",
        "YY-MM-[MON-]",
    ] {
        assert!(s.parse::<Spec>().is_err(), "{s} should be rejected");
    }

    // Every weekday in January 2025 (WeekendSkipper treats Jan 1 as a business day).
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap();
    let got: Vec<_> = SpecIteratorBuilder::new_after("YY-01-[MON-FRI]", WeekendSkipper::new(), after)
        .build()
        .unwrap()
        .take(23)
        .map(|o| Ok(o.observed().format("%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got.len(), 23);
    assert_eq!(got[..4], ["01", "02", "03", "06"]);
    assert_eq!(got[22], "31");
}

// ---------------------------------------------------------------------------
// Group 23: Week cycles (`/nW`) and positions inside cycles (`8D[1-4]`, `3M[1-2]`)
// ---------------------------------------------------------------------------

fn dates_from(spec: &str, start: (i32, u32, u32), n: usize) -> Vec<String> {
    let tz = chrono_tz::America::New_York;
    let start = tz.with_ymd_and_hms(start.0, start.1, start.2, 0, 0, 0).unwrap();
    SpecIteratorBuilder::new_with_start(spec, WeekendSkipper::new(), start)
        .build()
        .unwrap()
        .take(n)
        .map(|o| Ok(o.observed().format("%Y-%m-%d").to_string()))
        .collect()
        .unwrap()
}

#[test]
fn test_week_cycles() {
    // Every other week on Tuesday and Thursday, from Tue 7 Jan 2025.
    assert_eq!(
        dates_from("YY-MM-[TUE,THU]/2W", (2025, 1, 7), 6),
        ["2025-01-07", "2025-01-09", "2025-01-21", "2025-01-23", "2025-02-04", "2025-02-06"]
    );
    // Weeks 1 and 2 of every 4: two weeks on, two weeks off.
    let got = dates_from("YY-MM-[MON-FRI]/4W[1,2]", (2025, 1, 6), 11);
    assert_eq!(got[..2], ["2025-01-06", "2025-01-07"]);
    assert_eq!(got[9], "2025-01-17");
    assert_eq!(got[10], "2025-02-03");
    // The week cycle counts across month boundaries and from a mid-week start.
    assert_eq!(dates_from("YY-MM-FRI/2W", (2025, 1, 29), 3), ["2025-01-29", "2025-01-31", "2025-02-14"]);
    // Week 2 of every 2: the weeks not containing the start.
    assert_eq!(dates_from("YY-MM-MON/2W[2]", (2025, 1, 6), 3), ["2025-01-06", "2025-01-13", "2025-01-27"]);
    // Adjustments apply as usual.
    assert_eq!(dates_from("YY-MM-SAT/2W~NB", (2025, 1, 4), 3), ["2025-01-04", "2025-01-20", "2025-02-03"]);
}

#[test]
fn test_positions_inside_cycles() {
    // 4 on, 4 off: days 1–4 of every 8, continuing across months.
    let got = dates_from("YY-MM-8D[1-4]", (2025, 1, 1), 12);
    assert_eq!(got[..5], ["2025-01-01", "2025-01-02", "2025-01-03", "2025-01-04", "2025-01-09"]);
    assert_eq!(got[11], "2025-01-20");
    // From 29 Jan: on 29–31 Jan and 1 Feb, off 2–5 Feb, on again from 6 Feb.
    assert_eq!(dates_from("YY-MM-8D[1-4]", (2025, 1, 29), 6)[4..], ["2025-02-06", "2025-02-07"]);
    // A month filter removes dates but does not restart the cycle.
    let got = dates_from("YY-[01-02]-8D[1-4]", (2025, 1, 1), 40);
    assert!(got.iter().all(|d| d.contains("-01-") || d.contains("-02-")));
    // 2026-01-01 is day 366 of the cycle (position 6, off); the cycle resumes on 2026-01-04.
    let first_2026 = got.iter().find(|d| d.starts_with("2026")).unwrap();
    assert_eq!(first_2026, "2026-01-04");
    // Months 1 and 2 of every 3-month cycle.
    assert_eq!(
        dates_from("YY-3M[1-2]-15", (2025, 1, 15), 5),
        ["2025-01-15", "2025-02-15", "2025-04-15", "2025-05-15", "2025-07-15"]
    );
    // Years 1 and 3 of every 4.
    assert_eq!(dates_from("4Y[1,3]-06-15", (2025, 6, 15), 4), ["2025-06-15", "2027-06-15", "2029-06-15", "2031-06-15"]);
}

#[test]
fn test_cycle_parsing() {
    use crate::date::Spec;
    for (input, shown) in [
        ("YY-MM-[TUE,THU]/2W", "YY-MM-[TUE,THU]/2W"),
        ("YY-MM-[TUE,THU]/2W[1]", "YY-MM-[TUE,THU]/2W"),
        ("YY-MM-DD/4W[1,2]", "YY-MM-DD/4W[1-2]"),
        ("YY-MM-FRI/2W~NB", "YY-MM-FRI/2W~NB"),
        ("YY-MM-8D[1,2,3,4]", "YY-MM-8D[1-4]"),
        ("YY-MM-14D[1,3]", "YY-MM-14D[1,3]"),
        ("YY-3M[1-2]-15", "YY-3M[1-2]-15"),
        ("4Y[1,3]-06-15", "4Y[1,3]-06-15"),
        ("YY-MM-8D[1]", "YY-MM-8D[1]"),
    ] {
        let spec: Spec = input.parse().unwrap_or_else(|e| panic!("{input}: {e}"));
        assert_eq!(spec.to_string(), shown, "display of {input}");
    }
    for bad in [
        "YY-MM-MON/0W",
        "YY-MM-MON/2W[3]",
        "YY-MM-MON/2W[]",
        "YY-MM-8D[0]",
        "YY-MM-8D[9]",
        "YY-MM-5BD[1]",
        "YY-MM-7D/2W",
        "1Y-3M[1]-15",
        "4Y[1]-3M-15",
        "YY-3M[4]-15",
    ] {
        assert!(bad.parse::<Spec>().is_err(), "{bad} should be rejected");
    }
    // A datetime spec finds the time part after a week cycle.
    let dt: crate::datetime::Spec = "YY-MM-[TUE,THU]/2WT18:00:00".parse().unwrap();
    assert_eq!((dt.date_spec.as_str(), dt.time_spec.as_str()), ("YY-MM-[TUE,THU]/2W", "18:00:00"));
}

// ---------------------------------------------------------------------------
// Group 24: Lists of business-day positions (BD#[…]) and skipping (~SB)
// ---------------------------------------------------------------------------

#[test]
fn test_business_day_position_lists() {
    use crate::date::Spec;
    // Jan 2025: business days 1, 2, 3, 6, 7, …, 30, 31; Feb 1 is a Saturday.
    assert_eq!(
        dates_from("YY-MM-BD#[1-5]", (2025, 1, 1), 10),
        [
            "2025-01-01",
            "2025-01-02",
            "2025-01-03",
            "2025-01-06",
            "2025-01-07",
            "2025-02-03",
            "2025-02-04",
            "2025-02-05",
            "2025-02-06",
            "2025-02-07"
        ]
    );
    assert_eq!(dates_from("YY-MM-BD#[1,L]", (2025, 1, 1), 4), ["2025-01-01", "2025-01-31", "2025-02-03", "2025-02-28"]);
    assert_eq!(
        dates_from("YY-MM-BD#[L,3,2L,1]", (2025, 1, 1), 4),
        ["2025-01-01", "2025-01-03", "2025-01-30", "2025-01-31"]
    );
    // With a holiday calendar.
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap();
    let got: Vec<_> = SpecIteratorBuilder::new_after("YY-MM-BD#[10,11]", HolidayOn15Jan, after)
        .build()
        .unwrap()
        .take(2)
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(got, ["01-14", "01-16"]);
    for (input, shown) in [
        ("YY-MM-BD#[1-5]", "YY-MM-BD#[1-5]"),
        ("YY-MM-BD#[1,2,3]", "YY-MM-BD#[1-3]"),
        ("YY-MM-BD#[L,1]", "YY-MM-BD#[1,L]"),
        ("YY-MM-BD#[1,3,2L,L]", "YY-MM-BD#[1,3,2L,L]"),
        ("YY-MM-BD#[3]", "YY-MM-BD#3"),
        ("YY-MM-WD#[1,L]", "YY-MM-WD#[1,L]"),
        ("YY-MM-BD#[1,1,2]", "YY-MM-BD#[1-2]"),
    ] {
        assert_eq!(input.parse::<Spec>().unwrap().to_string(), shown, "display of {input}");
    }
    for bad in ["YY-MM-BD#[]", "YY-MM-BD#[0]", "YY-MM-BD#[24]", "YY-MM-BD#[3-1]", "YY-MM-BD#[1,]", "YY-MM-BD#[1-3L]"] {
        assert!(bad.parse::<Spec>().is_err(), "{bad} should be rejected");
    }
}

#[test]
fn test_skip_non_business_days() {
    let tz = chrono_tz::America::New_York;
    let day = |m, d| tz.with_ymd_and_hms(2025, m, d, 0, 0, 0).unwrap();
    let take = |spec: &str, after, n| -> Vec<String> {
        SpecIteratorBuilder::new_after(spec, HolidayOn15Jan, after)
            .build()
            .unwrap()
            .take(n)
            .map(|o| Ok(o.observed().format("%m-%d").to_string()))
            .collect()
            .unwrap()
    };
    // Weekends and the 15 January holiday produce nothing (not moved).
    assert_eq!(take("YY-MM-DD~SB", day(1, 12), 5), ["01-13", "01-14", "01-16", "01-17", "01-20"]);
    // The 15th, only when it is a business day: Feb 15 and Mar 15 2025 are Saturdays.
    assert_eq!(dates_from("YY-MM-15~SB", (2025, 1, 15), 3), ["2025-01-15", "2025-04-15", "2025-05-15"]);
    // Business days every other week, from Monday 6 January.
    let fortnights: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-DD/2W~SB", HolidayOn15Jan, day(1, 6))
        .build()
        .unwrap()
        .take(7)
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(fortnights, ["01-06", "01-07", "01-08", "01-09", "01-10", "01-20", "01-21"]);
    // Skipped dates are not counted.
    let counted: Vec<_> = SpecIteratorBuilder::new_after("YY-MM-15~SB", HolidayOn15Jan, day(1, 1))
        .with_count(3)
        .build()
        .unwrap()
        .take(9)
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(counted, ["04-15", "05-15", "07-15"]);
    // Display, and with a time part.
    assert_eq!("YY-MM-DD/2W~SB".parse::<crate::date::Spec>().unwrap().to_string(), "YY-MM-DD/2W~SB");
    let got: Vec<_> =
        crate::datetime::SpecIteratorBuilder::new_after("YY-MM-DD~SBT09:00:00", HolidayOn15Jan, day(1, 14))
            .build()
            .unwrap()
            .take(2)
            .map(|o| Ok(o.observed().format("%m-%d %H:%M").to_string()))
            .collect()
            .unwrap();
    assert_eq!(got, ["01-14 09:00", "01-16 09:00"]);
}

#[test]
fn test_every_business_day_two_ways() {
    // `1BD` and `DD~SB` give the same dates, including in a schedule
    // anchored at a Saturday.
    let tz = chrono_tz::America::New_York;
    let sat = tz.with_ymd_and_hms(2025, 1, 4, 0, 0, 0).unwrap();
    let take = |spec: &str| -> Vec<String> {
        SpecIteratorBuilder::new_after(spec, HolidayOn15Jan, sat)
            .build()
            .unwrap()
            .take(10)
            .map(|o| Ok(o.observed().format("%m-%d").to_string()))
            .collect()
            .unwrap()
    };
    let expected = ["01-06", "01-07", "01-08", "01-09", "01-10", "01-13", "01-14", "01-16", "01-17", "01-20"];
    for spec in ["YY-MM-1BD", "YY-MM-DD~SB"] {
        assert_eq!(take(spec), expected, "{spec}");
    }
    for rule in ["YY-MM-1BDT00:00:00", "YY-MM-DD~SBT00:00:00"] {
        let schedule = crate::schedule::Schedule::builder(sat, HolidayOn15Jan).rule(rule).build().unwrap();
        let got: Vec<_> =
            schedule.slots().unwrap().take(10).map(|s| Ok(s.at.format("%m-%d").to_string())).collect().unwrap();
        assert_eq!(got, expected, "{rule}");
    }
}

/// Like `dates_from`, but strictly after the cursor `after` (`new_after`),
/// so the cursor itself is not a result.
fn dates_after(spec: &str, after: (i32, u32, u32), n: usize) -> Vec<String> {
    let tz = chrono_tz::America::New_York;
    let after = tz.with_ymd_and_hms(after.0, after.1, after.2, 0, 0, 0).unwrap();
    SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), after)
        .build()
        .unwrap()
        .take(n)
        .map(|o| Ok(o.observed().format("%Y-%m-%d").to_string()))
        .collect()
        .unwrap()
}

#[test]
fn test_easter() {
    // Good Friday and Easter Monday, Western and Orthodox.
    assert_eq!(dates_after("YY-MM-EAS-2", (2025, 1, 1), 3), ["2025-04-18", "2026-04-03", "2027-03-26"]);
    assert_eq!(dates_after("YY-MM-EAS+1", (2025, 1, 1), 3), ["2025-04-21", "2026-04-06", "2027-03-29"]);
    assert_eq!(dates_after("YY-MM-OEAS+1", (2025, 1, 1), 3), ["2025-04-21", "2026-04-13", "2027-05-03"]);
    // Shrove Tuesday, Ascension: offsets cross into other months.
    assert_eq!(dates_after("YY-MM-EAS-47", (2025, 1, 1), 1), ["2025-03-04"]);
    assert_eq!(dates_after("YY-MM-EAS+39", (2025, 1, 1), 1), ["2025-05-29"]);
    // A start after this year's date moves to next year's.
    assert_eq!(dates_after("YY-MM-EAS-2", (2025, 5, 1), 1), ["2026-04-03"]);
    // The month rule filters: Easter Sunday only in the years it is in April.
    assert_eq!(dates_after("YY-04-EAS", (2024, 1, 1), 3), ["2025-04-20", "2026-04-05", "2028-04-16"]);
    // The ends of the offset range stay in Easter's year for Western Easter...
    assert_eq!(dates_after("YY-MM-EAS-80", (2008, 1, 1), 1), ["2008-01-03"]);
    assert_eq!(dates_after("YY-MM-EAS-80", (2284, 12, 31), 1), ["2285-01-01"]);
    assert_eq!(dates_after("YY-MM-EAS+250", (2038, 1, 1), 1), ["2038-12-31"]);
    // ...but a late Orthodox Easter can reach into the next year.
    assert_eq!(dates_after("YY-MM-OEAS+250", (2174, 1, 1), 1), ["2174-01-14"]);
    // With an end.
    let tz = chrono_tz::America::New_York;
    let bounded: Vec<_> = SpecIteratorBuilder::new_after(
        "YY-MM-EAS-2",
        WeekendSkipper::new(),
        tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
    )
    .with_end(tz.with_ymd_and_hms(2027, 12, 31, 0, 0, 0).unwrap())
    .build()
    .unwrap()
    .map(|o| Ok(o.observed().format("%Y-%m-%d").to_string()))
    .collect()
    .unwrap();
    assert_eq!(bounded, ["2025-04-18", "2026-04-03", "2027-03-26"]);
}

#[test]
fn test_easter_adjusted_and_combined() {
    let tz = chrono_tz::America::New_York;
    let day = |y, m, d| tz.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap();
    // Easter Sunday, moved to the next business day.
    let got: Vec<_> = SpecIteratorBuilder::new_after("YY-MM-EAS~NB", WeekendSkipper::new(), day(2025, 1, 1))
        .build()
        .unwrap()
        .take(1)
        .collect()
        .unwrap();
    assert_eq!(got, [Occurrence::AdjustedLater(day(2025, 4, 20), day(2025, 4, 21))]);
    // Weekdays except Good Friday and Easter Monday.
    let weekdays: Vec<_> = SpecIteratorBuilder::new_after("YY-MM-[MON-FRI]", WeekendSkipper::new(), day(2025, 4, 15))
        .with_exclusions("YY-MM-EAS-2")
        .with_exclusions("YY-MM-EAS+1")
        .build()
        .unwrap()
        .take(4)
        .map(|o| Ok(o.observed().format("%Y-%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(weekdays, ["2025-04-16", "2025-04-17", "2025-04-22", "2025-04-23"]);
    // With a time.
    let got: Vec<_> = crate::datetime::SpecIteratorBuilder::new_after(
        "YY-MM-OEAS-2T09:00:00",
        WeekendSkipper::new(),
        day(2026, 1, 1),
    )
    .build()
    .unwrap()
    .take(1)
    .map(|o| Ok(o.observed().format("%Y-%m-%d %H:%M").to_string()))
    .collect()
    .unwrap();
    assert_eq!(got, ["2026-04-10 09:00"]);
}

#[test]
fn test_easter_parsing() {
    use crate::date::{DayCycle, Easter, Spec};
    for spec in ["YY-MM-EAS", "YY-MM-EAS-2", "YY-MM-OEAS+1", "YY-MM-EAS-80~NB", "2025-[03-05]-EAS+250"] {
        assert_eq!(spec.parse::<Spec>().unwrap().to_string(), spec);
    }
    assert_eq!("YY-MM-EAS+0".parse::<Spec>().unwrap().to_string(), "YY-MM-EAS");
    assert_eq!("YY-MM-OEAS-2".parse::<Spec>().unwrap().days, DayCycle::Easter { easter: Easter::Orthodox, offset: -2 });
    for bad in [
        "YY-MM-EAS-81",
        "YY-MM-EAS+251",
        "YY-MM-EAS+99999999999",
        "YY-MM-EAS/2W",
        "YY-MM-EAS+",
        "YY-MM-EAS+-1",
        "YY-MM-EASTER",
        "YY-MM-eas",
        "YY-MM-EAS2",
    ] {
        assert!(bad.parse::<Spec>().is_err(), "{bad} should be rejected");
    }
}

// ---------------------------------------------------------------------------
// Moving to a day of the week: `~MON`, `~NMON`, `~PMON`
// ---------------------------------------------------------------------------

/// The observed dates of `spec` from 2020, one a year.
fn yearly(spec: &str, n: usize) -> Vec<String> {
    let start = chrono::Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), start)
        .build()
        .unwrap()
        .take(n)
        .map(|o| Ok(o.observed().format("%a %Y-%m-%d").to_string()))
        .collect()
        .unwrap()
}

#[test]
fn test_to_weekday() {
    // Auckland Anniversary: the Monday nearest 29 January.
    assert_eq!(
        yearly("YY-01-29~MON", 7),
        [
            "Mon 2020-01-27",
            "Mon 2021-02-01",
            "Mon 2022-01-31",
            "Mon 2023-01-30",
            "Mon 2024-01-29",
            "Mon 2025-01-27",
            "Mon 2026-01-26"
        ]
    );
    // Victoria Day: the Monday on or before 24 May.
    assert_eq!(yearly("YY-05-24~PMON", 4), ["Mon 2020-05-18", "Mon 2021-05-24", "Mon 2022-05-23", "Mon 2023-05-22"]);
    // The Monday on or after 18 May: the same days.
    assert_eq!(yearly("YY-05-18~NMON", 4), yearly("YY-05-24~PMON", 4));
    // The actual date is kept: Saturday 29 January 2022 moved to Monday 31.
    let start = chrono::Utc.with_ymd_and_hms(2022, 1, 1, 0, 0, 0).unwrap();
    let first = SpecIteratorBuilder::new_after("YY-01-29~MON", WeekendSkipper::new(), start)
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(first.actual().date_naive().to_string(), "2022-01-29");
    assert!(matches!(first, Occurrence::AdjustedLater(..)));
}

#[test]
fn test_to_weekday_parsing() {
    use crate::biz_day::Direction;
    use crate::date::{BizDayAdjustment, Spec};
    for spec in ["YY-01-29~MON", "YY-05-24~PMON", "YY-05-18~NMON", "YY-MM-DD~NWED", "YY-MM-DD~SUN", "YY-MM-DD~PSAT"] {
        assert_eq!(spec.parse::<Spec>().unwrap().to_string(), spec);
    }
    // Weekday names are read before `~NW`, `~W` and `~SB`.
    assert_eq!(
        "YY-MM-DD~NWED".parse::<Spec>().unwrap().biz_day_adj,
        Some(BizDayAdjustment::ToWeekday(chrono::Weekday::Wed, Direction::Next))
    );
    assert_eq!("YY-MM-DD~NW".parse::<Spec>().unwrap().biz_day_adj, Some(BizDayAdjustment::Weekday(Direction::Next)));
    assert_eq!("YY-MM-DD~SB".parse::<Spec>().unwrap().biz_day_adj, Some(BizDayAdjustment::SkipNonBizDay));
    assert!("YY-MM-DD~XMON".parse::<Spec>().is_err());
    assert_eq!(
        "YY-01-29~MON".parse::<Spec>().unwrap().describe(),
        "on the 29th of January, moved to the nearest Monday"
    );
}
