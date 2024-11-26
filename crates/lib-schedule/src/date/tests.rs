use crate::biz_day::WeekendSkipper;
use crate::date::SpecIteratorBuilder;
use crate::prelude::*;
use crate::NextResult;
use chrono::DateTime;
use chrono::TimeZone;
use fallible_iterator::FallibleIterator;

struct TestCase<Tz: TimeZone> {
    spec: &'static str,
    start: DateTime<Tz>,
    take: usize,
    expected: Result<Vec<NextResult<DateTime<Tz>>>>,
}

#[test]
fn test_date_iteration_for_day_valid_specs() {
    let tz = chrono_tz::America::New_York;
    let bdp = WeekendSkipper::new();
    let test_cases = vec![
        TestCase {
            spec: "YY-MM-DD",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![NextResult::Single(
                tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            )]),
        },
        TestCase {
            spec: "YY-MM-4D",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 17, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 21, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 25, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 2, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 6, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-01-4D",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 9, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 17, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 21, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 25, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-1M-7D",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 8, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 22, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 29, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-1M-14",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 14, 23, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2024, 12, 14, 23, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 14, 23, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 14, 23, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 14, 23, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 14, 23, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "1Y-06-15",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 6, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2027, 6, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2028, 6, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2029, 6, 15, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "1Y-3M-15",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 9, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2027, 12, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2029, 3, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2030, 6, 15, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "2025-3M-L",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 6, 30, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 9, 30, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 12, 31, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-1M-31L",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-1M-31N",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                NextResult::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap(),
                ),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                NextResult::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 5, 1, 0, 0, 0).unwrap(),
                ),
            ]),
        },
        TestCase {
            spec: "YY-1M-31O",
            take: 5,
            start: tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap()),
                NextResult::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 2, 28, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 3, 3, 0, 0, 0).unwrap(),
                ),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap()),
                NextResult::AdjustedLater(
                    tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap(),
                    tz.with_ymd_and_hms(2025, 5, 1, 0, 0, 0).unwrap(),
                ),
            ]),
        },
    ];

    for tc in test_cases {
        let iter = SpecIteratorBuilder::new_with_start(&tc.spec, bdp.clone(), tc.start)
            .build()
            .unwrap();
        let results: Vec<NextResult<DateTime<_>>> = iter.take(tc.take).collect().unwrap();
        assert_eq!(tc.expected, Ok(results), "Failed for spec: {}", tc.spec);
    }
}

#[test]
fn test_date_iteration_for_weekday_valid_specs() {
    let tz = chrono_tz::America::New_York;
    let bdp = WeekendSkipper::new();

    let test_cases = vec![
        TestCase {
            spec: "YY-MM-MON",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 13, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 20, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 27, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 3, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 10, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 17, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 24, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 3, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-1M-WED#1",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 5, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 5, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 2, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 5, 7, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-1M-WED#L",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 29, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 26, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 26, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 30, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 5, 28, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "2025-3M-WED#2L",
            take: 5,
            start: tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 22, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 23, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 7, 23, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 10, 22, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "1Y-12-SUN#L",
            take: 3,
            start: tz.with_ymd_and_hms(2025, 12, 28, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 12, 28, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 12, 27, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2027, 12, 26, 0, 0, 0).unwrap()),
            ]),
        },
    ];

    for tc in test_cases {
        let iter = SpecIteratorBuilder::new_with_start(&tc.spec, bdp.clone(), tc.start)
            .build()
            .unwrap();
        let results: Vec<NextResult<DateTime<_>>> = iter.take(tc.take).collect().unwrap();
        assert_eq!(tc.expected, Ok(results), "Failed for spec: {}", tc.spec);
    }
}

#[test]
fn test_date_iteration_for_multiple_days_valid_specs() {
    let tz = chrono_tz::America::New_York;
    let bdp = WeekendSkipper::new();

    let test_cases = vec![
        TestCase {
            spec: "YY-MM-[01,10,20,25]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 20, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 25, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 10, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 20, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 2, 25, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 3, 10, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "YY-[01,04,07,10]-[01,15]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 4, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "[2025,2026]-[01,07]-[01,15]",
            take: 10,
            start: tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 7, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 7, 15, 0, 0, 0).unwrap()),
            ]),
        },
        TestCase {
            spec: "[2025,2026]-MM-[01,15]",
            take: 12,
            start: tz.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap(),
            expected: Ok(vec![
                NextResult::Single(tz.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 10, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 11, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 11, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 12, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2025, 12, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 2, 15, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap()),
                NextResult::Single(tz.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap()),
            ]),
        },
    ];

    for tc in test_cases {
        let iter = SpecIteratorBuilder::new_with_start(&tc.spec, bdp.clone(), tc.start)
            .build()
            .unwrap();
        let results: Vec<NextResult<DateTime<_>>> = iter.take(tc.take).collect().unwrap();
        assert_eq!(tc.expected, Ok(results), "Failed for spec: {}", tc.spec);
    }
}
