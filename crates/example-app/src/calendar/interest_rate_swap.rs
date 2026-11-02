//! **Capital markets: the schedules of a euro interest-rate swap.**
//!
//! A 5-year swap traded in London, from 1 May 2026. One side pays a fixed
//! rate once a year; the other pays 6-month EURIBOR every six months. Dates
//! roll on the 1st and move by Modified Following onto days that are
//! business days in both London and TARGET (euro payments). Each floating
//! rate is fixed two TARGET business days before its period starts.
//!
//! ```text
//! roll dates   YY-6M-01~MF @2026-05-01 ..2031-05-31 [cal=GBLO+EUTA]
//! fixed leg    YY-05-01~MF @2027-01-01 ..2031-05-31 [cal=GBLO+EUTA]
//! fixings      2 EUTA business days before each period start (Calendar::sub)
//! ```
//!
//! The roll dates fall on holidays a lot: 1 May is TARGET's Labour Day every
//! year, and 3 May 2027 is a London bank holiday as well. Each period
//! accrues between adjusted dates, so the day counts differ period to
//! period; this is what an interest calculation consumes.
//!
//! Where a spec stops: "two business days before each adjusted period
//! start" applies an offset to the result of another adjustment. A spec has
//! one adjustment, so the fixings are computed from the roll dates with the
//! calendar (`sub`). Writing `YY-6M-01~2P` instead would count back from
//! the unadjusted 1st, and in 2027 fix a day too early (29 April, not 30).
//!
//! ```text
//! cargo run -p example-app --bin interest-rate-swap
//! ```

use chrono::NaiveDate;
use fallible_iterator::FallibleIterator;
use tkone_calendar::{Calendar, Calendars};
use tkone_schedule::biz_day::BizDayProcessor;
use tkone_schedule::date::{describe, SpecIteratorBuilder};
use tkone_schedule::Occurrence;

const ROLLS: &str = "YY-6M-01~MF @2026-05-01 ..2031-05-31 [cal=GBLO+EUTA]";
const FIXED: &str = "YY-05-01~MF @2027-01-01 ..2031-05-31 [cal=GBLO+EUTA]";

fn dates(spec: &str, cal: &Calendar) -> Result<Vec<Occurrence<NaiveDate>>, Box<dyn std::error::Error>> {
    Ok(SpecIteratorBuilder::from_spec(spec, cal.clone())?
        .build()?
        .map(|o| {
            Ok(match o {
                Occurrence::Exact(d) => Occurrence::Exact(d.date_naive()),
                Occurrence::AdjustedLater(a, d) => Occurrence::AdjustedLater(a.date_naive(), d.date_naive()),
                Occurrence::AdjustedEarlier(a, d) => Occurrence::AdjustedEarlier(a.date_naive(), d.date_naive()),
            })
        })
        .collect()?)
}

/// `"Fri 1 May → Mon 4 May (Labour Day)"` for a moved date.
fn moved(o: &Occurrence<NaiveDate>, cal: &Calendar) -> String {
    let (a, d) = (*o.actual(), *o.observed());
    if a == d {
        return String::new();
    }
    let why = if cal.is_weekend(a) {
        "weekend".to_string()
    } else {
        cal.holidays_between(a, a).map(|h| h.name.clone()).collect::<Vec<_>>().join(", ")
    };
    format!("  (1st is {}: {why})", a.format("%a"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cals = Calendars::builtin();
    let both = cals.for_spec(ROLLS)?;
    let target = cals.get("EUTA")?;

    for spec in [ROLLS, FIXED] {
        println!("{spec}\n  {}", describe(spec)?);
    }

    let rolls = dates(ROLLS, &both)?;
    println!("\nFloating leg: 6-month EURIBOR, Act/360 on adjusted dates");
    println!("  {:<4} {:<14} {:<14} {:>4}  {:<14}", "", "start", "end = payment", "days", "fixing");
    for (i, pair) in rolls.windows(2).enumerate() {
        let (start, end) = (*pair[0].observed(), *pair[1].observed());
        let at = |d: NaiveDate| d.and_hms_opt(0, 0, 0).unwrap();
        let fixing = target.sub(&at(start), 2)?.date();
        println!(
            "  {:<4} {:<14} {:<14} {:>4}  {:<14}{}",
            i + 1,
            start.format("%a %Y-%m-%d").to_string(),
            end.format("%a %Y-%m-%d").to_string(),
            (end - start).num_days(),
            fixing.format("%a %Y-%m-%d").to_string(),
            moved(&pair[1], &both)
        );
    }

    println!("\nFixed leg: annual payments");
    for p in dates(FIXED, &both)? {
        println!("  {}{}", p.observed().format("%a %Y-%m-%d"), moved(&p, &both));
    }

    // The trap the comment above describes.
    let naive = SpecIteratorBuilder::from_spec("YY-05-01~2P @2027-01-01 ..2027-12-31 [cal=EUTA]", target.clone())?
        .build()?
        .next()?
        .expect("a fixing in 2027");
    println!(
        "\nYY-05-01~2P would fix the May 2027 period on {}: two TARGET days before the unadjusted 1st,\n\
         not before the adjusted start (Tue 4 May, after London's bank holiday on the 3rd).",
        naive.observed().format("%a %Y-%m-%d")
    );
    Ok(())
}
