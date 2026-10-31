//! Keeps GRAMMAR.md honest: every schedule in its conformance tables and
//! validity-rule tables is built, and must be accepted or rejected as stated.

use crate::biz_day::WeekendSkipper;
use crate::{date, datetime, time};
use chrono::{TimeZone, Utc};

const GRAMMAR: &str = include_str!("../GRAMMAR.md");

/// Whether `spec` builds as a schedule of `kind` ("date", "time", "datetime").
fn builds(spec: &str, kind: &str) -> bool {
    let now = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let bdp = WeekendSkipper::new();
    match kind {
        "date" => date::SpecIteratorBuilder::new_after(spec, bdp, now).build().is_ok(),
        "time" => time::SpecIteratorBuilder::new_after(spec, now).build().is_ok(),
        "datetime" => datetime::SpecIteratorBuilder::new_after(spec, bdp, now).build().is_ok(),
        other => panic!("unknown kind {other}"),
    }
}

/// The text of the section headed `heading`, up to the next heading of the same level.
fn section<'a>(heading: &str) -> &'a str {
    let level = heading.chars().take_while(|c| *c == '#').count();
    // Match the whole heading line, so `### Bounds` is not `### Bounds: start, …`.
    let line = format!("\n{heading}\n");
    let start = GRAMMAR.find(&line).unwrap_or_else(|| panic!("no section {heading}")) + line.len();
    let rest = &GRAMMAR[start..];
    let end = rest
        .match_indices('\n')
        .map(|(i, _)| i + 1)
        .find(|&i| {
            let hashes = rest[i..].chars().take_while(|c| *c == '#').count();
            (1..=level).contains(&hashes) && rest[i + hashes..].starts_with(' ')
        })
        .unwrap_or(rest.len());
    &rest[..end]
}

/// The table rows (cells) of a section, without the header and separator rows.
fn rows(text: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let cells_of = |line: &str| -> Vec<String> {
        // Split on `|` that is not escaped as `\|`.
        let mut cells = vec![String::new()];
        let mut chars = line.trim().trim_matches('|').chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' if chars.peek() == Some(&'|') => {
                    cells.last_mut().unwrap().push('|');
                    chars.next();
                }
                '|' => cells.push(String::new()),
                c => cells.last_mut().unwrap().push(c),
            }
        }
        cells.into_iter().map(|c| c.trim().to_string()).collect()
    };
    for line in text.lines().filter(|l| l.trim_start().starts_with('|')) {
        out.push(cells_of(line));
    }
    // Drop the header and the `|---|` separator.
    out.into_iter().skip(2).collect()
}

/// The `code` spans in a table cell.
fn specs(cell: &str) -> Vec<String> {
    cell.split('`').skip(1).step_by(2).map(str::to_string).collect()
}

#[test]
fn test_grammar_accepted_examples() {
    let accepted = rows(section("### Accepted"));
    assert!(!accepted.is_empty());
    for row in accepted {
        let kind = row[1].as_str();
        for spec in specs(&row[0]) {
            assert!(builds(&spec, kind), "GRAMMAR.md lists `{spec}` as an accepted {kind} schedule");
        }
    }
}

#[test]
fn test_grammar_rejected_examples() {
    let mut checked = 0;
    let mut check = |spec: &str| {
        assert!(
            ["date", "time", "datetime"].iter().all(|kind| !builds(spec, kind)),
            "GRAMMAR.md lists `{spec}` as rejected, but it builds"
        );
        checked += 1;
    };
    for row in rows(section("### Rejected")) {
        for spec in specs(&row[0]) {
            check(&spec);
        }
    }
    // The examples in the validity-rule tables, except elided ones (`…`).
    for heading in ["### Date specs", "### Time specs", "### Exclusions", "### Bounds"] {
        for row in rows(section(heading)) {
            for spec in specs(&row[1]).into_iter().filter(|s| !s.contains('…')) {
                check(&spec);
            }
        }
    }
    assert!(checked > 40, "only {checked} rejected examples found");
}
