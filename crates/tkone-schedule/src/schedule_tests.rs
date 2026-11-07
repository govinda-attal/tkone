//! Schedules: merged rules, slots unique by instant, the start as an anchor,
//! `until`, counts, exclusions, and `slots_after` agreeing with `slots`.

use std::collections::BTreeSet;

use chrono::{DateTime, Datelike, Days, Duration, NaiveDate, NaiveDateTime, TimeZone, Weekday};
use chrono_tz::Europe::London;
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use crate::biz_day::{BizDayProcessor, Direction, WeekendSkipper};
use crate::schedule::{ExcludeMatch, Schedule, Slot};
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
    assert!(error(b().rule("YY-MM-DDT09:00:00 @2026-01-05")).contains("set them on the schedule"));
    assert!(error(b().rule("YY-MM-DDT09:00:00 @2026-01-05[Europe/London]")).contains("set them on the schedule"));
    assert!(error(b().rule("YY-MM-DDT09:00:00 [cal=GBLO]")).contains("set them on the schedule"));
    assert!(error(b().rule("YY-MM-DDT09:00:00[Europe/London]")).contains("invalid time spec"));
    assert!(error(b().rule("YY-MM-DDT09:00:00 x3")).contains("set them on the schedule"));
    assert!(error(b().rule("YY-MM-DDT09:00:00 ! YY-12-25")).contains("exclusions"));
    assert!(error(b().rule("YY-MM-01T09:00:00 | YY-MM-15T09:00:00")).contains("union"));
    assert!(error(b().named_rule("a", "YY-MM-DDT09:00:00").named_rule("a", "YY-MM-LT09:00:00")).contains("`a` is used twice"));
    assert!(error(b().named_rule("open", "YY-MM-DD")).starts_with("rule `open` (`YY-MM-DD`)"));
    assert!(error(b().rule("YY-MM-DDT09:00:00").exclude_rule("YY-13-01")).starts_with("exclusion `YY-13-01`"));
    assert!(error(b().rule("YY-MM-DDT09:00:00").until(at(2025, 12, 31, 0, 0))).contains("before the start"));
}
