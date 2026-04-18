//! Combined date + time recurrence with business-day adjustment.
//!
//! Schedules the last business day of every month at 11:00 London time,
//! using the `~W` (next weekday) adjustment.
//!
//! Run with:
//!   cargo run -p tkone-schedule --example datetime_recurrence

use chrono::{SubsecRound, Utc};
use chrono_tz::Europe::London;
use fallible_iterator::FallibleIterator;
use tkone_schedule::biz_day::WeekendSkipper;
use tkone_schedule::datetime::SpecIteratorBuilder;
fn main() {
    let bdp = WeekendSkipper::new();
    let now = Utc::now().with_timezone(&London).trunc_subsecs(0);

    // Step 1: find the first occurrence strictly after now.
    let start = SpecIteratorBuilder::new_after("YY-1M-L~WT11:00:00", bdp.clone(), now)
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .observed()
        .clone();

    // Step 2: iterate from that start datetime (inclusive).
    let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~WT11:00:00", bdp, start)
        .build()
        .unwrap();

    println!("Next 6 last-business-day-of-month at 11:00 London:");
    for r in iter.take(6).collect::<Vec<_>>().unwrap() {
        println!("  actual={}  observed={}", r.actual().to_rfc3339(), r.observed().to_rfc3339());
    }
}
