//! **Retail logistics: store deliveries in Johannesburg.**
//!
//! A distribution centre delivers to its stores at 06:00 every Monday,
//! Wednesday and Friday, and does not deliver on public holidays: those
//! deliveries are dropped, not moved.
//!
//! ```text
//! YY-MM-[MON,WED,FRI]~SBT06:00:00 @2026-11-02[Africa/Johannesburg] ..2026-12-31 [cal=ZAJO]
//! ```
//!
//! South Africa declares one-off holidays, such as election days, under the
//! Public Holidays Act. The calendar lists them with their proclamation, so
//! the 4 November 2026 local government election drops that Wednesday's
//! delivery.
//!
//! ```text
//! cargo run -p example-app --bin store-deliveries
//! ```

use fallible_iterator::FallibleIterator;
use tkone_calendar::Calendars;
use tkone_schedule::datetime::{describe, SpecIteratorBuilder};

const SPEC: &str = "YY-MM-[MON,WED,FRI]~SBT06:00:00 @2026-11-02[Africa/Johannesburg] ..2026-12-31 [cal=ZAJO]";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{SPEC}\n  {}\n", describe(SPEC)?);
    let johannesburg = Calendars::builtin().for_spec(SPEC)?;
    let deliveries: Vec<_> = SpecIteratorBuilder::from_spec(SPEC, johannesburg.clone())?.build()?.collect()?;
    println!("{} deliveries, the first and last:", deliveries.len());
    for d in deliveries.iter().take(4).chain(deliveries.iter().rev().take(2).rev()) {
        println!("  {}", d.observed().format("%a %Y-%m-%d %H:%M %Z"));
    }

    println!("\nNo delivery on:");
    let from = chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
    for h in johannesburg.holidays_between(from, to) {
        use chrono::Datelike;
        if matches!(h.observed.weekday(), chrono::Weekday::Mon | chrono::Weekday::Wed | chrono::Weekday::Fri) {
            println!("  {}  {}", h.observed.format("%a %Y-%m-%d"), h.name);
        }
    }
    Ok(())
}
