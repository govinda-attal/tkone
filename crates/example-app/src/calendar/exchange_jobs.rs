//! **Capital markets: end-of-day jobs on NYSE trading days.**
//!
//! A broker reconciles positions at 16:30 New York time on every day the
//! New York Stock Exchange trades, and runs its month-end report at 18:00 on
//! the last trading day of the month:
//!
//! ```text
//! YY-MM-DD~SBT16:30:00 | YY-MM-BD#LT18:00:00 @2026-11-23[America/New_York] ..2026-12-04 [cal=XNYS]
//! ```
//!
//! `~SB` drops the days the exchange is closed instead of moving the job.
//! The calendar is in the text, so a job runner given the wrong calendar
//! refuses to start.
//!
//! Early closes (13:00 on the day after Thanksgiving) are not holidays and
//! are not modelled; the 16:30 job still runs that day.
//!
//! ```text
//! cargo run -p example-app --bin exchange-jobs
//! ```

use fallible_iterator::FallibleIterator;
use tkone_calendar::Calendars;
use tkone_schedule::datetime::{describe, SpecIteratorBuilder};

const SPEC: &str =
    "YY-MM-DD~SBT16:30:00 | YY-MM-BD#LT18:00:00 @2026-11-23[America/New_York] ..2026-12-04 [cal=XNYS]";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{SPEC}\n  {}\n", describe(SPEC)?);

    let calendars = Calendars::builtin();
    let nyse = calendars.for_spec(SPEC)?;
    let runs: Vec<_> = SpecIteratorBuilder::from_spec(SPEC, nyse.clone())?.build()?.collect()?;
    for run in &runs {
        let job = if run.observed().format("%H:%M").to_string() == "18:00" { "month-end report" } else { "reconciliation" };
        println!("  {}  {job}", run.observed().format("%a %Y-%m-%d %H:%M %Z"));
    }
    let from = chrono::NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2026, 12, 4).unwrap();
    for h in nyse.holidays_between(from, to) {
        println!("  no jobs on {}: {}", h.observed.format("%a %Y-%m-%d"), h.name);
    }

    // The same text with the London calendar does not build.
    if let Err(e) = SpecIteratorBuilder::from_spec(SPEC, calendars.get("GBLO")?)?.build() {
        println!("\nWith the London calendar instead: {e}");
    }
    Ok(())
}
