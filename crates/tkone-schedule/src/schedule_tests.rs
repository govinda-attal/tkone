//! Schedules: merged rules, slots unique by instant, the start as an anchor,
//! `until`, counts, exclusions, and `slots_after` agreeing with `slots`.

use std::collections::BTreeSet;

use chrono::{DateTime, Datelike, Days, Duration, NaiveDate, NaiveDateTime, TimeZone, Weekday};
use chrono_tz::Europe::London;
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use crate::biz_day::{BizDayProcessor, Direction, WeekendSkipper};
use crate::schedule::{ExcludeMatch, Next, Schedule, Selector, Slot};
use crate::{Error, Result};

/// Weekends and the listed dates are not business days.
#[derive(Debug, Clone)]
struct Holidays(BTreeSet<NaiveDate>);

impl Holidays {
    /// Christmas 2025 to New Year 2026 in London.
    fn london() -> Self {
        Holidays([d(2025, 12, 25), d(2025, 12, 26), d(2026, 1, 1)].into())
    }

    fn open(&self, date: NaiveDate) -> bool {
        !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) && !self.0.contains(&date)
    }

    fn step(&self, dtm: &NaiveDateTime, n: u32, forward: bool) -> NaiveDateTime {
        let mut date = dtm.date();
        let mut left = n;
        while left > 0 {
            date = if forward { date + Days::new(1) } else { date - Days::new(1) };
            if self.open(date) {
                left -= 1;
            }
        }
        date.and_time(dtm.time())
    }
}

impl BizDayProcessor for Holidays {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool> {
        Ok(self.open(dtm.date()))
    }

    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime> {
        Ok(match direction {
            Direction::Prev => self.step(dtm, 1, false),
            Direction::Next | Direction::Nearest => self.step(dtm, 1, true),
        })
    }

    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        Ok(self.step(dtm, num, true))
    }

    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        Ok(self.step(dtm, num, false))
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn at(y: i32, m: u32, day: u32, h: u32, mi: u32) -> DateTime<Tz> {
    London.with_ymd_and_hms(y, m, day, h, mi, 0).unwrap()
}

fn fmt(slot: &Slot) -> String {
    let ids: Vec<_> = slot.sources.iter().map(|s| s.rule_id.as_deref().unwrap_or("-")).collect();
    format!("{} {}", slot.at.format("%a %d %b %H:%M"), ids.join(","))
}

fn first<B: BizDayProcessor>(schedule: &Schedule<B>, n: usize) -> Vec<String> {
    schedule.slots().unwrap().take(n).map(|s| Ok(fmt(&s))).collect().unwrap()
}

// ---------------------------------------------------------------------------
// Merging rules
// ---------------------------------------------------------------------------

#[test]
fn rules_merge_in_time_order_with_their_ids() {
    let schedule = Schedule::builder(at(2026, 5, 27, 0, 0), WeekendSkipper::new())
        .named_rule("open", "YY-MM-DD~SBT09:00:00")
        .named_rule("month-end", "YY-MM-L~PBT18:00:00")
        .build()
        .unwrap();
    assert_eq!(
        first(&schedule, 5),
        [
            "Wed 27 May 09:00 open",
            "Thu 28 May 09:00 open",
            "Fri 29 May 09:00 open",
            "Fri 29 May 18:00 month-end",
            "Mon 01 Jun 09:00 open",
        ]
    );
}

#[test]
fn rules_on_the_same_instant_are_one_slot() {
    // 31 May 2026 is a Sunday: the last business day is the 29th.
    let schedule = Schedule::builder(at(2026, 5, 1, 0, 0), WeekendSkipper::new())
        .named_rule("29th", "YY-MM-29T18:00:00")
        .named_rule("month-end", "YY-MM-L~PBT18:00:00")
        .until(at(2026, 5, 31, 23, 0))
        .build()
        .unwrap();
    let slots: Vec<Slot> = schedule.slots().unwrap().collect().unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(fmt(&slots[0]), "Fri 29 May 18:00 29th,month-end");
    assert_eq!(slots[0].sources[0].actual, at(2026, 5, 29, 18, 0));
    assert_eq!(slots[0].sources[1].actual, at(2026, 5, 31, 18, 0));
}

#[test]
fn dates_moved_to_one_day_are_one_slot() {
    // Saturday's and Sunday's 09:00 move to Monday, where Monday has its own.
    let schedule = Schedule::builder(at(2026, 5, 29, 0, 0), WeekendSkipper::new())
        .rule("YY-MM-DD~NBT09:00:00")
        .build()
        .unwrap();
    let slots: Vec<Slot> = schedule.slots().unwrap().take(3).collect().unwrap();
    assert_eq!(slots[0].at, at(2026, 5, 29, 9, 0));
    assert_eq!(slots[1].at, at(2026, 6, 1, 9, 0));
    let actual: Vec<_> = slots[1].sources.iter().map(|s| s.actual.day()).collect();
    assert_eq!(actual, [30, 31, 1]);
    assert_eq!(slots[2].at, at(2026, 6, 2, 9, 0));
}

#[test]
fn moved_times_are_put_in_order() {
    // The rule yields Sat 09:00 and 13:00 (moved to Monday) before Monday's
    // own 09:00: slots must still be in time order.
    let schedule = Schedule::builder(at(2026, 5, 29, 0, 0), WeekendSkipper::new())
        .rule("YY-MM-DD~NBT[09,13]:00:00")
        .build()
        .unwrap();
    let times: Vec<_> = schedule.slots().unwrap().take(4).map(|s| Ok(s.at)).collect().unwrap();
    assert_eq!(times, [at(2026, 5, 29, 9, 0), at(2026, 5, 29, 13, 0), at(2026, 6, 1, 9, 0), at(2026, 6, 1, 13, 0)]);
}

// ---------------------------------------------------------------------------
// Start, until, count
// ---------------------------------------------------------------------------

#[test]
fn start_anchors_but_is_not_a_slot() {
    // Every other Tuesday and Thursday from Tuesday 7 January 2025.
    let schedule = Schedule::builder(at(2025, 1, 7, 0, 0), WeekendSkipper::new())
        .rule("YY-MM-[TUE,THU]/2WT18:00:00")
        .build()
        .unwrap();
    assert_eq!(
        first(&schedule, 4),
        ["Tue 07 Jan 18:00 -", "Thu 09 Jan 18:00 -", "Tue 21 Jan 18:00 -", "Thu 23 Jan 18:00 -"]
    );
    // The same phase from any point.
    let later: Vec<_> = schedule.slots_after(&at(2025, 1, 15, 0, 0)).unwrap().take(1).map(|s| Ok(fmt(&s))).collect().unwrap();
    assert_eq!(later, ["Tue 21 Jan 18:00 -"]);
}

#[test]
fn nothing_before_the_start() {
    // From Saturday 30 May: Saturday's and Sunday's dates move back to
    // Friday 29th, before the start.
    let schedule = Schedule::builder(at(2026, 5, 30, 0, 0), WeekendSkipper::new())
        .rule("YY-MM-DD~PBT09:00:00")
        .build()
        .unwrap();
    assert_eq!(first(&schedule, 1), ["Mon 01 Jun 09:00 -"]);
}

#[test]
fn until_bounds_the_observed_time() {
    // Month end is observed on Friday 29 May although its date is the 31st.
    let schedule = Schedule::builder(at(2026, 5, 1, 0, 0), WeekendSkipper::new())
        .rule("YY-MM-L~PBT18:00:00")
        .until(at(2026, 5, 30, 0, 0))
        .build()
        .unwrap();
    assert_eq!(first(&schedule, 5), ["Fri 29 May 18:00 -"]);
}

#[test]
fn count_is_after_exclusions_and_replayed() {
    let schedule = Schedule::builder(at(2025, 12, 22, 0, 0), Holidays::london())
        .rule("YY-MM-DD~SBT09:00:00")
        .exclude_date(d(2025, 12, 24))
        .count(4)
        .build()
        .unwrap();
    assert_eq!(first(&schedule, 9), ["Mon 22 Dec 09:00 -", "Tue 23 Dec 09:00 -", "Mon 29 Dec 09:00 -", "Tue 30 Dec 09:00 -"]);
    let rest: Vec<_> = schedule.slots_after(&at(2025, 12, 29, 12, 0)).unwrap().map(|s| Ok(fmt(&s))).collect().unwrap();
    assert_eq!(rest, ["Tue 30 Dec 09:00 -"]);
}

// ---------------------------------------------------------------------------
// Exclusions
// ---------------------------------------------------------------------------

#[test]
fn exclusions_match_actual_or_observed_dates() {
    let base = || {
        Schedule::builder(at(2025, 12, 24, 0, 0), Holidays::london())
            .rule("YY-MM-DD~NBT09:00:00")
            .exclude_rule("YY-12-25")
            .until(at(2025, 12, 29, 23, 0))
    };
    // Christmas Day, Boxing Day and the weekend all move to Monday 29th.
    assert_eq!(first(&base().build().unwrap(), 9).len(), 2);
    let by_actual: Vec<Slot> = base().build().unwrap().slots().unwrap().collect().unwrap();
    let moved: Vec<_> = by_actual[1].sources.iter().map(|s| s.actual.day()).collect();
    assert_eq!(moved, [26, 27, 28, 29]);
    // Matching the observed date: the 25th is never observed, nothing goes.
    let by_observed = base().exclude_match(ExcludeMatch::Observed).build().unwrap();
    let moved: Vec<_> = by_observed.slots().unwrap().collect::<Vec<_>>().unwrap()[1].sources.iter().map(|s| s.actual.day()).collect();
    assert_eq!(moved, [25, 26, 27, 28, 29]);
    // Excluding the 29th by observed date drops the whole slot.
    let monday = Schedule::builder(at(2025, 12, 24, 0, 0), Holidays::london())
        .rule("YY-MM-DD~NBT09:00:00")
        .exclude_date(d(2025, 12, 29))
        .exclude_match(ExcludeMatch::Observed)
        .until(at(2025, 12, 31, 23, 0))
        .build()
        .unwrap();
    assert_eq!(first(&monday, 9), ["Wed 24 Dec 09:00 -", "Tue 30 Dec 09:00 -", "Wed 31 Dec 09:00 -"]);
}

// ---------------------------------------------------------------------------
// slots_after agrees with slots
// ---------------------------------------------------------------------------

/// From every hour across `days`, the slots after it are the full stream's.
fn agrees<B: BizDayProcessor>(schedule: &Schedule<B>, days: u32) {
    let all: Vec<Slot> = schedule.slots().unwrap().take(3000).collect().unwrap();
    let mut after = *schedule.start() - Duration::hours(30);
    for _ in 0..days * 24 {
        let want: Vec<&Slot> = all.iter().filter(|s| s.at > after).take(5).collect();
        let got: Vec<Slot> = schedule.slots_after(&after).unwrap().take(5).collect().unwrap();
        assert_eq!(got.iter().collect::<Vec<_>>(), want, "after {after}");
        after += Duration::hours(1);
    }
}

#[test]
fn slots_after_agrees_across_holidays() {
    let start = at(2025, 12, 15, 0, 0);
    for rule in [
        "YY-MM-DD~NBT[09,13,18]:00:00",
        "YY-MM-DD~PBT[09,13,18]:00:00",
        "YY-MM-DD~SBT09:00:00",
        "YY-MM-L~PBT18:00:00",
        "YY-MM-[TUE,THU]/2W~NBT18:00:00",
        "YY-MM-DD~NBT1H:30:00",
        // Moved two and three business days later: further than ~NB.
        "YY-MM-DD~2NT[09,18]:00:00",
        "YY-MM-[MON,FRI]~3NT09:00:00",
        "YY-MM-DD~MFT12:00:00",
    ] {
        let schedule = Schedule::builder(start, Holidays::london()).rule(rule).build().unwrap();
        agrees(&schedule, 30);
    }
    let two = Schedule::builder(start, Holidays::london())
        .named_rule("batch", "YY-MM-DD~SBT[09,13,18]:00:00")
        .named_rule("month-end", "YY-MM-L~PBT18:00:00")
        .exclude_date(d(2025, 12, 31))
        .build()
        .unwrap();
    agrees(&two, 30);
}

#[test]
fn slots_after_agrees_across_dst() {
    // Clocks go forward on 29 March 2026 and back on 25 October 2026.
    for start in [at(2026, 3, 25, 0, 0), at(2026, 10, 21, 0, 0)] {
        let schedule = Schedule::builder(start, WeekendSkipper::new()).rule("YY-MM-DDT[00,01,02,03]:30:00").build().unwrap();
        agrees(&schedule, 8);
        // Elapsed steps: each day's times count from its first, not from
        // wherever iteration starts.
        let elapsed = Schedule::builder(start, WeekendSkipper::new())
            .rule("YY-MM-DDT7H:15:00")
            .interval_mode(crate::IntervalMode::Elapsed)
            .build()
            .unwrap();
        agrees(&elapsed, 8);
    }
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Bounded slots and periods
// ---------------------------------------------------------------------------

#[test]
fn slots_between_are_the_slots_there() {
    let schedule = Schedule::builder(at(2025, 12, 1, 0, 0), Holidays::london())
        .rule("YY-MM-DD~NBT09:00:00")
        .rule("YY-MM-L~PBT18:00:00")
        .build()
        .unwrap();
    let all: Vec<Slot> = schedule.slots().unwrap().take(200).collect().unwrap();
    for (from, to) in [(at(2025, 12, 24, 9, 0), at(2026, 1, 2, 9, 0)), (at(2025, 12, 31, 18, 0), at(2025, 12, 31, 18, 0))] {
        let want: Vec<&Slot> = all.iter().filter(|s| (from..=to).contains(&s.at)).collect();
        let got: Vec<Slot> = schedule.slots_between(from..=to).unwrap().collect().unwrap();
        assert_eq!(got.iter().collect::<Vec<_>>(), want, "{from}..={to}");
    }
    // An `until` inside the range still ends it; a count is still kept.
    let counted = Schedule::builder(at(2026, 1, 1, 0, 0), WeekendSkipper::new()).rule("YY-MM-01T00:00:00").count(3).build().unwrap();
    assert_eq!(counted.slots_between(at(2026, 2, 1, 0, 0)..=at(2027, 1, 1, 0, 0)).unwrap().count().unwrap(), 2);
}

#[test]
fn slots_between_end_when_every_date_is_excluded() {
    // No `until`, and nothing to yield: the range still ends the search.
    for rule in ["YY-MM-DDT09:00:00", "YY-MM-L~PBT09:00:00", "YY-MM-15~2PT09:00:00"] {
        let schedule = Schedule::builder(at(2026, 1, 1, 0, 0), WeekendSkipper::new())
            .rule(rule)
            .exclude_rule("YY-MM-DD")
            .build()
            .unwrap();
        assert_eq!(schedule.slots_between(at(2026, 1, 1, 0, 0)..=at(2036, 1, 1, 0, 0)).unwrap().count().unwrap(), 0, "{rule}");
    }
}

#[test]
fn until_keeps_slots_moved_back_onto_it() {
    // 31 January 2026 is a Saturday: its slot is Friday 30th, the until.
    for rule in ["YY-MM-L~PBT18:00:00", "YY-MM-L~PWT18:00:00", "YY-MM-L~MFT18:00:00", "YY-MM-L~PFRIT18:00:00"] {
        let schedule = Schedule::builder(at(2026, 1, 1, 0, 0), WeekendSkipper::new())
            .rule(rule)
            .until(at(2026, 1, 30, 18, 0))
            .build()
            .unwrap();
        assert_eq!(first(&schedule, 5), ["Fri 30 Jan 18:00 -"], "{rule}");
    }
}

fn periods(periods: crate::schedule::Periods<WeekendSkipper>) -> Vec<String> {
    let f = |t: DateTime<Tz>| t.format("%d %b").to_string();
    periods
        .map(|p| Ok(format!("{}({})-{}({})", f(p.start.at), f(p.start.actual), f(p.end.at), f(p.end.actual))))
        .collect()
        .unwrap()
}

#[test]
fn periods_run_from_the_start_with_both_times() {
    let schedule = Schedule::builder(at(2026, 1, 15, 0, 0), WeekendSkipper::new())
        .rule("YY-MM-L~PBT00:00:00")
        .count(3)
        .build()
        .unwrap();
    assert_eq!(
        periods(schedule.periods().unwrap()),
        ["15 Jan(15 Jan)-30 Jan(31 Jan)", "30 Jan(31 Jan)-27 Feb(28 Feb)", "27 Feb(28 Feb)-31 Mar(31 Mar)"]
    );
    // The period ending in the range starts at the slot before it.
    assert_eq!(
        periods(schedule.periods_between(at(2026, 2, 1, 0, 0)..=at(2026, 2, 28, 0, 0)).unwrap()),
        ["30 Jan(31 Jan)-27 Feb(28 Feb)"]
    );
}

#[test]
fn a_slot_at_the_start_opens_the_first_period() {
    let schedule = Schedule::builder(at(2026, 1, 15, 0, 0), WeekendSkipper::new())
        .rule("YY-MM-15T00:00:00")
        .until(at(2026, 3, 15, 0, 0))
        .build()
        .unwrap();
    assert_eq!(first(&schedule, 5).len(), 3);
    assert_eq!(periods(schedule.periods().unwrap()), ["15 Jan(15 Jan)-15 Feb(15 Feb)", "15 Feb(15 Feb)-15 Mar(15 Mar)"]);
}

#[test]
fn slots_moved_before_the_start_are_reported() {
    let lost = |start: DateTime<Tz>, rule: &str, observed: bool| -> Vec<String> {
        let mut builder = Schedule::builder(start, WeekendSkipper::new()).rule(rule).until(start + Duration::days(60));
        if observed {
            builder = builder.exclude_match(ExcludeMatch::Observed).exclude_date(d(2026, 1, 30));
        }
        let schedule = builder.build().unwrap();
        let lost = schedule.moved_before_start().unwrap();
        // They are not slots.
        assert!(schedule.slots().unwrap().all(|s| Ok(s.at >= start)).unwrap());
        lost.iter().map(|s| format!("{} from {}", s.at.format("%a %d"), s.sources[0].actual.format("%a %d"))).collect()
    };
    // Saturday 31 January 2026.
    let sat = at(2026, 1, 31, 0, 0);
    assert_eq!(lost(sat, "YY-MM-L~PBT00:00:00", false), ["Fri 30 from Sat 31"]);
    assert_eq!(lost(sat, "YY-MM-L~MFT00:00:00", false), ["Fri 30 from Sat 31"]);
    // Sunday 1 February: Saturday is before the start, so only Sunday's.
    assert_eq!(lost(at(2026, 2, 1, 0, 0), "YY-MM-DD~PBT09:00:00", false), ["Fri 30 from Sun 01"]);
    // Moved later, or not moved: nothing lost.
    assert!(lost(sat, "YY-MM-L~NBT00:00:00", false).is_empty());
    assert!(lost(at(2026, 1, 15, 0, 0), "YY-MM-L~PBT00:00:00", false).is_empty());
    // Excluded where it landed: dropped anyway, so not reported.
    assert!(lost(sat, "YY-MM-L~PBT00:00:00", true).is_empty());
    // Ends, even when every date is excluded.
    let none = Schedule::builder(sat, WeekendSkipper::new()).rule("YY-MM-DD~3PT00:00:00").exclude_rule("YY-MM-DD").build().unwrap();
    assert!(none.moved_before_start().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Timelines
// ---------------------------------------------------------------------------

fn monthly(start: DateTime<Tz>, rule: &str) -> Schedule<WeekendSkipper> {
    Schedule::builder(start, WeekendSkipper::new()).rule(rule).build().unwrap()
}

#[test]
fn a_timeline_is_its_schedules_merged() {
    use crate::schedule::Timeline;
    let a = monthly(at(2026, 1, 1, 0, 0), "YY-MM-DD~SBT09:00:00");
    let b = monthly(at(2026, 1, 15, 0, 0), "YY-MM-L~PBT09:00:00");
    let (from, to) = (at(2026, 1, 20, 0, 0), at(2026, 4, 1, 0, 0));
    let timeline = Timeline::new().add('a', a.clone()).add('b', b.clone());
    let moments: Vec<_> = timeline.between(from..=to).unwrap().collect().unwrap();
    // Every slot of each, once, in time order.
    for (key, schedule) in [('a', &a), ('b', &b)] {
        let want: Vec<Slot> = schedule.slots_between(from..=to).unwrap().collect().unwrap();
        let got: Vec<Slot> =
            moments.iter().flat_map(|m| m.entries.iter().filter(|e| e.key == key).map(|e| e.slot.clone())).collect();
        assert_eq!(got, want, "{key}");
    }
    assert!(moments.windows(2).all(|w| w[0].at < w[1].at));
    // At month ends both, in the order added.
    let both: Vec<String> = moments.iter().filter(|m| m.entries.len() == 2).map(|m| m.at.format("%d %b").to_string()).collect();
    assert_eq!(both, ["30 Jan", "27 Feb", "31 Mar"]);
    assert!(moments.iter().filter(|m| m.entries.len() == 2).all(|m| m.entries.iter().map(|e| e.key).collect::<String>() == "ab"));
    // `since` is the period's start, as `periods` gives it.
    let periods: Vec<_> = b.periods_between(from..=to).unwrap().map(|p| Ok(p.start)).collect().unwrap();
    let since: Vec<_> = moments.iter().flat_map(|m| m.entries.iter().filter(|e| e.key == 'b').map(|e| e.since)).collect();
    assert_eq!(since, periods);
    assert_eq!(since[0].at, at(2026, 1, 15, 0, 0), "the first is the schedule's start");
}

#[test]
fn a_timeline_orders_instants_across_zones() {
    use crate::schedule::Timeline;
    let new_york = |y, m, d, h| chrono_tz::America::New_York.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap();
    let london = monthly(at(2026, 7, 1, 0, 0), "YY-MM-DD~SBT16:00:00");
    let ny = monthly(new_york(2026, 7, 1, 0), "YY-MM-DD~SBT11:00:00");
    let moments: Vec<_> = Timeline::new()
        .add("ny", ny)
        .add("london", london)
        .between(at(2026, 7, 1, 0, 0)..=at(2026, 7, 1, 23, 0))
        .unwrap()
        .map(|m| Ok((m.at.to_rfc3339(), m.entries.iter().map(|e| e.key).collect::<Vec<_>>())))
        .collect()
        .unwrap();
    // 16:00 in London is 11:00 in New York: one moment, in New York's zone.
    assert_eq!(moments, [("2026-07-01T11:00:00-04:00".to_string(), vec!["ny", "london"])]);
}

#[test]
fn timeline_edges() {
    use crate::schedule::Timeline;
    let a = || monthly(at(2026, 3, 1, 0, 0), "YY-MM-01T00:00:00");
    let error = |t: Timeline<&str, WeekendSkipper>| t.between(at(2026, 1, 1, 0, 0)..=at(2027, 1, 1, 0, 0)).unwrap_err().to_string();
    assert!(error(Timeline::new()).contains("at least one"));
    assert!(error(Timeline::new().add("a", a()).add("a", a())).contains("used twice"));
    // A range before a schedule starts has nothing from it.
    let early = Timeline::new().add("a", a()).between(at(2026, 1, 1, 0, 0)..=at(2026, 2, 1, 0, 0)).unwrap();
    assert_eq!(early.count().unwrap(), 0);
    // A slot at the schedule's start: `since` is the start itself.
    let first = Timeline::new().add("a", a()).between(at(2026, 3, 1, 0, 0)..=at(2026, 3, 1, 0, 0)).unwrap().next().unwrap().unwrap();
    assert_eq!((first.entries[0].since.at, first.at), (at(2026, 3, 1, 0, 0), at(2026, 3, 1, 0, 0)));
}

// ---------------------------------------------------------------------------
// Derived rules
// ---------------------------------------------------------------------------

fn derived(start: DateTime<Tz>, basis: &str, rule: &str) -> Schedule<Holidays> {
    Schedule::builder(start, Holidays::london()).basis_rule("basis", basis).named_rule("derived", rule).build().unwrap()
}

#[test]
fn derived_rules_count_from_the_adjusted_date() {
    // Two business days before each adjusted month end: never `YY-MM-L~2P`'s
    // count from the unadjusted date.
    let schedule = derived(at(2025, 11, 1, 0, 0), "YY-MM-L~MFT12:00:00", "{basis}~2PT09:00:00");
    let got: Vec<Slot> = schedule.slots().unwrap().take(14).collect().unwrap();
    let holidays = Holidays::london();
    let payments = Schedule::builder(at(2025, 11, 1, 0, 0), Holidays::london()).rule("YY-MM-L~MFT12:00:00").build().unwrap();
    let payments: Vec<Slot> = payments.slots().unwrap().take(14).collect().unwrap();
    for (slot, payment) in got.iter().zip(&payments) {
        let want = holidays.sub(&payment.at.naive_local(), 2).unwrap().date();
        assert_eq!(slot.at.date_naive(), want);
        assert_eq!(slot.at.format("%H:%M").to_string(), "09:00");
        assert_eq!(slot.sources[0].basis, Some(payment.at), "provenance");
        assert_eq!(slot.sources[0].actual.date_naive(), payment.at.date_naive());
    }
    // Sat 31 Jan 2026: paid Fri 30 Jan; derived Wed 28 Jan (`~2P` from the 31st gives Thu 29).
    assert!(got.iter().any(|s| s.at == at(2026, 1, 28, 9, 0)));
}

#[test]
fn a_basis_has_no_slots_of_its_own() {
    let schedule = derived(at(2026, 1, 1, 0, 0), "YY-MM-15T12:00:00", "{basis}T_:_:_");
    // `T_:_:_` keeps the basis slot's time; no adjustment keeps its date.
    assert_eq!(first(&schedule, 2), ["Thu 15 Jan 12:00 derived", "Sun 15 Feb 12:00 derived"]);
    // A rule that is referred to and not a basis keeps its own slots.
    let both = Schedule::builder(at(2026, 1, 1, 0, 0), Holidays::london())
        .named_rule("payment", "YY-MM-15~NBT12:00:00")
        .named_rule("notice", "{payment}~1PT12:00:00")
        .build()
        .unwrap();
    assert_eq!(first(&both, 4), ["Wed 14 Jan 12:00 notice", "Thu 15 Jan 12:00 payment", "Fri 13 Feb 12:00 notice", "Mon 16 Feb 12:00 payment"]);
}

#[test]
fn derived_slots_follow_the_schedule_around_them() {
    // Exclusions match the actual date, the basis slot's date (D3): no
    // derived slot for the drawdown.
    let start = at(2026, 1, 1, 0, 0);
    let schedule = Schedule::builder(start, Holidays::london())
        .basis_rule("boundary", "YY-MM-15~MFT12:00:00")
        .named_rule("fixing", "{boundary}~2PT11:00:00")
        .exclude_date(d(2026, 1, 15))
        .until(at(2026, 3, 31, 0, 0))
        .build()
        .unwrap();
    assert_eq!(first(&schedule, 5), ["Thu 12 Feb 11:00 fixing", "Thu 12 Mar 11:00 fixing"]);
    // A derived slot moved before the start is dropped, and reported.
    let late = Schedule::builder(at(2026, 1, 14, 0, 0), Holidays::london())
        .basis_rule("boundary", "YY-MM-15~MFT12:00:00")
        .named_rule("fixing", "{boundary}~2PT11:00:00")
        .build()
        .unwrap();
    let lost = late.moved_before_start().unwrap();
    assert_eq!(lost.iter().map(|s| s.at).collect::<Vec<_>>(), [at(2026, 1, 13, 11, 0)]);
    assert_eq!(first(&late, 1), ["Thu 12 Feb 11:00 fixing"]);
    // `~SB` after a reference drops what is not a business day.
    let skip = derived(start, "YY-MM-DDT12:00:00", "{basis}~SBT12:00:00");
    assert!(first(&skip, 10).iter().all(|s| !s.starts_with("Sat") && !s.starts_with("Sun")));
}

#[test]
fn derived_slots_after_agree() {
    for (basis, rule) in [
        ("YY-MM-L~MFT12:00:00", "{basis}~2PT09:00:00"),
        ("YY-MM-DD~NBT09:00:00", "{basis}~2NT18:00:00"),
        ("YY-MM-[1,15]~PBT10:00:00", "{basis}~MONT_:_:_"),
    ] {
        agrees(&derived(at(2025, 11, 1, 0, 0), basis, rule), 70);
    }
}

#[test]
fn invalid_derived_rules() {
    let error = |rules: &[(&str, &str, bool)]| {
        let mut builder = Schedule::builder(at(2026, 1, 1, 0, 0), WeekendSkipper::new());
        for (id, rule, basis) in rules {
            builder = if *basis { builder.basis_rule(*id, *rule) } else { builder.named_rule(*id, *rule) };
        }
        builder.build().unwrap_err().to_string()
    };
    type Case<'a> = (&'a [(&'a str, &'a str, bool)], &'a str);
    let cases: &[Case] = &[
        (&[("a", "{b}~2PT09:00:00", false)], "which no rule is"),
        (&[("a", "{a}~2PT09:00:00", false)], "itself derived"),
        (&[("p", "YY-MM-15T12:00:00", true), ("a", "{p}~2PT09:00:00", false), ("b", "{a}~1PT09:00:00", false)], "itself derived"),
        (&[("p", "YY-MM-15T12:00:00", true)], "not a basis"),
        (&[("p", "YY-MM-15T12:00:00", true), ("q", "YY-MM-01T12:00:00", false)], "no rule refers to it"),
        (&[("p", "YY-MM-15T12:00:00", true), ("a", "{p}/2WT09:00:00", false)], "only an adjustment"),
        (&[("p", "YY-MM-15T12:00:00", true), ("a", "{p}~2P", false)], "then `T` and a time"),
        (&[("p", "YY-MM-15T12:00:00", true), ("a", "{p}~2PT1H:00:00", false)], "a fixed time"),
        (&[("p", "YY-MM-15T12:00:00", true), ("a", "{p}~XXT09:00:00", false)], "the adjustment"),
        (&[("p", "YY-MM-15T12:00:00", true), ("a", "{p~2PT09:00:00", false)], "closed by"),
    ];
    for (rules, want) in cases {
        let got = error(rules);
        assert!(got.contains(want), "{rules:?}: {got}");
    }
}

// ---------------------------------------------------------------------------
// Invalid schedules
// ---------------------------------------------------------------------------

fn error(builder: crate::schedule::ScheduleBuilder<WeekendSkipper>) -> String {
    match builder.build() {
        Err(Error::InvalidSchedule(why)) => why,
        other => panic!("expected InvalidSchedule, got {other:?}"),
    }
}

#[test]
fn invalid_schedules() {
    let b = || Schedule::builder(at(2026, 1, 1, 0, 0), WeekendSkipper::new());
    assert!(error(b()).contains("at least one rule"));
    // A rule is one rule: bounds, exclusions and unions do not parse.
    for rule in [
        "YY-MM-DDT09:00:00 @2026-01-05",
        "YY-MM-DDT09:00:00 [cal=GBLO]",
        "YY-MM-DDT09:00:00[Europe/London]",
        "YY-MM-DDT09:00:00 x3",
        "YY-MM-DDT09:00:00 ! YY-12-25",
        "YY-MM-01T09:00:00 | YY-MM-15T09:00:00",
    ] {
        assert!(error(b().rule(rule)).starts_with(&format!("rule 1 (`{rule}`): invalid")), "{rule}");
    }
    assert!(error(b().named_rule("a", "YY-MM-DDT09:00:00").named_rule("a", "YY-MM-LT09:00:00")).contains("`a` is used twice"));
    assert!(error(b().named_rule("open", "YY-MM-DD")).starts_with("rule `open` (`YY-MM-DD`)"));
    assert!(error(b().rule("YY-MM-DDT09:00:00").exclude_rule("YY-13-01")).starts_with("exclusion `YY-13-01`"));
    assert!(error(b().rule("YY-MM-DDT09:00:00").until(at(2025, 12, 31, 0, 0))).contains("before the start"));
    assert!(error(b().rule("YY-MM-DDT09:00:00").count(0)).contains("at least 1"));
}

// ---------------------------------------------------------------------------
// Selectors
// ---------------------------------------------------------------------------

/// Accrual batches, three a business day, from 1 January 2026.
fn batches() -> Schedule<Holidays> {
    Schedule::builder(at(2026, 1, 1, 0, 0), Holidays::london()).rule("YY-MM-DD~SBT[09,13,18]:00:00").build().unwrap()
}

/// `next_after`, with the slots it reports missed.
fn next(selector: &Selector<Holidays>, definition: &Schedule<Holidays>, after: DateTime<Tz>, horizon: DateTime<Tz>) -> (Next, Vec<String>) {
    let mut missed = Vec::new();
    let next = selector.next_after(definition, &after, &horizon, |slot| missed.push(fmt(&slot))).unwrap();
    (next, missed)
}

fn due(next: Next) -> String {
    match next {
        Next::Due(slot) => fmt(&slot),
        other => panic!("expected Due, got {other:?}"),
    }
}

#[test]
fn selects_the_month_end_batch() {
    let definition = batches();
    let account = definition.selector(at(2026, 3, 15, 0, 0)).named_rule("month-end", "YY-MM-L~PBT18:00:00").build().unwrap();
    // 31 May 2026 is a Sunday.
    let (found, missed) = next(&account, &definition, at(2026, 5, 1, 0, 0), at(2026, 7, 1, 0, 0));
    let Next::Due(slot) = found else { panic!("{found:?}") };
    assert_eq!(fmt(&slot), "Fri 29 May 18:00 month-end");
    assert_eq!(slot.sources[0].actual, at(2026, 5, 31, 18, 0));
    assert!(missed.is_empty());
    // From the due that fired, the next is June's.
    let (found, _) = next(&account, &definition, slot.at, at(2026, 8, 1, 0, 0));
    assert_eq!(due(found), "Tue 30 Jun 18:00 month-end");
}

#[test]
fn the_window_starts_the_selection() {
    let definition = batches();
    let opens = definition.selector(at(2026, 6, 1, 0, 0)).rule("YY-MM-DD~SBT09:00:00").build().unwrap();
    // Registered before it opens: `after` is anything before its start.
    let (found, missed) = next(&opens, &definition, at(2026, 5, 20, 0, 0), at(2026, 7, 1, 0, 0));
    assert_eq!(due(found), "Mon 01 Jun 09:00 -");
    assert!(missed.is_empty());
}

#[test]
fn instants_the_definition_lacks_are_missed() {
    let definition = batches();
    // Calendar days at 09:00: weekends and holidays are not batches.
    let daily = definition.selector(at(2026, 1, 1, 0, 0)).rule("YY-MM-DDT09:00:00").build().unwrap();
    let (found, missed) = next(&daily, &definition, at(2026, 5, 29, 9, 0), at(2026, 7, 1, 0, 0));
    assert_eq!(due(found), "Mon 01 Jun 09:00 -");
    assert_eq!(missed, ["Sat 30 May 09:00 -", "Sun 31 May 09:00 -"]);
    // 17:00 is never a batch: nothing is found, and every month end in the
    // horizon is missed.
    let wrong = definition.selector(at(2026, 1, 1, 0, 0)).rule("YY-MM-L~PBT17:00:00").build().unwrap();
    let (found, missed) = next(&wrong, &definition, at(2026, 1, 1, 0, 0), at(2026, 3, 31, 23, 0));
    assert_eq!(found, Next::NotFound);
    assert_eq!(missed, ["Fri 30 Jan 17:00 -", "Fri 27 Feb 17:00 -", "Tue 31 Mar 17:00 -"]);
}

#[test]
fn the_end_of_the_window_is_done() {
    let definition = batches();
    let closes = definition.selector(at(2026, 1, 1, 0, 0)).rule("YY-MM-L~PBT18:00:00").until(at(2026, 2, 28, 0, 0)).build().unwrap();
    let (found, _) = next(&closes, &definition, at(2026, 1, 1, 0, 0), at(2027, 1, 1, 0, 0));
    assert_eq!(due(found), "Fri 30 Jan 18:00 -");
    let (found, _) = next(&closes, &definition, at(2026, 1, 30, 18, 0), at(2027, 1, 1, 0, 0));
    assert_eq!(due(found), "Fri 27 Feb 18:00 -");
    let (found, _) = next(&closes, &definition, at(2026, 2, 27, 18, 0), at(2027, 1, 1, 0, 0));
    assert_eq!(found, Next::Done);
}

#[test]
fn a_count_becomes_until() {
    let definition = batches();
    // Three month ends; January's is excluded from the selection, so the
    // third is April's. Exclusions match the actual date: Saturday 31
    // January, observed on Friday 30.
    let three = definition
        .selector(at(2026, 1, 1, 0, 0))
        .rule("YY-MM-L~PBT18:00:00")
        .exclude_date(d(2026, 1, 31))
        .count(3)
        .build()
        .unwrap();
    assert_eq!(three.until(), Some(&at(2026, 4, 30, 18, 0)));
    assert_eq!(three.schedule().slots().unwrap().count().unwrap(), 3);
    // Missed slots count: a definition that drops February's month end does
    // not give the instance a fourth.
    let gappy = Schedule::builder(at(2026, 1, 1, 0, 0), Holidays::london())
        .rule("YY-MM-DD~SBT[09,13,18]:00:00")
        .exclude_date(d(2026, 2, 27))
        .build()
        .unwrap();
    let (found, missed) = next(&three, &gappy, at(2026, 2, 1, 0, 0), at(2027, 1, 1, 0, 0));
    assert_eq!(due(found), "Tue 31 Mar 18:00 -");
    assert_eq!(missed, ["Fri 27 Feb 18:00 -"]);
    let (found, _) = next(&three, &gappy, at(2026, 4, 30, 18, 0), at(2027, 1, 1, 0, 0));
    assert_eq!(found, Next::Done);
}

#[test]
fn uncovered_lists_what_the_definition_lacks() {
    let definition = Schedule::builder(at(2026, 1, 1, 0, 0), Holidays::london())
        .rule("YY-MM-DD~SBT[09,13,18]:00:00")
        .exclude_rule("YY-06-DD")
        .build()
        .unwrap();
    let month_end = definition.selector(at(2026, 1, 1, 0, 0)).rule("YY-MM-L~PBT18:00:00").build().unwrap();
    let year = at(2026, 1, 1, 0, 0)..=at(2026, 12, 31, 23, 59);
    let gaps: Vec<_> = month_end.uncovered(&definition, year.clone()).unwrap().iter().map(fmt).collect();
    assert_eq!(gaps, ["Tue 30 Jun 18:00 -"]);
    let evening = definition.selector(at(2026, 1, 1, 0, 0)).rule("YY-MM-DD~SBT18:00:00").build().unwrap();
    assert_eq!(evening.uncovered(&definition, year.clone()).unwrap().len(), 22, "June's business days");
    let morning_and_evening = definition.selector(at(2026, 1, 1, 0, 0)).rule("YY-MM-[MON-FRI]T[09,18]:00:00").build().unwrap();
    // June's 22 business days, and New Year's Day, twice each.
    assert_eq!(morning_and_evening.uncovered(&definition, year).unwrap().len(), 46);
}

/// Stepping `next_after` from due to due gives the slots both schedules
/// have, and the misses are the rest of the selector's.
#[test]
fn lazy_selection_is_the_intersection() {
    let definition = batches();
    let horizon = at(2026, 3, 1, 0, 0);
    for rule in ["YY-MM-DDT[09,13]:00:00", "YY-MM-DD~NBT13:00:00", "YY-MM-[MON,FRI]T18:00:00", "YY-MM-L~PBT18:00:00", "YY-MM-DD~2NT09:00:00"] {
        let selector = definition.selector(at(2025, 12, 20, 0, 0)).rule(rule).build().unwrap();
        let produced: BTreeSet<_> = definition.slots().unwrap().take_while(|s| Ok(s.at <= horizon)).map(|s| Ok(s.at)).collect().unwrap();
        let wanted: Vec<_> = selector.schedule().slots().unwrap().take_while(|s| Ok(s.at <= horizon)).map(|s| Ok(s.at)).collect().unwrap();
        let (mut dues, mut misses) = (Vec::new(), Vec::new());
        let mut after = at(2025, 12, 1, 0, 0);
        loop {
            match selector.next_after(&definition, &after, &horizon, |m| misses.push(m.at)).unwrap() {
                Next::Due(slot) => {
                    dues.push(slot.at);
                    after = slot.at;
                }
                Next::NotFound => break,
                Next::Done => panic!("{rule}: no until, so never done"),
            }
        }
        let (expect_dues, expect_misses): (Vec<_>, Vec<_>) = wanted.iter().partition(|at| produced.contains(at));
        assert_eq!(dues, expect_dues, "{rule}");
        assert_eq!(misses, expect_misses, "{rule}");
        assert_eq!(selector.uncovered(&definition, at(2025, 12, 1, 0, 0)..=horizon).unwrap().iter().map(|s| s.at).collect::<Vec<_>>(), expect_misses, "{rule}");
    }
}

#[test]
fn a_selector_needs_its_definition() {
    let definition = batches();
    let account = definition.selector(at(2026, 1, 1, 0, 0)).rule("YY-MM-L~PBT18:00:00").build().unwrap();
    let elsewhere = Schedule::builder(chrono_tz::America::New_York.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(), Holidays::london())
        .rule("YY-MM-DD~SBT18:00:00")
        .build()
        .unwrap();
    let err = account.next_after(&elsewhere, &at(2026, 1, 1, 0, 0), &at(2026, 3, 1, 0, 0), |_| {}).unwrap_err();
    assert!(matches!(err, Error::InvalidSchedule(why) if why.contains("another time zone")));
}
