//! `docs/loan-lifecycle.md`: every YAML block on the page is built here on
//! the built-in `GBLO` calendar, and every date the page quotes is checked.

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone};
use chrono_tz::Europe::London;
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;
use tkone_calendar::{Calendar, Calendars};
use tkone_schedule::schedule::document::{BusinessDays, ScheduleDoc};
use tkone_schedule::schedule::{Bundle, Next, Schedule, Selector, Timeline};

const PAGE: &str = include_str!("../../../docs/loan-lifecycle.md");

type Bdp = BusinessDays<Calendar>;

/// The page's YAML blocks, by the label on their first line
/// (`# definition: servicing`, `# instance: L-1001/drawdown (definition: servicing)`).
fn blocks() -> BTreeMap<String, String> {
    let mut blocks = BTreeMap::new();
    for block in PAGE.split("```yaml\n").skip(1) {
        let yaml = block.split("```").next().unwrap();
        let label = yaml.lines().next().unwrap().trim_start_matches("# ").to_string();
        assert!(blocks.insert(label.clone(), yaml.to_string()).is_none(), "{label} twice");
    }
    blocks
}

fn doc(yaml: &str) -> ScheduleDoc {
    serde_yaml_ng::from_str(yaml).unwrap()
}

fn london(y: i32, m: u32, d: u32, h: u32) -> DateTime<Tz> {
    London.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
}

fn fmt(t: &DateTime<Tz>) -> String {
    t.format("%a %d %b %Y %H:%M").to_string()
}

struct Loan {
    definitions: BTreeMap<String, Schedule<Bdp>>,
    blocks: BTreeMap<String, String>,
}

impl Loan {
    fn new() -> Self {
        let blocks = blocks();
        let definitions = blocks
            .iter()
            .filter_map(|(label, yaml)| label.strip_prefix("definition: ").map(|name| (name.to_string(), yaml)))
            .map(|(name, yaml)| (name, doc(yaml).schedule(Calendars::builtin()).unwrap()))
            .collect();
        Self { definitions, blocks }
    }

    /// The instance labelled `key` (`L-1001/collections`, or with its
    /// version, `L-1001/collections, version 2`), with its definition, after
    /// replacing `until` when `until` is given. It is built in the loan's
    /// bundle, with the other instances as the page gives them: the
    /// reminders refer to the collections.
    fn instance(&self, key: &str, until: Option<&str>) -> (Selector<Bdp>, &Schedule<Bdp>) {
        let (bundle, name, definition) = self.bundle(key, until);
        (bundle.instance(&name).unwrap().0.clone(), definition)
    }

    /// The loan's bundle, with `key` as `instance` takes it; and the name
    /// and definition of `key`'s instance in it.
    fn bundle(&self, key: &str, until: Option<&str>) -> (Bundle<Bdp>, String, &Schedule<Bdp>) {
        let base = key.split(", version").next().unwrap();
        let mut bundle = Bundle::builder();
        let mut wanted = None;
        for (label, yaml) in &self.blocks {
            let Some(rest) = label.strip_prefix("instance: ") else { continue };
            let (this, definition) = rest.split_once(" (definition: ").unwrap();
            let definition = &self.definitions[definition.trim_end_matches(')')];
            let this_base = this.split(", version").next().unwrap();
            // Version 1 of every instance, except the one asked for.
            let chosen = if this_base == base { this == key } else { !this.contains(", version") };
            if !chosen {
                continue;
            }
            let yaml = match until {
                Some(until) if this == key => yaml
                    .lines()
                    .map(|l| if l.starts_with("until:") { format!("until: {until}") } else { l.to_string() })
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => yaml.clone(),
            };
            let name = this_base.rsplit('/').next().unwrap();
            if this == key {
                wanted = Some((name.to_string(), definition));
            }
            bundle = bundle.instance(name, definition, doc(&yaml).selector_builder(definition).unwrap());
        }
        let (name, definition) = wanted.unwrap_or_else(|| panic!("no instance {key}"));
        (bundle.build().unwrap(), name, definition)
    }

    /// Every run of an instance, checked to be its definition's.
    fn runs(&self, key: &str) -> Vec<String> {
        let (selector, definition) = self.instance(key, None);
        let end = london(2031, 12, 31, 0);
        assert!(selector.uncovered(definition, *selector.start()..=end).unwrap().is_empty(), "{key}");
        assert!(selector.schedule().moved_before_start().unwrap().is_empty(), "{key}");
        selector.schedule().slots().unwrap().map(|s| Ok(fmt(&s.at))).collect().unwrap()
    }
}

fn next(selector: &Selector<Bdp>, definition: &Schedule<Bdp>, after: DateTime<Tz>) -> Option<String> {
    match selector
        .next_after(definition, &after, &(after + chrono::Duration::days(2000)), |m| panic!("missed {m:?}"))
        .unwrap()
    {
        Next::Due(slot) => Some(fmt(&slot.at)),
        Next::Done => None,
        Next::NotFound => panic!("not found"),
    }
}

#[test]
fn every_block_is_read() {
    let loan = Loan::new();
    assert_eq!(loan.definitions.len(), 4);
    assert_eq!(loan.blocks.len(), 4 + 7 + 2);
}

#[test]
fn the_instances() {
    let loan = Loan::new();
    let check = |key: &str, count: usize, first: &[&str], last: &str| {
        let runs = loan.runs(key);
        assert_eq!(runs.len(), count, "{key}");
        assert_eq!(&runs[..first.len()], first, "{key}");
        assert_eq!(runs.last().unwrap(), last, "{key}");
    };
    check("L-1001/drawdown", 1, &["Thu 15 Jan 2026 12:00"], "Thu 15 Jan 2026 12:00");
    check("L-1001/accrual", 1265, &["Thu 15 Jan 2026 18:00"], "Wed 15 Jan 2031 18:00");
    check(
        "L-1001/collections",
        60,
        &["Mon 16 Feb 2026 12:00", "Mon 16 Mar 2026 12:00", "Wed 15 Apr 2026 12:00"],
        "Wed 15 Jan 2031 12:00",
    );
    check(
        "L-1001/fixings",
        60,
        &["Tue 13 Jan 2026 11:00", "Thu 12 Feb 2026 11:00", "Thu 12 Mar 2026 11:00", "Mon 13 Apr 2026 11:00"],
        "Thu 12 Dec 2030 11:00",
    );
    check(
        "L-1001/reminders",
        60,
        &["Thu 12 Feb 2026 09:00", "Thu 12 Mar 2026 09:00", "Mon 13 Apr 2026 09:00"],
        "Mon 13 Jan 2031 09:00",
    );
    check(
        "L-1001/statements",
        61,
        &["Mon 02 Feb 2026 09:00", "Mon 02 Mar 2026 09:00", "Wed 01 Apr 2026 09:00"],
        "Mon 03 Feb 2031 09:00",
    );
    check("L-1001/closure", 1, &["Wed 22 Jan 2031 12:00"], "Wed 22 Jan 2031 12:00");
}

#[test]
fn fixings_and_reminders_match_the_adjusted_repayments() {
    // `~2P` from the 15th: the same days as two business days before each
    // adjusted repayment (and the drawdown, for fixings).
    use tkone_schedule::biz_day::BizDayProcessor;
    let loan = Loan::new();
    let gblo = Calendars::builtin().get("GBLO").unwrap();
    let back = |t: &str, h: &str| {
        let t = chrono::NaiveDateTime::parse_from_str(t, "%a %d %b %Y %H:%M").unwrap();
        format!("{} {h}", gblo.sub(&t, 2).unwrap().format("%a %d %b %Y"))
    };
    let collections = loan.runs("L-1001/collections");
    let mut starts = vec!["Thu 15 Jan 2026 12:00".to_string()];
    starts.extend(collections[..59].iter().cloned());
    assert_eq!(loan.runs("L-1001/fixings"), starts.iter().map(|t| back(t, "11:00")).collect::<Vec<_>>());
    assert_eq!(loan.runs("L-1001/reminders"), collections.iter().map(|t| back(t, "09:00")).collect::<Vec<_>>());
}

#[test]
fn derived_runs_name_their_basis() {
    let loan = Loan::new();
    let first = |key: &str| loan.instance(key, None).0.schedule().slots().unwrap().next().unwrap().unwrap();
    let fixing = first("L-1001/fixings");
    assert_eq!((fixing.at, fixing.sources[0].basis), (london(2026, 1, 13, 11), Some(london(2026, 1, 15, 12))));
    let reminder = first("L-1001/reminders");
    assert_eq!((reminder.at, reminder.sources[0].basis), (london(2026, 2, 12, 9), Some(london(2026, 2, 16, 12))));
}

#[test]
fn the_first_five_weeks() {
    let loan = Loan::new();
    let keys = ["drawdown", "accrual", "collections", "fixings", "reminders", "statements", "closure"];
    let timeline = keys.iter().fold(Timeline::new(), |t, key| {
        t.add(*key, loan.instance(&format!("L-1001/{key}"), None).0.schedule().clone())
    });
    let moments: Vec<String> = timeline
        .between(london(2026, 1, 1, 0)..=london(2026, 2, 16, 23))
        .unwrap()
        .map(|m| Ok(format!("{} {}", fmt(&m.at), m.entries.iter().map(|e| e.key).collect::<Vec<_>>().join("+"))))
        .collect()
        .unwrap();
    // The page lists these, eliding most accruals.
    let listed = [
        "Tue 13 Jan 2026 11:00 fixings",
        "Thu 15 Jan 2026 12:00 drawdown",
        "Thu 15 Jan 2026 18:00 accrual",
        "Fri 16 Jan 2026 18:00 accrual",
        "Mon 02 Feb 2026 09:00 statements",
        "Thu 12 Feb 2026 09:00 reminders",
        "Thu 12 Feb 2026 11:00 fixings",
        "Thu 12 Feb 2026 18:00 accrual",
        "Fri 13 Feb 2026 18:00 accrual",
        "Mon 16 Feb 2026 12:00 collections",
        "Mon 16 Feb 2026 18:00 accrual",
    ];
    let mut at = 0;
    for line in listed {
        at += moments[at..].iter().position(|m| m == line).unwrap_or_else(|| panic!("{line} not in order"));
    }
    // And what it elides is only accrual.
    let others: Vec<_> = moments.iter().filter(|m| !m.ends_with(" accrual")).collect();
    assert_eq!(others.len(), 6);
}

#[test]
fn early_repayment() {
    let loan = Loan::new();
    let morning = london(2028, 3, 6, 8);
    // (instance, new until, stored next due in version 1, version 2's next due)
    let cases = [
        ("L-1001/accrual", Some("2028-03-10"), "Mon 06 Mar 2028 18:00", Some("Mon 06 Mar 2028 18:00")),
        ("L-1001/fixings", Some("2028-03-09"), "Mon 13 Mar 2028 11:00", None),
        ("L-1001/reminders", Some("2028-03-09"), "Mon 13 Mar 2028 09:00", None),
        ("L-1001/statements", Some("2028-04-03"), "Mon 03 Apr 2028 09:00", Some("Mon 03 Apr 2028 09:00")),
    ];
    for (key, until, stored, then) in cases {
        let (v1, definition) = loan.instance(key, None);
        assert_eq!(next(&v1, definition, morning).as_deref(), Some(stored), "{key} v1");
        let (v2, definition) = loan.instance(key, until);
        assert_eq!(next(&v2, definition, morning).as_deref(), then, "{key} v2");
    }
    // Accrual ends on the 10th; statements with the April one.
    let (accrual, definition) = loan.instance("L-1001/accrual", Some("2028-03-10"));
    assert_eq!(next(&accrual, definition, london(2028, 3, 10, 12)).as_deref(), Some("Fri 10 Mar 2028 18:00"));
    assert_eq!(next(&accrual, definition, london(2028, 3, 10, 18)), None);

    // Collections: version 1 holds 15 March; version 2 repays on the 10th.
    let (v1, definition) = loan.instance("L-1001/collections", None);
    assert_eq!(next(&v1, definition, morning).as_deref(), Some("Wed 15 Mar 2028 12:00"));
    let (v2, definition) = loan.instance("L-1001/collections, version 2", None);
    assert_eq!(next(&v2, definition, morning).as_deref(), Some("Fri 10 Mar 2028 12:00"));
    assert_eq!(next(&v2, definition, london(2028, 3, 10, 12)), None);
    let all: Vec<_> = v2.schedule().slots().unwrap().collect().unwrap();
    assert_eq!(all.len(), 26);
    assert_eq!(all.iter().filter(|s| s.sources[0].rule_id.as_deref() == Some("repayment")).count(), 25);

    // The reminders follow the collections: with version 2 of the
    // collections, and no change of their own, nothing is left to remind of.
    let (set, _, _) = loan.bundle("L-1001/collections, version 2", None);
    let (reminders, notices) = set.instance("reminders").unwrap();
    assert_eq!(next(reminders, notices, morning), None);
    assert_eq!(next(reminders, notices, london(2028, 2, 1, 0)).as_deref(), Some("Fri 11 Feb 2028 09:00"));

    // Closure: 22 January 2031 becomes 17 March 2028.
    let (v1, definition) = loan.instance("L-1001/closure", None);
    assert_eq!(next(&v1, definition, morning).as_deref(), Some("Wed 22 Jan 2031 12:00"));
    let (v2, definition) = loan.instance("L-1001/closure, version 2", None);
    assert_eq!(next(&v2, definition, morning).as_deref(), Some("Fri 17 Mar 2028 12:00"));
}
