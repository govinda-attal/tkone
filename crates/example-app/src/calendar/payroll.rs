//! **HR: fortnightly payroll for offices in Sydney and Auckland.**
//!
//! Staff are paid every other Friday. When payday is a public holiday, pay
//! goes out the business day before. Both offices use the same rule, so the
//! rule names no calendar: it is a template, run with each office's
//! calendar.
//!
//! ```text
//! YY-MM-FRI/2W~PB @2026-01-09
//! ```
//!
//! In 2026 the paydays meet Waitangi Day and Matariki in New Zealand, Good
//! Friday and Christmas in both.
//!
//! ```text
//! cargo run -p example-app --bin payroll
//! ```

use chrono::{TimeZone, Utc};
use fallible_iterator::FallibleIterator;
use tkone_calendar::Calendars;
use tkone_schedule::date::{describe, SpecIteratorBuilder};

const TEMPLATE: &str = "YY-MM-FRI/2W~PB @2026-01-09 ..2026-12-31";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{TEMPLATE}\n  {}\n", describe(TEMPLATE)?);
    let calendars = Calendars::builtin();
    let cursor = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    for (office, code) in [("Sydney", "AUSY"), ("Auckland", "NZAU")] {
        let cal = calendars.get(code)?;
        let paydays: Vec<_> = SpecIteratorBuilder::new_after(TEMPLATE, cal.clone(), cursor).build()?.collect()?;
        println!("{office} ({code}): {} paydays in 2026; moved ones:", paydays.len());
        for p in paydays.iter().filter(|p| p.actual() != p.observed()) {
            let friday = p.actual().date_naive();
            let names: Vec<_> = cal.holidays_between(friday, friday).map(|h| h.name.as_str()).collect();
            println!(
                "  Fri {} is {} → paid {}",
                friday.format("%Y-%m-%d"),
                names.join(", "),
                p.observed().format("%a %Y-%m-%d")
            );
        }
    }
    Ok(())
}
