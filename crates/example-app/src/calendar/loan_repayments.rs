//! **Banking: a cross-border loan's repayment schedule.**
//!
//! A London company borrows US dollars for 25 years from 31 January 2026.
//! Repayments fall on the last day of each month. A payment moves only
//! within its month (Modified Following), and must fall on a day that is a
//! business day in both London, where the borrower pays, and New York, where
//! dollars settle.
//!
//! The schedule names its calendar, so the text stored with the loan fixes
//! every payment date:
//!
//! ```text
//! YY-1M-31L~MF @2026-01-31 ..+25Y [cal=GBLO+USNY]
//! ```
//!
//! Each payment keeps two dates: the scheduled one, which interest accrues
//! to, and the one cash moves on.
//!
//! ```text
//! cargo run -p example-app --bin loan-repayments
//! ```

use fallible_iterator::FallibleIterator;
use tkone_calendar::{Calendars, Status};
use tkone_schedule::date::{describe, SpecIteratorBuilder};

const SPEC: &str = "YY-1M-31L~MF @2026-01-31 ..+25Y [cal=GBLO+USNY]";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{SPEC}\n  {}\n", describe(SPEC)?);

    // The calendar the text names: London and New York together.
    let calendars = Calendars::builtin();
    let both = calendars.for_spec(SPEC)?;
    let payments: Vec<_> = SpecIteratorBuilder::from_spec(SPEC, both.clone())?.build()?.collect()?;

    println!("{} payments; the first two years:", payments.len());
    for p in payments.iter().take(24) {
        let (scheduled, paid) = (p.actual().date_naive(), p.observed().date_naive());
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

    let last = payments.last().expect("a 25-year loan has payments").observed().date_naive();
    println!("\n  last payment {last}: {:?} (holiday data is confirmed until {})", both.status(last)?, both.confirmed_until());
    debug_assert_eq!(both.status(last)?, Status::Projected);

    // Why name both calendars: on either one alone, a payment can fall on a
    // day the other is closed (31 August 2026 is a London bank holiday).
    println!("\nThe same rule on one calendar at a time, where they disagree:");
    let rule = SPEC.replace(" [cal=GBLO+USNY]", "");
    for code in ["GBLO", "USNY"] {
        let alone: Vec<_> = SpecIteratorBuilder::from_spec(&rule, calendars.get(code)?)?.build()?.take(24).collect()?;
        for (a, b) in alone.iter().zip(&payments) {
            if a.observed() != b.observed() {
                println!(
                    "  {code} alone pays {}, but {} is needed for both",
                    a.observed().format("%a %Y-%m-%d"),
                    b.observed().format("%a %Y-%m-%d")
                );
            }
        }
    }
    Ok(())
}
