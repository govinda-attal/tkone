//! Intra-day time recurrence examples.
//!
//! Shows two common patterns:
//! - Every 30 minutes from a fixed start.
//! - Every hour until a fixed end time.
//!
//! Run with:
//!   cargo run -p lib-schedule --example time_recurrence

use chrono::{TimeZone, Utc};
use fallible_iterator::FallibleIterator;
use lib_schedule::time::SpecIteratorBuilder;

fn main() {
    let start = Utc.with_ymd_and_hms(2024, 6, 1, 9, 0, 0).unwrap();

    // Every 30 minutes starting at 09:00.
    let times = SpecIteratorBuilder::new_with_start("HH:30M:00", start)
        .build()
        .unwrap()
        .take(4)
        .collect::<Vec<_>>()
        .unwrap();

    println!("Every 30 minutes (4 occurrences from 09:00):");
    for t in &times {
        println!("  {}", t);
    }

    // Every hour from 09:00 until 13:00 (inclusive).
    let end = Utc.with_ymd_and_hms(2024, 6, 1, 13, 0, 0).unwrap();
    let times = SpecIteratorBuilder::new_with_start("1H:00:00", start)
        .with_end(end)
        .build()
        .unwrap()
        .collect::<Vec<_>>()
        .unwrap();

    println!("\nEvery hour from 09:00 to 13:00:");
    for t in &times {
        println!("  {}", t);
    }
}
