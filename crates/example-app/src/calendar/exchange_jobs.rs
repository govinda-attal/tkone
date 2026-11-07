//! **Capital markets: end-of-day jobs on NYSE trading days.**
//!
//! A broker reconciles positions at 16:30 New York time on every day the
//! New York Stock Exchange trades, and runs its month-end report at 18:00 on
//! the last trading day of the month. One schedule document holds both
//! jobs, each a named rule:
//!
//! ```yaml
//! version: 1
//! time_zone: America/New_York
//! calendar: XNYS
//! start: 2026-11-23
//! until: 2026-12-04
//! rules:
//!   - id: reconciliation
//!     rule: YY-MM-DD~SBT16:30:00
//!   - id: month-end-report
//!     rule: YY-MM-BD#LT18:00:00
//! ```
//!
//! `~SB` drops the days the exchange is closed instead of moving the job.
//! Each run carries the ids of the rules that produced it.
//!
//! Early closes (13:00 on the day after Thanksgiving) are not holidays and
//! are not modelled; the 16:30 job still runs that day.
//!
//! ```text
//! cargo run -p example-app --bin exchange-jobs
//! ```

use fallible_iterator::FallibleIterator;
use tkone_calendar::Calendars;
use tkone_schedule::schedule::document::ScheduleDoc;

const JOBS: &str = "
version: 1
time_zone: America/New_York
calendar: XNYS
start: 2026-11-23
until: 2026-12-04
rules:
  - id: reconciliation
    rule: YY-MM-DD~SBT16:30:00
  - id: month-end-report
    rule: YY-MM-BD#LT18:00:00
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc: ScheduleDoc = serde_yaml_ng::from_str(JOBS)?;
    for rule in &doc.rules {
        println!("{:<17} {}", rule.id.as_deref().unwrap_or("-"), tkone_schedule::datetime::describe(&rule.rule)?);
    }
    println!();

    let calendars = Calendars::builtin();
    let runs: Vec<_> = doc.schedule(calendars)?.slots()?.collect()?;
    for run in &runs {
        let jobs: Vec<_> = run.sources.iter().filter_map(|s| s.rule_id.as_deref()).collect();
        println!("  {}  {}", run.at.format("%a %Y-%m-%d %H:%M %Z"), jobs.join(", "));
    }
    let nyse = calendars.get("XNYS")?;
    let from = chrono::NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
    let to = chrono::NaiveDate::from_ymd_opt(2026, 12, 4).unwrap();
    for h in nyse.holidays_between(from, to) {
        println!("  no jobs on {}: {}", h.observed.format("%a %Y-%m-%d"), h.name);
    }

    // Without a calendar, `~SB` and `BD#L` would silently mean weekdays: the
    // document is refused instead.
    let no_calendar = ScheduleDoc { calendar: None, ..doc };
    if let Err(e) = no_calendar.schedule(calendars) {
        println!("\nWithout `calendar`: {e}");
    }
    Ok(())
}
