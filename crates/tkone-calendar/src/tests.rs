use chrono::{NaiveDate, NaiveDateTime};
use fallible_iterator::FallibleIterator;
use tkone_schedule::biz_day::{BizDayProcessor, Direction};

use crate::model::CalendarFile;
use crate::{Calendar, Calendars, Error, Status};

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn at(y: i32, m: u32, d: u32) -> NaiveDateTime {
    ymd(y, m, d).and_hms_opt(9, 30, 0).unwrap()
}

/// A calendar file from the fields after the header, as JSON.
fn file(substitute: &str, holidays: &str) -> CalendarFile {
    let json = format!(
        r#"{{"code": "TEST", "name": "Test", "kind": "public", "zone": "Europe/London",
            "weekend": ["SAT", "SUN"], "valid_from": "2020-01-01", "valid_until": "2030-12-31",
            "confirmed_until": "2025-12-31", "substitute": "{substitute}", "holidays": [{holidays}]}}"#
    );
    serde_json::from_str(&json).unwrap()
}

fn days_off(cal: &Calendar, year: i32) -> Vec<String> {
    cal.holidays(year).map(|h| format!("{} {}", h.observed.format("%m-%d %a"), h.name)).collect()
}

fn problems(f: &CalendarFile) -> Vec<String> {
    match Calendar::from_file(f) {
        Err(Error::Invalid { problems, .. }) => problems,
        other => panic!("expected problems, got {other:?}"),
    }
}

const XMAS: &str = r#"{"name": "Christmas", "rule": "YY-12-25"}, {"name": "Boxing Day", "rule": "YY-12-26"}"#;

#[test]
fn test_builtin() {
    let codes: Vec<&str> = Calendars::builtin().codes().collect();
    assert_eq!(codes, ["AUSY", "CATO", "EUTA", "GBLO", "NZAU", "NZWE", "USNY", "XNYS", "ZAJO"]);
    let london = Calendars::builtin().get("GBLO").unwrap();
    assert_eq!(london.zone(), Some("Europe/London"));
    assert_eq!(
        days_off(&london, 2022),
        [
            "01-03 Mon New Year's Day",
            "04-15 Fri Good Friday",
            "04-18 Mon Easter Monday",
            "05-02 Mon Early May bank holiday",
            "06-02 Thu Spring bank holiday",
            "06-03 Fri Platinum Jubilee of Queen Elizabeth II",
            "08-29 Mon Summer bank holiday",
            "09-19 Mon State Funeral of Queen Elizabeth II",
            "12-26 Mon Boxing Day",
            "12-27 Tue Christmas Day",
        ]
    );
    // Christmas Day keeps its actual date.
    let xmas = london.holidays(2022).find(|h| h.name == "Christmas Day").unwrap();
    assert_eq!((xmas.actual, xmas.observed), (ymd(2022, 12, 25), ymd(2022, 12, 27)));
    assert!(matches!(Calendars::builtin().get("XXXX"), Err(Error::Unknown(c)) if c == "XXXX"));
}

#[test]
fn test_substitute_policies() {
    // 2021: Christmas on Saturday, Boxing Day on Sunday.
    // 2022: Christmas on Sunday, Boxing Day on Monday.
    let run = |policy| {
        let cal = Calendar::from_file(&file(policy, XMAS)).unwrap();
        [2021, 2022].map(|y| cal.holidays(y).map(|h| h.observed.format("%d %a").to_string()).collect::<Vec<_>>().join(", "))
    };
    assert_eq!(run("next-free-weekday"), ["27 Mon, 28 Tue", "26 Mon, 27 Tue"]);
    assert_eq!(run("next-weekday"), ["27 Mon, 27 Mon", "26 Mon, 26 Mon"]);
    assert_eq!(run("nearest-weekday"), ["24 Fri, 27 Mon", "26 Mon, 26 Mon"]);
    assert_eq!(run("sunday-to-monday"), ["25 Sat, 27 Mon", "26 Mon, 26 Mon"]);
    assert_eq!(run("none"), ["25 Sat, 26 Sun", "25 Sun, 26 Mon"]);
    // A holiday can override the calendar's policy.
    let f = file("next-free-weekday", r#"{"name": "Christmas", "rule": "YY-12-25", "substitute": "none"}"#);
    assert_eq!(days_off(&Calendar::from_file(&f).unwrap(), 2022), ["12-25 Sun Christmas"]);
}

#[test]
fn test_rules_dates_and_exceptions() {
    let f = file(
        "none",
        r#"{"name": "May Day", "rule": "YY-05-MON#1", "except": ["2020-05-04"], "source": "moved"},
           {"name": "May Day", "dates": ["2020-05-08"], "source": "moved"},
           {"name": "Juneteenth", "rule": "YY-06-19~NW", "from": 2022, "until": 2023},
           {"name": "Victoria Day", "rule": "YY-05-24~PMON"}"#,
    );
    let cal = Calendar::from_file(&f).unwrap();
    assert_eq!(days_off(&cal, 2020), ["05-08 Fri May Day", "05-18 Mon Victoria Day"]);
    assert_eq!(
        days_off(&cal, 2022),
        ["05-02 Mon May Day", "05-23 Mon Victoria Day", "06-20 Mon Juneteenth"]
    );
    assert_eq!(cal.holidays(2024).count(), 2);
}

#[test]
fn test_checks() {
    let check = |holidays: &str| problems(&file("none", holidays));
    let one = |holidays: &str| {
        let p = check(holidays);
        assert_eq!(p.len(), 1, "{p:?}");
        p[0].clone()
    };
    assert!(one(r#"{"name": "A", "rule": "YY-01-01", "dates": ["2021-01-01"], "source": "s"}"#).contains("either"));
    assert!(one(r#"{"name": "A", "rule": "YY-01-01~NB"}"#).contains("uses business days"));
    assert!(one(r#"{"name": "A", "rule": "YY-01-BD#1"}"#).contains("uses business days"));
    assert!(one(r#"{"name": "A", "rule": "YY-01-01 @2021-01-01"}"#).contains("invalid date spec"));
    assert!(one(r#"{"name": "A", "rule": "YY-01-01 | YY-12-31"}"#).contains("invalid date spec"));
    assert!(one(r#"{"name": "A", "rule": "YY-MM-DD"}"#).contains("more than"));
    assert!(one(r#"{"name": "A", "rule": "YY-02-30"}"#).contains("YY-02-30"));
    assert!(one(r#"{"name": "A", "rule": "YY-01-01", "except": ["2021-01-02"], "source": "s"}"#).contains("not a date the rule gives"));
    assert!(one(r#"{"name": "A", "rule": "YY-01-01", "except": ["2021-01-01"]}"#).contains("needs a `source`"));
    assert!(one(r#"{"name": "A", "rule": "YY-01-01", "from": 2040}"#).contains("no year"));
    assert!(one(r#"{"name": "A", "dates": ["2021-01-01"]}"#).contains("needs a `source`"));
    assert!(one(r#"{"name": "A", "dates": ["2021-01-02"], "source": "s"}"#).contains("weekend"));
    assert!(one(r#"{"name": "A", "dates": ["2041-01-01"], "source": "s"}"#).contains("outside"));
    assert!(check(r#"{"name": "A", "dates": ["2021-01-01", "2021-01-01"], "source": "s"}"#).iter().any(|p| p.contains("listed twice")));
    assert!(one(r#"{"name": "A", "dates": {"2021": "2021-01-01", "2023": "2023-01-02"}, "source": "s"}"#).contains("skip"));
    assert!(one(r#"{"name": "A", "dates": {"2021": "2022-01-03"}, "source": "s"}"#).contains("listed under 2021"));
    // The Mauritius mistake: a date both cancelled and listed.
    assert!(check(
        r#"{"name": "A", "rule": "YY-02-01", "except": ["2021-02-01"], "source": "s"},
           {"name": "A", "dates": ["2021-02-01"], "source": "s"}"#
    )
    .iter()
    .any(|p| p.contains("both excepted and listed")));
    // Two entries giving the same date.
    assert!(check(r#"{"name": "A", "rule": "YY-01-01"}, {"name": "A", "rule": "YY-01-01~NW"}"#)
        .iter()
        .any(|p| p.contains("2021-01-01 appears twice")));

    // The header.
    let mut f = file("none", XMAS);
    f.zone = "Mars/Base".into();
    f.weekend = vec!["SAT".into(), "SAT".into(), "FUN".into()];
    f.code = "gb+lo".into();
    assert_eq!(problems(&f).len(), 4, "{:?}", problems(&f));
    let mut f = file("none", XMAS);
    f.confirmed_until = "2031-01-01".into();
    assert!(problems(&f)[0].contains("≤"));
}

#[test]
fn test_range_and_status() {
    let cal = Calendar::from_file(&file("none", XMAS)).unwrap();
    assert_eq!(
        cal.is_business_day(ymd(2031, 1, 2)),
        Err(Error::OutOfRange { code: "TEST".into(), date: ymd(2031, 1, 2), from: ymd(2020, 1, 1), until: ymd(2030, 12, 31) })
    );
    assert_eq!(cal.status(ymd(2025, 12, 31)), Ok(Status::Confirmed));
    assert_eq!(cal.status(ymd(2026, 1, 1)), Ok(Status::Projected));
    // Weekends are known for any date.
    assert!(cal.is_weekend(ymd(2040, 1, 7)));
}

#[test]
fn test_biz_day_processor() {
    let london = Calendars::builtin().get("GBLO").unwrap();
    // Easter 2025: Good Friday 18 April, Easter Monday 21 April.
    assert_eq!(london.add(&at(2025, 4, 17), 1).unwrap(), at(2025, 4, 22));
    assert_eq!(london.sub(&at(2025, 4, 22), 2).unwrap(), at(2025, 4, 16));
    assert_eq!(london.find_biz_day(&at(2025, 4, 18), Direction::Next).unwrap(), at(2025, 4, 22));
    assert_eq!(london.find_biz_day(&at(2025, 4, 21), Direction::Prev).unwrap(), at(2025, 4, 17));
    // Nearest: Good Friday is 1 day from Thursday, 4 from Tuesday.
    assert_eq!(london.find_biz_day(&at(2025, 4, 18), Direction::Nearest).unwrap(), at(2025, 4, 17));
    assert_eq!(london.find_biz_day(&at(2025, 4, 21), Direction::Nearest).unwrap(), at(2025, 4, 22));
    // Errors outside the range reach the caller.
    assert!(london.add(&at(2099, 12, 31), 1).is_err());
}

#[test]
fn test_schedules_on_a_calendar() {
    use chrono::{TimeZone, Utc};
    use tkone_schedule::date::SpecIteratorBuilder;
    let run = |spec: &str, code: &str, n: usize| -> Vec<String> {
        let cal = Calendars::builtin().get(code).unwrap();
        SpecIteratorBuilder::new_after(spec, cal, Utc.with_ymd_and_hms(2025, 3, 31, 0, 0, 0).unwrap())
            .build()
            .unwrap()
            .take(n)
            .map(|o| Ok(o.observed().format("%m-%d").to_string()))
            .collect()
            .unwrap()
    };
    // 3rd business day of the month: April 2025 has Good Friday and Easter
    // Monday in London, May has its bank holiday on the 5th.
    assert_eq!(run("YY-MM-BD#3", "GBLO", 2), ["04-03", "05-06"]);
    // Every 3rd business day across Easter, from Monday 14 April.
    let london = Calendars::builtin().get("GBLO").unwrap();
    let steps: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-3BD", london, Utc.with_ymd_and_hms(2025, 4, 14, 0, 0, 0).unwrap())
        .build()
        .unwrap()
        .take(3)
        .map(|o| Ok(o.observed().format("%m-%d").to_string()))
        .collect()
        .unwrap();
    assert_eq!(steps, ["04-14", "04-17", "04-24"]);
    // Modified following on the 5th of May: a London holiday, not a TARGET one.
    assert_eq!(run("YY-05-05~MF", "GBLO", 1), ["05-06"]);
    assert_eq!(run("YY-05-05~MF", "EUTA", 1), ["05-05"]);
    assert_eq!(run("YY-05-05~MF", "GBLO+EUTA", 1), ["05-06"]);
}

#[test]
fn test_joint() {
    let cals = Calendars::builtin();
    let joint = cals.get("GBLO+EUTA").unwrap();
    assert_eq!(joint.code(), "GBLO+EUTA");
    assert_eq!(joint.zone(), None);
    // TARGET2 starts in 1999.
    assert_eq!(joint.valid_from(), ymd(1999, 1, 1));
    for (date, business) in [
        (ymd(2025, 5, 1), false),  // Labour Day (TARGET2)
        (ymd(2025, 5, 5), false),  // Early May bank holiday (London)
        (ymd(2025, 5, 6), true),
        (ymd(2025, 12, 26), false), // both
    ] {
        assert_eq!(joint.is_business_day(date).unwrap(), business, "{date}");
    }
    // A holiday both have appears once.
    assert_eq!(joint.holidays(2025).filter(|h| h.observed == ymd(2025, 12, 25)).count(), 1);
    assert_eq!(
        joint.holidays(2025).filter(|h| h.observed == ymd(2025, 4, 18)).map(|h| h.name.as_str()).collect::<Vec<_>>(),
        ["Good Friday"]
    );
}

#[test]
fn test_calendar_named_in_a_document() {
    use tkone_schedule::schedule::document::ScheduleDoc;
    let cals = Calendars::builtin();
    let first = |calendar: &str, rule: &str| -> String {
        let doc = format!(
            r#"{{"version": 1, "time_zone": "Europe/London", "calendar": "{calendar}", "start": "2025-01-01", "rules": [{{"rule": "{rule}"}}]}}"#
        );
        let doc: ScheduleDoc = serde_json::from_str(&doc).unwrap();
        doc.schedule(cals).unwrap().slots().unwrap().next().unwrap().unwrap().at.format("%Y-%m-%d").to_string()
    };
    // 5 May 2025 is a London bank holiday; 1 May is TARGET's Labour Day.
    assert_eq!(first("GBLO", "YY-05-05~MFT09:00:00"), "2025-05-06");
    assert_eq!(first("EUTA", "YY-05-05~MFT09:00:00"), "2025-05-05");
    assert_eq!(first("GBLO+EUTA", "YY-05-BD#1T09:00:00"), "2025-05-02");
    assert_eq!(first("EUTA+GBLO", "YY-05-BD#1T09:00:00"), "2025-05-02");
}

#[test]
fn test_datetime_dates_checked_in_the_calendars_zone() {
    use chrono::TimeZone;
    use tkone_schedule::schedule::Schedule;
    let cals = Calendars::builtin();
    let run = |rule: &str, start: chrono::DateTime<chrono_tz::Tz>, until: chrono::DateTime<chrono_tz::Tz>| -> tkone_schedule::Result<Vec<String>> {
        Schedule::builder(start, cals.get("XNYS").unwrap())
            .rule(rule)
            .until(until)
            .build()?
            .slots()?
            .map(|s| Ok(s.at.format("%a %m-%d %H:%M").to_string()))
            .collect()
    };
    let london = |d, h| chrono_tz::Europe::London.with_ymd_and_hms(2026, 11, d, h, 0, 0).unwrap();
    let utc = |d, h| chrono_tz::UTC.with_ymd_and_hms(2026, 11, d, h, 0, 0).unwrap();
    let new_york = |d, h| chrono_tz::America::New_York.with_ymd_and_hms(2026, 11, d, h, 0, 0).unwrap();
    // 21:30 in London is 16:30 the same day in New York: fine.
    assert_eq!(run("YY-MM-DD~SBT21:30:00", london(25, 0), london(27, 23)).unwrap(), ["Wed 11-25 21:30", "Fri 11-27 21:30"]);
    // 02:00 UTC is the evening before in New York: refused, not run on the
    // wrong day. (Without the check, Fri 27 Nov 02:00 UTC would pass as a
    // trading day though it is Thanksgiving evening in New York.)
    let err = run("YY-MM-DD~SBT02:00:00", utc(25, 0), utc(27, 23)).unwrap_err();
    assert!(matches!(err, tkone_schedule::Error::Calendar(ref m) if m.contains("America/New_York")), "{err}");
    // In the calendar's zone, it runs.
    assert_eq!(
        run("YY-MM-DD~SBT21:00:00", new_york(24, 0), new_york(27, 23)).unwrap(),
        ["Tue 11-24 21:00", "Wed 11-25 21:00", "Fri 11-27 21:00"]
    );
    // A joint calendar across zones has no single zone, so no check.
    assert_eq!(cals.get("GBLO+USNY").unwrap().zone(), None);
    // Date schedules are dates: not checked.
    let dates: Vec<_> = tkone_schedule::date::SpecIteratorBuilder::new_after("YY-MM-BD#1", cals.get("XNYS").unwrap(), chrono_tz::UTC.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap())
        .build()
        .unwrap()
        .take(1)
        .map(|o| Ok(o.observed().date_naive()))
        .collect()
        .unwrap();
    assert_eq!(dates, [ymd(2026, 1, 2)]);
}

/// The embedded data is `data/` as it is now.
#[cfg(feature = "yaml")]
#[test]
fn test_compiled_data_is_up_to_date() {
    let files = crate::read_dir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))).unwrap();
    let json = serde_json::to_string_pretty(&files).unwrap() + "\n";
    assert!(
        json == include_str!("../compiled/calendars.json"),
        "compiled/calendars.json is out of date: cargo run -p tkone-calendar --features yaml --bin tkone-calendar-compile"
    );
}

// ---------------------------------------------------------------------------
// Schedule documents
// ---------------------------------------------------------------------------

fn document(calendar: &str, start: &str, rule: &str) -> tkone_schedule::schedule::Schedule<impl BizDayProcessor> {
    let json = format!(
        r#"{{"version": 1, "time_zone": "Europe/London", "calendar": "{calendar}", "start": "{start}", "rules": [{{"rule": "{rule}"}}]}}"#
    );
    let doc: tkone_schedule::schedule::document::ScheduleDoc = serde_json::from_str(&json).unwrap();
    doc.schedule(Calendars::builtin()).unwrap()
}

fn slot_days(schedule: &tkone_schedule::schedule::Schedule<impl BizDayProcessor>, n: usize) -> Vec<String> {
    schedule.slots().unwrap().take(n).map(|s| Ok(s.at.format("%a %d %b").to_string())).collect().unwrap()
}

#[test]
fn test_document_calendars() {
    // Spring bank holiday, Monday 25 May 2026; 31 May is a Sunday.
    assert_eq!(slot_days(&document("GBLO", "2026-05-22", "YY-MM-DD~SBT18:00:00"), 3), ["Fri 22 May", "Tue 26 May", "Wed 27 May"]);
    assert_eq!(slot_days(&document("GBLO", "2026-05-01", "YY-MM-L~PBT18:00:00"), 2), ["Fri 29 May", "Tue 30 Jun"]);
    // Independence Day 2026 is a Saturday. The exchange closes the Friday
    // before; the Federal Reserve stays open, so dollar payments settle.
    assert_eq!(slot_days(&document("GBLO+XNYS", "2026-07-01", "YY-MM-DD~SBT12:00:00"), 3), ["Wed 01 Jul", "Thu 02 Jul", "Mon 06 Jul"]);
    assert_eq!(slot_days(&document("GBLO+USNY", "2026-07-01", "YY-MM-DD~SBT12:00:00"), 3), ["Wed 01 Jul", "Thu 02 Jul", "Fri 03 Jul"]);
}

#[test]
fn test_document_unknown_calendar() {
    let doc: tkone_schedule::schedule::document::ScheduleDoc = serde_json::from_str(
        r#"{"version": 1, "time_zone": "Europe/London", "calendar": "GBLO+XXXX", "start": "2026-01-01", "rules": [{"rule": "YY-MM-DD~SBT09:00:00"}]}"#,
    )
    .unwrap();
    let err = doc.schedule(Calendars::builtin()).unwrap_err();
    assert!(matches!(&err, tkone_schedule::Error::Calendar(why) if why.contains("XXXX")), "{err:?}");
}
