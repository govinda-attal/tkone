//! Date-only recurrence with business-day adjustment.
//!
//! Demonstrates the two-step pattern:
//! 1. Use `new_after` to find the first occurrence after now.
//! 2. Use `new_with_start` to iterate from that date onwards.
//!
//! Run with:
//!   cargo run -p lib-schedule --example date_recurrence

use chrono::{SubsecRound, Utc};
use chrono_tz::America::New_York;
use fallible_iterator::FallibleIterator;
use lib_schedule::biz_day::WeekendSkipper;
use lib_schedule::date::SpecIteratorBuilder;
fn main() {
    let bdp = WeekendSkipper::new();
    let tz = New_York;
    let now = Utc::now().with_timezone(&tz).trunc_subsecs(0);

    // Step 1: find the first occurrence strictly after now.
    let start = SpecIteratorBuilder::new_after("YY-1M-L", bdp.clone(), now)
        .build()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .observed()
        .clone();

    // Step 2: iterate from that start date (inclusive).
    let iter = SpecIteratorBuilder::new_with_start("YY-1M-L", bdp, start)
        .build()
        .unwrap();

    println!("Next 6 last-business-days-of-month (New York):");
    for r in iter.take(6).collect::<Vec<_>>().unwrap() {
        println!("  actual={}  observed={}", r.actual().to_rfc3339(), r.observed().to_rfc3339());
    }
}
