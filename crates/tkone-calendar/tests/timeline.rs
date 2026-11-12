//! The examples in `docs/timeline.md`, on the built-in calendars. Their
//! 2026 dates are confirmed, so these do not change with projected data.

use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;
use chrono_tz::{America::New_York, Europe::Brussels, Europe::London};
use fallible_iterator::FallibleIterator;
use tkone_calendar::{Calendar, Calendars};
use tkone_schedule::schedule::{Moment, Schedule, Timeline};

fn calendar(code: &str) -> Calendar {
    Calendars::builtin().get(code).unwrap()
}

fn schedule(start: DateTime<Tz>, code: &str, rule: &str) -> Schedule<Calendar> {
    Schedule::builder(start, calendar(code)).rule(rule).build().unwrap()
}

/// One line per moment: the instant in `zone`, then each entry's key, with
/// its actual date when it was moved.
fn lines<K: std::fmt::Display>(moments: &[Moment<K>], zone: Tz) -> Vec<String> {
    moments
        .iter()
        .map(|m| {
            let entries: Vec<String> = m
                .entries
                .iter()
                .map(|e| {
                    let actual = e.slot.sources[0].actual;
                    if actual.date_naive() == e.slot.at.date_naive() {
                        e.key.to_string()
                    } else {
                        format!("{} (from {})", e.key, actual.format("%a %d %b"))
                    }
                })
                .collect();
            format!("{} {}", m.at.with_timezone(&zone).format("%a %d %b %H:%M %Z"), entries.join(" + "))
        })
        .collect()
}

#[test]
fn a_floating_rate_loan() {
    // A euro loan to a London company, drawn on 15 January 2026. Repaid on
    // the 15th at 12:00 London, Modified Following on London and TARGET; its
    // rate is fixed at 11:00 Brussels two TARGET days before the 15th.
    let fixings = schedule(Brussels.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(), "EUTA", "YY-MM-15~2PT11:00:00");
    // The schedule starts at the drawdown, which is not a repayment.
    let drawdown = London.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
    let repayments = Schedule::builder(drawdown, calendar("GBLO+EUTA"))
        .rule("YY-MM-15~MFT12:00:00")
        .exclude_date(drawdown.date_naive())
        .build()
        .unwrap();
    let timeline = Timeline::new().add("fixing", fixings).add("repayment", repayments);
    let from = London.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let to = London.with_ymd_and_hms(2026, 4, 30, 0, 0, 0).unwrap();
    let moments: Vec<_> = timeline.between(from..=to).unwrap().collect().unwrap();
    assert_eq!(
        lines(&moments, London),
        [
            "Tue 13 Jan 10:00 GMT fixing (from Thu 15 Jan)",
            "Thu 12 Feb 10:00 GMT fixing (from Sun 15 Feb)",
            "Mon 16 Feb 12:00 GMT repayment (from Sun 15 Feb)",
            "Thu 12 Mar 10:00 GMT fixing (from Sun 15 Mar)",
            "Mon 16 Mar 12:00 GMT repayment (from Sun 15 Mar)",
            "Mon 13 Apr 10:00 BST fixing (from Wed 15 Apr)",
            "Wed 15 Apr 12:00 BST repayment",
        ]
    );
    // Each repayment ends the period since the one before; the first, since
    // the drawdown.
    let periods: Vec<String> = moments
        .iter()
        .flat_map(|m| m.entries.iter().filter(|e| e.key == "repayment"))
        .map(|e| format!("{} -> {}", e.since.at.format("%d %b"), e.slot.at.format("%d %b")))
        .collect();
    assert_eq!(periods, ["15 Jan -> 16 Feb", "16 Feb -> 16 Mar", "16 Mar -> 15 Apr"]);
}

#[test]
fn a_savings_account() {
    // Interest accrues each London business day at 18:00, is added to the
    // balance at the month end, and a statement goes out at 09:00 on the
    // first business day.
    let open = London.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let timeline = Timeline::new()
        .add("accrue", schedule(open, "GBLO", "YY-MM-DD~SBT18:00:00"))
        .add("capitalise", schedule(open, "GBLO", "YY-MM-L~PBT18:00:00"))
        .add("statement", schedule(open, "GBLO", "YY-MM-BD#1T09:00:00"));
    let from = London.with_ymd_and_hms(2026, 5, 21, 0, 0, 0).unwrap();
    let to = London.with_ymd_and_hms(2026, 6, 2, 23, 0, 0).unwrap();
    let moments: Vec<_> = timeline.between(from..=to).unwrap().collect().unwrap();
    // Monday 25 May is the Spring bank holiday; 31 May is a Sunday.
    assert_eq!(
        lines(&moments, London),
        [
            "Thu 21 May 18:00 BST accrue",
            "Fri 22 May 18:00 BST accrue",
            "Tue 26 May 18:00 BST accrue",
            "Wed 27 May 18:00 BST accrue",
            "Thu 28 May 18:00 BST accrue",
            "Fri 29 May 18:00 BST accrue + capitalise (from Sun 31 May)",
            "Mon 01 Jun 09:00 BST statement",
            "Mon 01 Jun 18:00 BST accrue",
            "Tue 02 Jun 18:00 BST accrue",
        ]
    );
    // The day's accrual comes first: it was added first.
    let month_end = &moments[5];
    assert_eq!(month_end.entries.iter().map(|e| e.key).collect::<Vec<_>>(), ["accrue", "capitalise"]);
    // What is capitalised: the accruals since the last capitalisation (here
    // the account's opening), up to and including this moment.
    let since = month_end.entries[1].since.at;
    let all: Vec<_> = timeline.between(since..=month_end.at).unwrap().collect().unwrap();
    let accruals = all.iter().flat_map(|m| &m.entries).filter(|e| e.key == "accrue").count();
    // May 2026 has 21 weekdays; 4 and 25 May are bank holidays.
    assert_eq!((since, accruals), (open, 19));
}

#[test]
fn cross_border_payments() {
    // Instructions cut off at 16:00 London; dollars settle at 12:00 New
    // York. 31 August is a London holiday, 7 September a New York one.
    let timeline = Timeline::new()
        .add(
            "london cut-off",
            schedule(London.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap(), "GBLO", "YY-MM-DD~SBT16:00:00"),
        )
        .add(
            "new york settlement",
            schedule(New_York.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap(), "USNY", "YY-MM-DD~SBT12:00:00"),
        );
    let from = London.with_ymd_and_hms(2026, 8, 28, 0, 0, 0).unwrap();
    let to = London.with_ymd_and_hms(2026, 9, 8, 23, 0, 0).unwrap();
    let moments: Vec<_> = timeline.between(from..=to).unwrap().collect().unwrap();
    assert_eq!(
        lines(&moments, New_York),
        [
            "Fri 28 Aug 11:00 EDT london cut-off",
            "Fri 28 Aug 12:00 EDT new york settlement",
            "Mon 31 Aug 12:00 EDT new york settlement",
            "Tue 01 Sep 11:00 EDT london cut-off",
            "Tue 01 Sep 12:00 EDT new york settlement",
            "Wed 02 Sep 11:00 EDT london cut-off",
            "Wed 02 Sep 12:00 EDT new york settlement",
            "Thu 03 Sep 11:00 EDT london cut-off",
            "Thu 03 Sep 12:00 EDT new york settlement",
            "Fri 04 Sep 11:00 EDT london cut-off",
            "Fri 04 Sep 12:00 EDT new york settlement",
            "Mon 07 Sep 11:00 EDT london cut-off",
            "Tue 08 Sep 11:00 EDT london cut-off",
            "Tue 08 Sep 12:00 EDT new york settlement",
        ]
    );
    // Each settlement takes the instructions cut off since the last one:
    // Monday 31 August's has none (London was closed); Tuesday 8
    // September's has two (New York was closed on the 7th).
    let mut waiting = 0;
    let mut batches = Vec::new();
    for entry in moments.iter().flat_map(|m| &m.entries) {
        match entry.key {
            "london cut-off" => waiting += 1,
            _ => batches.push((entry.slot.at.format("%d %b").to_string(), std::mem::take(&mut waiting))),
        }
    }
    let batch = |d: &str| batches.iter().find(|(day, _)| day == d).unwrap().1;
    assert_eq!((batch("31 Aug"), batch("01 Sep"), batch("08 Sep")), (0, 1, 2));
}

#[test]
fn ordered_steps_are_rules_of_one_document() {
    // The two steps as rules of one schedule: at the month end they are one
    // slot, one run, with its sources in rule order.
    let open = London.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let account = Schedule::builder(open, calendar("GBLO"))
        .named_rule("accrue", "YY-MM-DD~SBT18:00:00")
        .named_rule("capitalise", "YY-MM-L~PBT18:00:00")
        .build()
        .unwrap();
    let month_end = London.with_ymd_and_hms(2026, 5, 29, 18, 0, 0).unwrap();
    let slot = account.slots_between(month_end..=month_end).unwrap().next().unwrap().unwrap();
    let steps: Vec<_> = slot.sources.iter().map(|s| s.rule_id.as_deref().unwrap()).collect();
    assert_eq!(steps, ["accrue", "capitalise"]);
    // 31 May 2026 is a Sunday: the capitalisation's own date.
    assert_eq!(slot.sources[1].actual.date_naive(), chrono::NaiveDate::from_ymd_opt(2026, 5, 31).unwrap());
}
