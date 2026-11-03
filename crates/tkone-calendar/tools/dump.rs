//! Prints a calendar's holiday days off that are weekdays, one per line, to
//! compare with other sources (`tools/compare_quantlib.py` uses it). Reads
//! `data/` directly, so it needs no compile step:
//!
//! ```text
//! cargo run -p tkone-calendar --features yaml --bin tkone-calendar-dump -- GBLO 1990 2075
//! ```

use chrono::NaiveDate;
use tkone_calendar::Calendars;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [code, from, until] = args.as_slice() else {
        return Err("usage: dump CODE FROM_YEAR UNTIL_YEAR".into());
    };
    let calendars = Calendars::from_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))?;
    let calendar = calendars.get(code)?;
    let from = NaiveDate::from_ymd_opt(from.parse()?, 1, 1).ok_or("bad year")?;
    let until = NaiveDate::from_ymd_opt(until.parse()?, 12, 31).ok_or("bad year")?;
    let mut last = None;
    for h in calendar.holidays_between(from, until) {
        if !calendar.is_weekend(h.observed) && last != Some(h.observed) {
            println!("{} {}", h.observed, h.name);
            last = Some(h.observed);
        }
    }
    Ok(())
}
