//! **Loan servicing: when to collect a cross-border loan's repayments.**
//!
//! A servicing system collects repayments on a 25-year US dollar loan to a
//! London company, from January 2026. Collections run at 07:00 London time on
//! the last day of each month. A collection moves only within its month
//! (Modified Following), and must fall on a day that is a business day in
//! both London, where the borrower pays, and New York, where dollars settle.
//!
//! The schedule document stored with the loan fixes every collection:
//!
//! ```yaml
//! version: 1
//! time_zone: Europe/London
//! calendar: GBLO+USNY
//! start: 2026-01-01
//! until: 2050-12-31
//! rules:
//!   - id: collection
//!     rule: YY-1M-31L~MFT07:00:00
//! ```
//!
//! Each collection keeps two dates: the due date the statement shows, and
//! the day cash actually moves. (The loan's terms, such as its interest,
//! are the lending system's business; this schedules the collections.)
//!
//! ```text
//! cargo run -p example-app --bin loan-repayments
//! ```

use fallible_iterator::FallibleIterator;
use tkone_calendar::{Calendars, Status};
use tkone_schedule::schedule::document::ScheduleDoc;

const LOAN: &str = "
version: 1
time_zone: Europe/London
calendar: GBLO+USNY
start: 2026-01-01
until: 2050-12-31
rules:
  - id: collection
    rule: YY-1M-31L~MFT07:00:00
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc: ScheduleDoc = serde_yaml_ng::from_str(LOAN)?;
    let rule = &doc.rules[0].rule;
    println!("{rule} on GBLO+USNY\n  {}\n", tkone_schedule::datetime::describe(rule)?);

    // London and New York together.
    let calendars = Calendars::builtin();
    let both = calendars.get("GBLO+USNY")?;
    let payments: Vec<_> = doc.schedule(calendars)?.slots()?.collect()?;

    println!("{} payments; the first two years:", payments.len());
    for p in payments.iter().take(24) {
        let (scheduled, paid) = (p.sources[0].actual.date_naive(), p.at.date_naive());
        let why = if scheduled == paid {
            String::new()
        } else if both.is_weekend(scheduled) {
            format!("  ← {} is a weekend", scheduled.format("%a %-d %b"))
        } else {
            let names: Vec<_> = both.holidays_between(scheduled, scheduled).map(|h| h.name.as_str()).collect();
            format!("  ← {} is {}", scheduled.format("%a %-d %b"), names.join(", "))
        };
        println!("  scheduled {}  paid {}{why}", scheduled.format("%Y-%m-%d"), paid.format("%a %Y-%m-%d"));
    }

    let last = payments.last().expect("a 25-year loan has payments").at.date_naive();
    println!("\n  last payment {last}: {:?} (holiday data is confirmed until {})", both.status(last)?, both.confirmed_until());
    debug_assert_eq!(both.status(last)?, Status::Projected);

    // Why name both calendars: on either one alone, a payment can fall on a
    // day the other is closed (31 August 2026 is a London bank holiday).
    println!("\nThe same document on one calendar at a time, where they disagree:");
    for code in ["GBLO", "USNY"] {
        let alone = ScheduleDoc { calendar: Some(code.into()), ..doc.clone() };
        let alone: Vec<_> = alone.schedule(calendars)?.slots()?.take(24).collect()?;
        for (a, b) in alone.iter().zip(&payments) {
            if a.at != b.at {
                println!("  {code} alone pays {}, but {} is needed for both", a.at.format("%a %Y-%m-%d"), b.at.format("%a %Y-%m-%d"));
            }
        }
    }
    Ok(())
}
