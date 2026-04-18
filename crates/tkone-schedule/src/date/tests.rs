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
        let iter = SpecIteratorBuilder::new_with_start(tc.spec, bdp.clone(), tc.start)
            .build()
            .unwrap();
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
    let iter = SpecIteratorBuilder::new_with_start(
        "2025-MM-01",
        bdp,
        tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
    )
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
        // new_with_start always returns the start date as Single (no adjustment applied).
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
        // new_with_start returns the start date as Single; adjustment applies from result 2 onward.
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

