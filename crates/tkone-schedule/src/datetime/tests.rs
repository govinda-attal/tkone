use crate::biz_day::WeekendSkipper;
use crate::datetime::SpecIteratorBuilder;
use crate::Occurrence;
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use fallible_iterator::FallibleIterator;

/// Convenience alias — reduces line noise in assertions.
type NR<Tz> = Occurrence<DateTime<Tz>>;

// ---------------------------------------------------------------------------
// Group 1: Fixed time — one tick per valid calendar date (specs 1–5)
// ---------------------------------------------------------------------------

/// Spec 1: `YY-1M-31L~NBT11:00:00` — last business day of each month at 11:00
///
/// Apr 30 (Wed) → Single.
/// May 31 (Sat) → AdjustedLater, observed = Jun 2 (Mon).
/// Jun 30 (Mon) → Single.
#[test]
fn test_last_biz_day_monthly_fixed_time() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 4, 30, 11, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-31L~NBT11:00:00",
        WeekendSkipper::new(),
        start,
    )
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.take(3).collect().unwrap();

    // Apr 30 (Wed): Single at 11:00
    assert_eq!(
        results[0],
        Occurrence::Exact(tz.with_ymd_and_hms(2025, 4, 30, 11, 0, 0).unwrap())
    );

    // May 31 (Sat) → Jun 2 (Mon): AdjustedLater, observed at 11:00 on Jun 2
    assert!(
        matches!(results[1], Occurrence::AdjustedLater(_, _)),
        "May 31 (Sat) should be AdjustedLater"
    );
    assert_eq!(
        results[1].observed().date_naive(),
        NaiveDate::from_ymd_opt(2025, 6, 2).unwrap(),
        "adjusted May 31 should be observed on Jun 2"
    );
    assert_eq!(results[1].observed().hour(), 11);
    // actual carries the pre-adjustment calendar month (May)
    assert_eq!(results[1].actual().month(), 5, "actual month should be May");

    // Jun 30 (Mon): Single at 11:00
    assert_eq!(
        results[2],
        Occurrence::Exact(tz.with_ymd_and_hms(2025, 6, 30, 11, 0, 0).unwrap())
    );
}

/// Spec 2: `YY-MM-[MON,WED,FRI]T09:30:00` — Monday, Wednesday, Friday at 09:30
#[test]
fn test_mwf_fixed_time() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 6, 9, 30, 0).unwrap(); // Monday
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-[MON,WED,FRI]T09:30:00",
        WeekendSkipper::new(),
        start,
    )
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.take(6).collect().unwrap();

    let expected = [
        tz.with_ymd_and_hms(2025, 1, 6, 9, 30, 0).unwrap(),  // Mon
        tz.with_ymd_and_hms(2025, 1, 8, 9, 30, 0).unwrap(),  // Wed
        tz.with_ymd_and_hms(2025, 1, 10, 9, 30, 0).unwrap(), // Fri
        tz.with_ymd_and_hms(2025, 1, 13, 9, 30, 0).unwrap(), // Mon
        tz.with_ymd_and_hms(2025, 1, 15, 9, 30, 0).unwrap(), // Wed
        tz.with_ymd_and_hms(2025, 1, 17, 9, 30, 0).unwrap(), // Fri
    ];
    for (i, dt) in expected.iter().enumerate() {
        assert_eq!(results[i], Occurrence::Exact(*dt), "result[{}]", i);
    }
    // All on Mon/Wed/Fri
    for r in &results {
        let wd = r.observed().date_naive().weekday();
        assert!(
            matches!(wd, Weekday::Mon | Weekday::Wed | Weekday::Fri),
            "expected Mon/Wed/Fri, got {:?}",
            wd
        );
    }
}

/// Spec 3: `YY-MM-FRIT16:30:00` — every Friday at 16:30
#[test]
fn test_weekly_friday_fixed_time() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 3, 16, 30, 0).unwrap(); // Friday
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-FRIT16:30:00",
        WeekendSkipper::new(),
        start,
    )
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.take(4).collect().unwrap();

    let expected = [
        tz.with_ymd_and_hms(2025, 1, 3, 16, 30, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 1, 10, 16, 30, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 1, 17, 16, 30, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 1, 24, 16, 30, 0).unwrap(),
    ];
    for (i, dt) in expected.iter().enumerate() {
        assert_eq!(results[i], Occurrence::Exact(*dt), "result[{}]", i);
        assert_eq!(
            results[i].observed().date_naive().weekday(),
            Weekday::Fri
        );
    }
}

/// Spec 4: `YY-3M-15T09:00:00` — 15th of each quarter at 09:00
#[test]
fn test_quarterly_15th_fixed_time() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 15, 9, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-3M-15T09:00:00",
        WeekendSkipper::new(),
        start,
    )
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.take(4).collect().unwrap();

    let expected = [
        tz.with_ymd_and_hms(2025, 1, 15, 9, 0, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 4, 15, 9, 0, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 7, 15, 9, 0, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 10, 15, 9, 0, 0).unwrap(),
    ];
    for (i, dt) in expected.iter().enumerate() {
        assert_eq!(results[i], Occurrence::Exact(*dt), "result[{}]", i);
    }
    // Adjacent results are 3 months apart
    for w in results.windows(2) {
        let a_month = w[0].observed().month();
        let b_month = w[1].observed().month();
        let diff = ((b_month as i32) - (a_month as i32) + 12) % 12;
        assert_eq!(diff, 3, "quarters should be 3 months apart");
    }
}

/// Spec 5: `YY-MM-MON#1T09:30:00` — first Monday of each month at 09:30
#[test]
fn test_first_monday_monthly_fixed_time() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 6, 9, 30, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-MON#1T09:30:00",
        WeekendSkipper::new(),
        start,
    )
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.take(4).collect().unwrap();

    let expected = [
        tz.with_ymd_and_hms(2025, 1, 6, 9, 30, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 2, 3, 9, 30, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 3, 3, 9, 30, 0).unwrap(),
        tz.with_ymd_and_hms(2025, 4, 7, 9, 30, 0).unwrap(),
    ];
    for (i, dt) in expected.iter().enumerate() {
        assert_eq!(results[i], Occurrence::Exact(*dt), "result[{}]", i);
        assert_eq!(results[i].observed().date_naive().weekday(), Weekday::Mon);
        // Each result is the first Monday of its month (day ≤ 7)
        assert!(
            results[i].observed().day() <= 7,
            "first Monday must be in days 1–7, got day {}",
            results[i].observed().day()
        );
    }
}

// ---------------------------------------------------------------------------
// Group 2: Every-N-hours — multiple ticks per date (specs 6–8)
// ---------------------------------------------------------------------------

/// Spec 6: `YY-MM-DDT6H:00:00` — every calendar day, every 6 hours
///
/// Initial date starts at the passthrough time (06:00).
/// All subsequent dates start at midnight (spec_delta = 6 h steps back to 18:00 → +6 h → 00:00).
#[test]
fn test_daily_every_6h() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 1, 6, 0, 0).unwrap();
    let end = tz.with_ymd_and_hms(2025, 1, 2, 18, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-DDT6H:00:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // Jan 1: 06:00 (passthrough), 12:00, 18:00 — 3 ticks (start was mid-cycle)
    let jan1: Vec<_> = results
        .iter()
        .filter(|r| r.observed().day() == 1)
        .collect();
    assert_eq!(jan1.len(), 3, "Jan 1 should have 3 ticks");
    assert_eq!(jan1[0].observed().hour(), 6);
    assert_eq!(jan1[1].observed().hour(), 12);
    assert_eq!(jan1[2].observed().hour(), 18);

    // Jan 2: 00:00 (midnight — spec_delta fix), 06:00, 12:00, 18:00 — 4 ticks
    let jan2: Vec<_> = results
        .iter()
        .filter(|r| r.observed().day() == 2)
        .collect();
    assert_eq!(jan2.len(), 4, "Jan 2 should have 4 ticks");
    assert_eq!(jan2[0].observed().hour(), 0, "Jan 2 first tick must be midnight");
    assert_eq!(jan2[1].observed().hour(), 6);
    assert_eq!(jan2[2].observed().hour(), 12);
    assert_eq!(jan2[3].observed().hour(), 18);
}

/// Spec 7: `YY-MM-[MON]T1H:00:00` — every Monday, hourly
///
/// Initial Monday (Jan 6) starts at 09:00 (passthrough) → 15 ticks (09:00–23:00).
/// Next Monday (Jan 13) starts at 00:00 (midnight) → 24 ticks.
#[test]
fn test_weekly_monday_every_hour() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 6, 9, 0, 0).unwrap(); // Monday
    let end = tz.with_ymd_and_hms(2025, 1, 14, 0, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-[MON]T1H:00:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // All ticks are on Mondays
    for r in &results {
        assert_eq!(
            r.observed().date_naive().weekday(),
            Weekday::Mon,
            "all ticks must be on Monday"
        );
    }

    // Jan 6: 09:00 (passthrough) through 23:00 — 15 ticks
    let jan6: Vec<_> = results.iter().filter(|r| r.observed().day() == 6).collect();
    assert_eq!(jan6.len(), 15, "Jan 6 should have 15 hourly ticks (09:00–23:00)");
    assert_eq!(jan6[0].observed().hour(), 9);
    assert_eq!(jan6[14].observed().hour(), 23);

    // Jan 13: 00:00 through 23:00 — 24 ticks, starting at midnight
    let jan13: Vec<_> = results.iter().filter(|r| r.observed().day() == 13).collect();
    assert_eq!(jan13.len(), 24, "Jan 13 should have all 24 hourly ticks");
    assert_eq!(jan13[0].observed().hour(), 0, "Jan 13 first tick must be midnight");
    assert_eq!(jan13[23].observed().hour(), 23);
}

/// Spec 8: `YY-1M-15T4H:00:00` — 15th of each month, every 4 hours
///
/// Jan 15 starts at 08:00 (passthrough) → 4 ticks (08:00, 12:00, 16:00, 20:00).
/// Feb 15 starts at 00:00 (midnight included) → 6 ticks.
#[test]
fn test_monthly_15th_every_4h() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 15, 8, 0, 0).unwrap();
    let end = tz.with_ymd_and_hms(2025, 2, 16, 0, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-15T4H:00:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // Jan 15: 08:00, 12:00, 16:00, 20:00 — 4 ticks
    let jan15: Vec<_> = results.iter().filter(|r| r.observed().month() == 1).collect();
    assert_eq!(jan15.len(), 4, "Jan 15 should have 4 ticks");
    assert_eq!(jan15[0].observed().hour(), 8);
    assert_eq!(jan15[3].observed().hour(), 20);

    // Feb 15: 00:00, 04:00, 08:00, 12:00, 16:00, 20:00 — 6 ticks (midnight included)
    let feb15: Vec<_> = results.iter().filter(|r| r.observed().month() == 2).collect();
    assert_eq!(feb15.len(), 6, "Feb 15 should have 6 ticks (midnight included)");
    assert_eq!(feb15[0].observed().hour(), 0, "Feb 15 first tick must be midnight");
    assert_eq!(feb15[5].observed().hour(), 20);
}

// ---------------------------------------------------------------------------
// Group 3: Every-N-minutes — many ticks per date (specs 9–10)
// ---------------------------------------------------------------------------

/// Spec 9: `YY-1M-31L~NBTHH:30M:00` — last business day of each month, every 30 min
///
/// Same-day consecutive ticks are exactly 30 min apart.
/// All ticks fall on weekdays.
#[test]
fn test_last_biz_day_every_30min() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 31, 8, 0, 0).unwrap();
    let end = tz.with_ymd_and_hms(2025, 3, 31, 18, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-31L~NBTHH:30M:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    assert!(!results.is_empty());

    // Same-day consecutive ticks are 30 min apart
    for w in results.windows(2) {
        let a = w[0].observed();
        let b = w[1].observed();
        if a.date_naive() == b.date_naive() {
            assert_eq!(*b - *a, Duration::minutes(30), "same-day gap must be 30 min");
        }
    }

    // All ticks are on weekdays
    for r in &results {
        let wd = r.observed().date_naive().weekday();
        assert!(
            !matches!(wd, Weekday::Sat | Weekday::Sun),
            "weekends must not appear, got {:?} on {}",
            wd,
            r.observed().date_naive()
        );
    }
}

/// Spec 10: `YY-MM-1BDT4H:00:00` — every business day, every 4 hours
///
/// Weekends (Jan 4 Sat, Jan 5 Sun) are skipped entirely.
/// Business-day boundaries start at midnight.
#[test]
fn test_every_biz_day_every_4h() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(); // Wednesday
    let end = tz.with_ymd_and_hms(2025, 1, 7, 0, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-1BDT4H:00:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // No Saturday (Jan 4) or Sunday (Jan 5)
    for r in &results {
        let wd = r.observed().date_naive().weekday();
        assert!(
            !matches!(wd, Weekday::Sat | Weekday::Sun),
            "weekends must be skipped, got {:?} on {}",
            wd,
            r.observed().date_naive()
        );
    }

    // Jan 2 (Thu): first tick must be midnight (spec_delta fix ensures 00:00 is included)
    let jan2_first = results
        .iter()
        .find(|r| r.observed().day() == 2)
        .expect("expected ticks on Jan 2");
    assert_eq!(jan2_first.observed().hour(), 0, "Jan 2 first tick must be midnight");

    // Same-day ticks are 4 h apart
    for w in results.windows(2) {
        let a = w[0].observed();
        let b = w[1].observed();
        if a.date_naive() == b.date_naive() {
            assert_eq!(*b - *a, Duration::hours(4), "same-day gap must be 4 h");
        }
    }
}

// ---------------------------------------------------------------------------
// Group 4: Overflow and business-day adjustment with time (specs 11–12)
// ---------------------------------------------------------------------------

/// Spec 11: `YY-1M-31NT11:00:00` — 31st-or-next-month at 11:00
///
/// Jan 31 (has 31 days) → Single.
/// Feb (28 days in 2025) → AdjustedLater, observed = Mar 1.
/// Mar 31 → Single.
/// Apr (30 days) → AdjustedLater, observed = May 1.
#[test]
fn test_31n_overflow_fixed_time() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 31, 11, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-31NT11:00:00",
        WeekendSkipper::new(),
        start,
    )
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.take(4).collect().unwrap();

    // Jan 31: Single
    assert_eq!(
        results[0],
        Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 31, 11, 0, 0).unwrap())
    );

    // Feb: overflows → AdjustedLater, observed = Mar 1 at 11:00
    assert!(
        matches!(results[1], Occurrence::AdjustedLater(_, _)),
        "Feb overflow should be AdjustedLater"
    );
    assert_eq!(
        results[1].observed().date_naive(),
        NaiveDate::from_ymd_opt(2025, 3, 1).unwrap()
    );
    assert_eq!(results[1].observed().hour(), 11);

    // Mar 31: Single
    assert_eq!(
        results[2],
        Occurrence::Exact(tz.with_ymd_and_hms(2025, 3, 31, 11, 0, 0).unwrap())
    );

    // Apr: overflows → AdjustedLater, observed = May 1 at 11:00
    assert!(
        matches!(results[3], Occurrence::AdjustedLater(_, _)),
        "Apr overflow should be AdjustedLater"
    );
    assert_eq!(
        results[3].observed().date_naive(),
        NaiveDate::from_ymd_opt(2025, 5, 1).unwrap()
    );
    assert_eq!(results[3].observed().hour(), 11);
}

/// Spec 12: `AdjustedLater` appears on the first intra-day tick only
///
/// When May 31 (Sat) adjusts to Jun 2 (Mon), only the 00:00 tick on Jun 2 is
/// `AdjustedLater`; all subsequent 30-minute ticks on Jun 2 are `Single`.
#[test]
fn test_adjusted_later_only_on_first_intraday_tick() {
    let tz = Utc;
    // Apr 30 → last biz day of Apr (Single); May 31 Sat → Jun 2 Mon (AdjustedLater)
    let start = tz.with_ymd_and_hms(2025, 4, 30, 11, 0, 0).unwrap();
    let end = tz.with_ymd_and_hms(2025, 6, 2, 2, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-31L~NBTHH:30M:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // Collect all Jun 2 ticks
    let jun2: Vec<_> = results
        .iter()
        .filter(|r| r.observed().date_naive() == NaiveDate::from_ymd_opt(2025, 6, 2).unwrap())
        .collect();
    assert!(!jun2.is_empty(), "expected ticks on Jun 2");

    // First tick on Jun 2 is AdjustedLater (actual = May, observed = Jun 2)
    assert!(
        matches!(jun2[0], Occurrence::AdjustedLater(_, _)),
        "first tick on Jun 2 must be AdjustedLater"
    );
    assert_eq!(jun2[0].actual().month(), 5, "actual month should be May");
    assert_eq!(jun2[0].observed().hour(), 0);

    // All subsequent ticks on Jun 2 are Single
    for tick in jun2.iter().skip(1) {
        assert!(
            matches!(tick, Occurrence::Exact(_)),
            "subsequent ticks on Jun 2 must be Single, got {:?} at {:?}",
            tick,
            tick.observed().time()
        );
    }

    // Consecutive Jun 2 ticks are 30 min apart
    for w in jun2.windows(2) {
        assert_eq!(
            *w[1].observed() - *w[0].observed(),
            Duration::minutes(30),
            "Jun 2 intra-day gap must be 30 min"
        );
    }
}

// ---------------------------------------------------------------------------
// Group 5: `new_after` semantics (specs 13–14)
// ---------------------------------------------------------------------------

/// Spec 13: `YY-1M-15T11:00:00` via `new_after` from 12:00 on the 15th
///
/// The 11:00 slot for Jan 15 has already passed; Jan 15 is skipped entirely.
/// First result is Feb 15 at 11:00.
#[test]
fn test_new_after_skips_entire_initial_date() {
    let tz = Utc;
    let dtm = tz.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap();
    let iter =
        SpecIteratorBuilder::new_after("YY-1M-15T11:00:00", WeekendSkipper::new(), dtm)
            .build()
            .unwrap();
    let results: Vec<NR<_>> = iter.take(3).collect().unwrap();

    // Jan 15 is skipped; first result is Feb 15
    assert_eq!(
        results[0].observed().date_naive(),
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
        "Jan 15 should be skipped when dtm is past 11:00"
    );
    assert_eq!(results[0].observed().hour(), 11);
    assert_eq!(
        results[1].observed().date_naive(),
        NaiveDate::from_ymd_opt(2025, 3, 15).unwrap()
    );
}

/// Spec 14: `YY-MM-DDT1H:00:00` via `new_after` from a non-aligned cursor
///
/// `At` transforms run before `Every`: sec→0, min→0, hours+1.
/// From 09:30 → 09:00 → 10:00. First result is 10:00, not 09:30 or 10:30.
#[test]
fn test_new_after_non_aligned_cursor() {
    let tz = Utc;
    let dtm = tz.with_ymd_and_hms(2025, 1, 15, 9, 30, 0).unwrap();
    let iter =
        SpecIteratorBuilder::new_after("YY-MM-DDT1H:00:00", WeekendSkipper::new(), dtm)
            .build()
            .unwrap();
    let results: Vec<NR<_>> = iter.take(3).collect().unwrap();

    assert_eq!(
        results[0],
        Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 10, 0, 0).unwrap()),
        "first result after 09:30 with 1H:00:00 should be 10:00"
    );
    assert_eq!(
        results[1],
        Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 11, 0, 0).unwrap())
    );
    assert_eq!(
        results[2],
        Occurrence::Exact(tz.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap())
    );
}

/// `new_after` from before the daily fixed time: first result is same day
#[test]
fn test_new_after_same_day_future_time() {
    let tz = Utc;
    let dtm = tz.with_ymd_and_hms(2025, 1, 15, 9, 0, 0).unwrap();
    let iter =
        SpecIteratorBuilder::new_after("YY-MM-DDT11:00:00", WeekendSkipper::new(), dtm)
            .build()
            .unwrap();
    let results: Vec<NR<_>> = iter.take(2).collect().unwrap();

    assert_eq!(results[0].observed().day(), 15);
    assert_eq!(results[0].observed().hour(), 11);
}

/// `new_after` from after the daily fixed time: first result is the next day
#[test]
fn test_new_after_past_time_skips_to_next_day() {
    let tz = Utc;
    let dtm = tz.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap();
    let iter =
        SpecIteratorBuilder::new_after("YY-MM-DDT11:00:00", WeekendSkipper::new(), dtm)
            .build()
            .unwrap();
    let results: Vec<NR<_>> = iter.take(2).collect().unwrap();

    assert_eq!(results[0].observed().day(), 16, "past daily time should skip to next day");
    assert_eq!(results[0].observed().hour(), 11);
}

// ---------------------------------------------------------------------------
// Group 6: `AsIs` (`_`) and carry edge cases (specs 15–16)
// ---------------------------------------------------------------------------

/// Spec 15: `YY-1M-31LT_:_:_` — AsIs time on a monthly date spec
///
/// The start passthrough preserves 14:30:00. On all subsequent dates the
/// synthetic cursor (midnight − 1 s) has time 23:59:59 which applied through
/// `_:_:_` lands before midnight, triggering the midnight-fallback.
/// The midnight fallback applies `_:_:_` to 00:00:00 → **00:00:00**.
/// The original 14:30 is NOT carried forward.
#[test]
fn test_asis_time_carry_loss_on_date_gaps() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 31, 14, 30, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-31LT_:_:_",
        WeekendSkipper::new(),
        start,
    )
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.take(3).collect().unwrap();

    // Passthrough: start preserved unchanged
    assert_eq!(results[0], Occurrence::Exact(start));
    assert_eq!(results[0].observed().hour(), 14);
    assert_eq!(results[0].observed().minute(), 30);

    // Feb 28: time resets to 00:00:00 (carry lost — not 14:30)
    assert_eq!(
        results[1].observed().date_naive(),
        NaiveDate::from_ymd_opt(2025, 2, 28).unwrap()
    );
    assert_eq!(
        results[1].observed().hour(),
        0,
        "carry lost: time should reset to midnight, not 14:30"
    );
    assert_eq!(results[1].observed().minute(), 0);

    // Mar 31: same midnight-reset pattern
    assert_eq!(
        results[2].observed().date_naive(),
        NaiveDate::from_ymd_opt(2025, 3, 31).unwrap()
    );
    assert_eq!(results[2].observed().hour(), 0);
}

/// Spec 16: `YY-1M-15T1H:MM:SS` — hourly with minute/second carry, lost on date gaps
///
/// On the initial date the carry (:22:45 from start) is preserved on every tick.
/// On Feb 15 the synthetic cursor is `00:00:00 − 1h = 23:00:00` (min=0, sec=0),
/// so the carry resets to :00:00 on the new date.
#[test]
fn test_hourly_carry_loss_on_date_gaps() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 15, 9, 22, 45).unwrap();
    let end = tz.with_ymd_and_hms(2025, 2, 15, 2, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-15T1H:MM:SS",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // Jan 15: all ticks carry :22:45 (passthrough seeds the cursor)
    let jan15: Vec<_> = results.iter().filter(|r| r.observed().month() == 1).collect();
    assert!(jan15.len() >= 2, "expected multiple Jan 15 ticks");
    for r in &jan15 {
        assert_eq!(r.observed().minute(), 22, "Jan 15: minute carry should be 22");
        assert_eq!(r.observed().second(), 45, "Jan 15: second carry should be 45");
    }

    // Feb 15: cursor synthesised from midnight − 1h = 23:00:00 (min=0, sec=0)
    // → carry resets to :00:00
    let feb15: Vec<_> = results.iter().filter(|r| r.observed().month() == 2).collect();
    assert!(!feb15.is_empty(), "expected Feb 15 ticks");
    assert_eq!(feb15[0].observed().hour(), 0, "Feb 15 first tick: hour should be 0");
    assert_eq!(
        feb15[0].observed().minute(),
        0,
        "Feb 15: minute carry lost, should be 0 (was 22)"
    );
    assert_eq!(
        feb15[0].observed().second(),
        0,
        "Feb 15: second carry lost, should be 0 (was 45)"
    );
}

// ---------------------------------------------------------------------------
// Group 7: Regression — spec_delta midnight fix
// ---------------------------------------------------------------------------

/// `YY-MM-DDT1H:00:00`: Jan 2 must start at 00:00, not 01:00 (pre-fix bug).
#[test]
fn test_every_hour_includes_midnight_on_new_days() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
    let end = tz.with_ymd_and_hms(2025, 1, 3, 0, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-DDT1H:00:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    let jan1: Vec<_> = results.iter().filter(|r| r.observed().day() == 1).collect();
    let jan2: Vec<_> = results.iter().filter(|r| r.observed().day() == 2).collect();

    // Jan 1: passthrough at 09:00 → last tick at 23:00 = 15 ticks
    assert_eq!(jan1.len(), 15, "Jan 1: 15 hourly ticks from 09:00");
    assert_eq!(jan1[0].observed().hour(), 9);

    // Jan 2: all 24 hourly ticks, starting at midnight
    assert_eq!(jan2.len(), 24, "Jan 2 should have all 24 hourly ticks");
    assert_eq!(jan2[0].observed().hour(), 0, "Jan 2 first tick must be midnight");
}

/// `YY-MM-DDTHH:30M:00`: day-boundary tick at 00:00 must not be skipped.
#[test]
fn test_every_30min_includes_midnight_on_new_days() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 1, 1, 23, 0, 0).unwrap();
    // End at 00:31 so the 01:00 tick is excluded, giving exactly 4 results.
    let end = tz.with_ymd_and_hms(2025, 1, 2, 0, 31, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-MM-DDTHH:30M:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // 23:00 (passthrough), 23:30, 00:00 (midnight — spec_delta fix), 00:30
    assert_eq!(results.len(), 4);
    assert_eq!(results[0].observed().hour(), 23);
    assert_eq!(results[0].observed().minute(), 0);
    assert_eq!(results[1].observed().hour(), 23);
    assert_eq!(results[1].observed().minute(), 30);
    assert_eq!(results[2].observed().hour(), 0, "midnight must be included");
    assert_eq!(results[2].observed().minute(), 0);
    assert_eq!(results[3].observed().hour(), 0);
    assert_eq!(results[3].observed().minute(), 30);
}

// ---------------------------------------------------------------------------
// Group 8: Regression — business-day adjustment across month boundary
// ---------------------------------------------------------------------------

/// `AdjustedLater(May 31→Jun 2)` must not cause June's natural occurrence to be
/// skipped.  The occurrence after Jun 2 should be Jun 30, not Jul 31.
#[test]
fn test_biz_day_adjustment_does_not_skip_next_month() {
    let tz = Utc;
    let start = tz.with_ymd_and_hms(2025, 5, 1, 11, 0, 0).unwrap();
    let end = tz.with_ymd_and_hms(2025, 8, 1, 11, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-31L~NBT11:00:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    // Find Jun 2 (the AdjustedLater for May 31)
    let jun2_idx = results
        .iter()
        .position(|r| r.observed().date_naive() == NaiveDate::from_ymd_opt(2025, 6, 2).unwrap())
        .expect("expected an occurrence on Jun 2 (adjusted May 31)");

    // The occurrence after Jun 2 should be in June (Jun 30), not July
    let after_jun2 = &results[jun2_idx + 1];
    assert_eq!(
        after_jun2.observed().month(),
        6,
        "occurrence after AdjustedLater(May 31→Jun 2) should be Jun 30, not Jul"
    );
}

// ---------------------------------------------------------------------------
// Group 9: Timezone-aware
// ---------------------------------------------------------------------------

/// All ticks at 11:00 local NY time across DST transitions.
#[test]
fn test_ny_timezone_last_biz_day_11am() {
    use chrono_tz::America::New_York;
    let ny = New_York;
    let start = ny.with_ymd_and_hms(2024, 11, 29, 11, 0, 0).unwrap();
    let end = ny.with_ymd_and_hms(2025, 4, 30, 11, 0, 0).unwrap();
    let iter = SpecIteratorBuilder::new_with_start(
        "YY-1M-31L~NBT11:00:00",
        WeekendSkipper::new(),
        start,
    )
    .with_end(end)
    .build()
    .unwrap();
    let results: Vec<NR<_>> = iter.collect().unwrap();

    assert!(!results.is_empty());
    for r in &results {
        assert_eq!(r.observed().hour(), 11, "all ticks should be at 11:00 NY time");
    }
}
