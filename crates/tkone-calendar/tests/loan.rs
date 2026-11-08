//! Testing a loan's computations against its schedule.
//!
//! A ten-year loan repaid monthly, stepped through period by period. tkone
//! gives the dates: when each repayment happens, when its rule put it, and
//! the period it closes. The interest, the day count and the principal are
//! the test's own, as they would be the lender's.
//!
//! The calendar is written out in full with [`Calendar::fixed`], so the
//! expected dates do not change when the built-in holiday data does.

use chrono::{DateTime, NaiveDate, TimeZone, Weekday};
use chrono_tz::Europe::London;
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;
use tkone_calendar::Calendar;
use tkone_schedule::schedule::{Period, Schedule};

const PRINCIPAL: i64 = 12_000_000; // 120,000.00, in cents
const RATE_BP: i64 = 500; // 5% a year

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn midnight(y: i32, m: u32, d: u32) -> DateTime<Tz> {
    London.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
}

/// Weekends, Christmas, Boxing Day and New Year's Day, 2026 to 2036.
fn calendar() -> Calendar {
    let days_off = (2026..=2036).flat_map(|y| [date(y, 1, 1), date(y, 12, 25), date(y, 12, 26)]);
    Calendar::fixed("LOAN", date(2026, 1, 1)..=date(2036, 12, 31), &[Weekday::Sat, Weekday::Sun], days_off).unwrap()
}

/// Drawn on 15 January 2026; repaid on the 15th of each month for ten
/// years, moved by Modified Following when the 15th is not a business day.
fn repayments(calendar: Calendar) -> Schedule<Calendar> {
    Schedule::builder(midnight(2026, 1, 15), calendar)
        .rule("YY-MM-15~MFT00:00:00")
        .until(midnight(2036, 1, 15))
        .build()
        .unwrap()
}

fn days(from: DateTime<Tz>, to: DateTime<Tz>) -> i64 {
    // Dates, not instants: a period across a clock change is not a whole
    // number of 24 hours.
    (to.date_naive() - from.date_naive()).num_days()
}

/// Which days interest accrues over: the rule's dates (unadjusted) or the
/// dates money moves (adjusted). Contracts choose; tkone gives both.
#[derive(Clone, Copy)]
enum Accrual {
    Unadjusted,
    Adjusted,
}

fn accrual_days(period: &Period, accrual: Accrual) -> i64 {
    match accrual {
        Accrual::Unadjusted => days(period.start.actual, period.end.actual),
        Accrual::Adjusted => days(period.start.at, period.end.at),
    }
}

/// Equal principal repayments, interest Actual/365 on the balance, in
/// cents. Returns the total interest and what is left owing.
fn amortise(schedule: &Schedule<Calendar>, accrual: Accrual) -> (i64, i64) {
    let periods: Vec<Period> = schedule.periods().unwrap().collect().unwrap();
    let repayment = PRINCIPAL / periods.len() as i64;
    let (mut balance, mut interest) = (PRINCIPAL, 0);
    for period in &periods {
        interest += balance * RATE_BP * accrual_days(period, accrual) / (10_000 * 365);
        balance -= repayment;
    }
    (interest, balance)
}

#[test]
fn every_period_is_accounted_for() {
    let calendar = calendar();
    let schedule = repayments(calendar.clone());
    let periods: Vec<Period> = schedule.periods().unwrap().collect().unwrap();

    // No repayment was moved before the drawdown and dropped.
    assert!(schedule.moved_before_start().unwrap().is_empty());
    // The drawdown opens the first period; 120 repayments close them.
    assert_eq!(periods.len(), 120);
    assert_eq!(periods[0].start.at, midnight(2026, 1, 15));
    // No gaps and no overlaps: each period starts where the last one ended.
    for pair in periods.windows(2) {
        assert_eq!(pair[1].start, pair[0].end);
    }
    // So the periods' days add up to the loan's, either way they are counted.
    let term = days(midnight(2026, 1, 15), midnight(2036, 1, 15));
    for accrual in [Accrual::Unadjusted, Accrual::Adjusted] {
        assert_eq!(periods.iter().map(|p| accrual_days(p, accrual)).sum::<i64>(), term);
    }
    // Money moves only on business days, and never into another month.
    for period in &periods {
        let (paid, due) = (period.end.at.date_naive(), period.end.actual.date_naive());
        assert!(calendar.is_business_day(paid).unwrap(), "{paid}");
        assert_eq!(paid.format("%Y-%m").to_string(), due.format("%Y-%m").to_string());
    }
    // 15 March 2026 is a Sunday: repaid on Monday the 16th.
    assert_eq!((periods[1].end.actual, periods[1].end.at), (midnight(2026, 3, 15), midnight(2026, 3, 16)));

    // The loan is repaid in full.
    assert_eq!(amortise(&schedule, Accrual::Unadjusted).1, 0);
}

#[test]
fn a_drawdown_on_a_month_end_weekend() {
    // Drawn on Saturday 31 January 2026 and repaid at each month end, moved
    // to the previous business day: January's falls on Friday the 30th,
    // before the loan exists, and is dropped. The check catches it.
    let schedule = Schedule::builder(midnight(2026, 1, 31), calendar())
        .rule("YY-MM-L~PBT00:00:00")
        .until(midnight(2036, 1, 31))
        .build()
        .unwrap();
    let lost = schedule.moved_before_start().unwrap();
    assert_eq!(lost.len(), 1);
    assert_eq!((lost[0].at, lost[0].sources[0].actual), (midnight(2026, 1, 30), midnight(2026, 1, 31)));
}

#[test]
fn one_year_of_interest() {
    // The periods that end in 2030, to check that year's statement.
    let schedule = repayments(calendar());
    let year = schedule.periods_between(midnight(2030, 1, 1)..=midnight(2030, 12, 31)).unwrap();
    let periods: Vec<Period> = year.collect().unwrap();
    assert_eq!(periods.len(), 12);
    // The first starts at December 2029's repayment.
    assert_eq!(periods[0].start.actual, midnight(2029, 12, 15));
    assert_eq!(periods.iter().map(|p| accrual_days(p, Accrual::Unadjusted)).sum::<i64>(), 365);
}

#[test]
fn a_closure_at_short_notice() {
    // Wednesday 15 September 2027 is declared a day off.
    let closed = calendar().with_days_off([date(2027, 9, 15)]).unwrap();
    let before = repayments(calendar());
    let after = repayments(closed);

    let paid = |s: &Schedule<Calendar>| -> Vec<NaiveDate> { s.slots().unwrap().map(|s| Ok(s.at.date_naive())).collect().unwrap() };
    let moved: Vec<_> = paid(&before).into_iter().zip(paid(&after)).filter(|(a, b)| a != b).collect();
    assert_eq!(moved, [(date(2027, 9, 15), date(2027, 9, 16))]);

    // Unadjusted accrual does not see it; adjusted accrual moves a day of
    // interest from the next period into this one, on a higher balance.
    assert_eq!(amortise(&before, Accrual::Unadjusted), amortise(&after, Accrual::Unadjusted));
    assert!(amortise(&after, Accrual::Adjusted).0 > amortise(&before, Accrual::Adjusted).0);
}
