//! Schedule documents: reading YAML and JSON, the checks a document gets
//! beyond the builder's, and calendars resolved by code.

use chrono::{DateTime, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use crate::biz_day::{BizDayProcessor, Direction, WeekendSkipper};
use crate::schedule::document::{BusinessDays, CalendarResolver, LocalTime, ScheduleDoc};
use crate::schedule::Schedule;
use crate::{Error, Result};

/// Weekends, with a code and a time zone, like a holiday calendar.
#[derive(Debug, Clone)]
struct Place {
    code: &'static str,
    zone: Tz,
}

impl BizDayProcessor for Place {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool> {
        WeekendSkipper::new().is_biz_day(dtm)
    }
    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime> {
        WeekendSkipper::new().find_biz_day(dtm, direction)
    }
    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        WeekendSkipper::new().add(dtm, num)
    }
    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
        WeekendSkipper::new().sub(dtm, num)
    }
    fn code(&self) -> Option<String> {
        Some(self.code.into())
    }
    fn time_zone(&self) -> Option<Tz> {
        Some(self.zone)
    }
}

struct Places;

impl CalendarResolver for Places {
    type Bdp = Place;
    fn resolve(&self, code: &str) -> Result<Place> {
        match code {
            "GBLO" => Ok(Place { code: "GBLO", zone: chrono_tz::Europe::London }),
            "USNY" => Ok(Place { code: "USNY", zone: chrono_tz::America::New_York }),
            _ => Err(Error::Calendar(format!("no calendar {code}"))),
        }
    }
}

fn yaml(text: &str) -> ScheduleDoc {
    serde_yaml_ng::from_str(text).unwrap()
}

fn build(text: &str) -> Result<Schedule<BusinessDays<Place>>> {
    yaml(text).schedule(&Places)
}

fn invalid(text: &str) -> String {
    match build(text) {
        Err(Error::InvalidSchedule(why)) => why,
        other => panic!("expected InvalidSchedule, got {other:?}"),
    }
}

fn london(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Tz> {
    chrono_tz::Europe::London.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
}

fn times(schedule: &Schedule<BusinessDays<Place>>, n: usize) -> Vec<String> {
    schedule.slots().unwrap().take(n).map(|s| Ok(s.at.format("%a %d %b %H:%M").to_string())).collect().unwrap()
}

const ACCRUAL: &str = "
version: 1
time_zone: Europe/London
calendar: GBLO
start: 2026-05-28
until: 2026-06-01
rules:
  - id: batch
    rule: YY-MM-DD~SBT[09,13,18]:00:00
exclude:
  dates: [2026-05-29]
";

#[test]
fn reads_a_definition() {
    let schedule = build(ACCRUAL).unwrap();
    assert_eq!(schedule.zone(), chrono_tz::Europe::London);
    // `until` is a date: the whole of 1 June is in.
    assert_eq!(
        times(&schedule, 9),
        ["Thu 28 May 09:00", "Thu 28 May 13:00", "Thu 28 May 18:00", "Mon 01 Jun 09:00", "Mon 01 Jun 13:00", "Mon 01 Jun 18:00"]
    );
    let first = schedule.slots().unwrap().next().unwrap().unwrap();
    assert_eq!(first.sources[0].rule_id.as_deref(), Some("batch"));
}

#[test]
fn json_and_yaml_are_the_same_document() {
    let from_yaml = yaml(ACCRUAL);
    let json = serde_json::to_string(&from_yaml).unwrap();
    let from_json: ScheduleDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(from_json, from_yaml);
    // Written back, only the fields given appear.
    let written = serde_yaml_ng::to_string(&from_yaml).unwrap();
    assert!(!written.contains("options"), "{written}");
    assert!(!written.contains("count"), "{written}");
    assert_eq!(yaml(&written), from_yaml);
}

#[test]
fn reads_options_and_local_times() {
    let doc = yaml(
        "
version: 1
time_zone: Europe/London
start: 2026-03-29T00:30
rules: [{ rule: YY-MM-DDT6H:30:00 }]
exclude: { match: observed }
options: { dst_policy: strict, interval_mode: elapsed, time_anchor: midnight }
",
    );
    assert_eq!(doc.start, LocalTime::DateTime(chrono::NaiveDate::from_ymd_opt(2026, 3, 29).unwrap().and_hms_opt(0, 30, 0).unwrap()));
    assert_eq!(doc.options.interval_mode, Some(crate::IntervalMode::Elapsed));
    assert_eq!(doc.exclude.by, crate::schedule::ExcludeMatch::Observed);
    assert!(doc.schedule(&Places).is_ok());
}

#[test]
fn unknown_fields_and_offsets_are_refused() {
    let typo = "version: 1\ntime_zone: Europe/London\nstart: 2026-01-01\nuntill: 2026-12-31\nrules: [{ rule: YY-MM-DDT09:00:00 }]";
    let err = serde_yaml_ng::from_str::<ScheduleDoc>(typo).unwrap_err().to_string();
    assert!(err.contains("unknown field `untill`"), "{err}");
    let offset = "version: 1\ntime_zone: Europe/London\nstart: 2026-01-01T09:00:00+01:00\nrules: [{ rule: YY-MM-DDT09:00:00 }]";
    let err = serde_yaml_ng::from_str::<ScheduleDoc>(offset).unwrap_err().to_string();
    assert!(err.contains("has a UTC offset"), "{err}");
}

#[test]
fn document_checks() {
    let base = |extra: &str| format!("version: 1\nstart: 2026-01-01\n{extra}");
    assert!(invalid(&base("time_zone: Europe/London\nrules: [{ rule: YY-MM-DDT09:00:00 }]").replace("version: 1", "version: 2")).contains("version 2"));
    assert!(invalid(&base("rules: [{ rule: YY-MM-DDT09:00:00 }]")).contains("`time_zone` is required"));
    assert!(invalid(&base("time_zone: Europe/Londres\nrules: [{ rule: YY-MM-DDT09:00:00 }]")).contains("not an IANA time zone"));
    // The builder's checks come through.
    assert!(invalid(&base("time_zone: Europe/London\nrules: []")).contains("at least one rule"));
}

#[test]
fn business_days_need_a_calendar() {
    let doc = |rule: &str, extra: &str| format!("version: 1\ntime_zone: Europe/London\nstart: 2026-01-01\n{extra}rules: [{{ rule: '{rule}' }}]\n");
    for rule in ["YY-MM-DD~SBT09:00:00", "YY-MM-L~PBT18:00:00", "YY-MM-BD#3T09:00:00", "YY-MM-L~2NT09:00:00"] {
        assert!(invalid(&doc(rule, "")).contains("uses business days"), "{rule}");
        assert!(build(&doc(rule, "calendar: WEEKENDS\n")).is_ok(), "{rule}");
    }
    // Weekdays and plain dates need no calendar.
    for rule in ["YY-MM-DDT09:00:00", "YY-MM-[MON-FRI]T09:00:00", "YY-MM-L~WT18:00:00", "YY-MM-WD#3T09:00:00"] {
        assert!(build(&doc(rule, "")).is_ok(), "{rule}");
    }
    let excluded = "version: 1\ntime_zone: Europe/London\nstart: 2026-01-01\nrules: [{ rule: YY-MM-DDT09:00:00 }]\nexclude: { rules: [YY-MM-BD#1] }";
    assert!(invalid(excluded).contains("exclusion `YY-MM-BD#1` uses business days"));
}

#[test]
fn calendars_resolve_by_code() {
    let doc = |code: &str| format!("version: 1\ntime_zone: Europe/London\ncalendar: {code}\nstart: 2026-01-01\nrules: [{{ rule: YY-MM-DD~SBT09:00:00 }}]");
    assert!(build(&doc("GBLO")).is_ok());
    assert!(matches!(build(&doc("XXXX")), Err(Error::Calendar(why)) if why.contains("no calendar XXXX")));
    // New York's business days judged on London dates fail at the first
    // slot whose date differs there: 02:00 in London is still yesterday in New York.
    let early = "version: 1\ntime_zone: Europe/London\ncalendar: USNY\nstart: 2026-01-01\nrules: [{ rule: YY-MM-DD~SBT02:00:00 }]";
    let err = build(early).unwrap().slots().unwrap().next().unwrap_err();
    assert!(matches!(err, Error::Calendar(_)), "{err:?}");
    // Its own zone is fine.
    let ny = early.replace("Europe/London", "America/New_York");
    assert!(build(&ny).unwrap().slots().unwrap().take(3).count().is_ok());
    assert_eq!(build(&doc("GBLO")).unwrap().slots().unwrap().next().unwrap().unwrap().at, london(2026, 1, 1, 9, 0));
}

// ---------------------------------------------------------------------------
// Instance documents
// ---------------------------------------------------------------------------

const ACCOUNT: &str = "
version: 1
start: 2026-03-15
until: 2027-03-14
rules:
  - id: month-end
    rule: YY-MM-L~PBT18:00:00
exclude:
  dates: [2026-12-31]
";

#[test]
fn reads_an_instance() {
    let definition = build(&ACCRUAL.replace("until: 2026-06-01\n", "")).unwrap();
    let account = yaml(ACCOUNT).selector(&definition).unwrap();
    assert_eq!(account.start(), &london(2026, 3, 15, 0, 0));
    // `until` is a date: the whole of 14 March 2027.
    assert_eq!(account.until(), Some(&crate::utils::just_before(&london(2027, 3, 15, 0, 0))));
    // The definition excludes 29 May, May's month end: it is missed.
    let mut missed = Vec::new();
    let next = account.next_after(&definition, &london(2026, 5, 1, 0, 0), &london(2026, 7, 1, 0, 0), |m| missed.push(m.at)).unwrap();
    let crate::schedule::Next::Due(slot) = next else { panic!("{next:?}") };
    assert_eq!(missed, [london(2026, 5, 29, 18, 0)]);
    assert_eq!(slot.at, london(2026, 6, 30, 18, 0));
    assert_eq!(slot.sources[0].rule_id.as_deref(), Some("month-end"));
    // A count becomes `until`.
    let three = yaml(&ACCOUNT.replace("until: 2027-03-14", "count: 3")).selector(&definition).unwrap();
    assert_eq!(three.until(), Some(&london(2026, 5, 29, 18, 0)));
}

#[test]
fn an_instance_uses_its_definitions_zone_calendar_and_options() {
    let definition = build(ACCRUAL).unwrap();
    for (field, line) in [
        ("time_zone", "time_zone: Europe/London"),
        ("calendar", "calendar: GBLO"),
        ("options", "options: { dst_policy: strict }"),
    ] {
        let doc = format!("{ACCOUNT}{line}\n");
        match yaml(&doc).selector(&definition) {
            Err(Error::InvalidSchedule(why)) => assert!(why.contains(&format!("may not set `{field}`")), "{why}"),
            other => panic!("{field}: {other:?}"),
        }
    }
    // With no calendar in the definition, an instance's business days would
    // silently be weekdays.
    let plain = build("version: 1\ntime_zone: Europe/London\nstart: 2026-01-01\nrules: [{ rule: YY-MM-DDT18:00:00 }]").unwrap();
    match yaml(ACCOUNT).selector(&plain) {
        Err(Error::InvalidSchedule(why)) => assert!(why.contains("its definition has no `calendar`"), "{why}"),
        other => panic!("{other:?}"),
    }
    let weekday = ACCOUNT.replace("YY-MM-L~PBT18:00:00", "YY-MM-L~WT18:00:00");
    assert!(yaml(&weekday).selector(&plain).is_ok());
}

#[test]
fn derived_rules_in_a_document() {
    let text = "
version: 1
time_zone: Europe/London
calendar: GBLO
start: 2026-05-01
rules:
  - { id: payment, rule: YY-MM-L~MFT12:00:00, basis: true }
  - { id: report, rule: '{payment}~2PT09:00:00' }
";
    let doc = yaml(text);
    // 31 May 2026 is a Sunday: paid Friday 29 May, reported Wednesday 27 May.
    assert_eq!(times(&doc.schedule(&Places).unwrap(), 2), ["Wed 27 May 09:00", "Fri 26 Jun 09:00"]);
    // `basis` is written only when set.
    let json = serde_json::to_string(&doc).unwrap();
    assert!(json.contains(r#""basis":true"#) && json.matches("basis").count() == 1, "{json}");
    assert_eq!(serde_json::from_str::<ScheduleDoc>(&json).unwrap(), doc);
    // A basis needs an id; a derived rule's adjustment needs a calendar.
    assert!(invalid(&text.replace("id: payment, ", "")).contains("needs an `id`"));
    assert!(invalid(&text.replace("calendar: GBLO\n", "").replace("~MF", "~W")).contains("`{payment}~2PT09:00:00` uses business days"));
    assert!(build(&text.replace("calendar: GBLO\n", "").replace("~MF", "~W").replace("~2P", "~PFRI")).is_ok());
}

#[test]
fn derived_rules_keep_to_the_calendars_date() {
    // 02:00 London is the evening before in New York, whose business days
    // these are: refused, as for any rule.
    let text = "
version: 1
time_zone: Europe/London
calendar: USNY
start: 2026-05-01
rules:
  - { id: payment, rule: 'YY-MM-L~MFT12:00:00', basis: true }
  - { id: report, rule: '{payment}~2PT02:00:00' }
";
    let schedule = build(text).unwrap();
    let err = schedule.slots().unwrap().next().unwrap_err();
    assert!(err.to_string().contains("America/New_York"), "{err}");
}
