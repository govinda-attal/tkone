//! **HR: fortnightly payroll for offices in Sydney and Auckland.**
//!
//! Staff are paid at 09:00 every other Friday. When payday is a public
//! holiday, pay goes out the business day before. Both offices use the same
//! document as a template; each office fills in its own time zone and
//! calendar:
//!
//! ```yaml
//! version: 1
//! start: 2026-01-09
//! until: 2026-12-31
//! rules:
//!   - id: payday
//!     rule: YY-MM-FRI/2W~PBT09:00:00
//! ```
//!
//! The start anchors the fortnights: Friday 9 January is week 1.
//!
//! In 2026 the paydays meet Waitangi Day and Matariki in New Zealand, Good
//! Friday and Christmas in both.
//!
//! ```text
//! cargo run -p example-app --bin payroll
//! ```

use fallible_iterator::FallibleIterator;
use tkone_calendar::Calendars;
use tkone_schedule::schedule::document::ScheduleDoc;

const TEMPLATE: &str = "
version: 1
start: 2026-01-09
until: 2026-12-31
rules:
  - id: payday
    rule: YY-MM-FRI/2W~PBT09:00:00
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let template: ScheduleDoc = serde_yaml_ng::from_str(TEMPLATE)?;
    let rule = &template.rules[0].rule;
    println!("{rule}\n  {}\n", tkone_schedule::datetime::describe(rule)?);
    let calendars = Calendars::builtin();
    for (office, zone, code) in [("Sydney", "Australia/Sydney", "AUSY"), ("Auckland", "Pacific/Auckland", "NZAU")] {
        let doc = ScheduleDoc { time_zone: Some(zone.into()), calendar: Some(code.into()), ..template.clone() };
        let paydays: Vec<_> = doc.schedule(calendars)?.slots()?.collect()?;
        let cal = calendars.get(code)?;
        println!("{office} ({code}): {} paydays in 2026; moved ones:", paydays.len());
        for p in paydays.iter().filter(|p| p.sources[0].actual != p.at) {
            let friday = p.sources[0].actual.date_naive();
            let names: Vec<_> = cal.holidays_between(friday, friday).map(|h| h.name.as_str()).collect();
            println!("  Fri {} is {} → paid {}", friday.format("%Y-%m-%d"), names.join(", "), p.at.format("%a %Y-%m-%d"));
        }
    }
    Ok(())
}
