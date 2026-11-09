# tkone-schedule

New to the schedule format? Read the [introduction](INTRODUCTION.md). It explains why the format
exists, how it compares with cron, Quartz and iCalendar RRULE, and walks through the spec with
real-world examples. This README covers using the library.

## Add to your project

```toml
[dependencies]
tkone-schedule    = { path = "crates/tkone-schedule" }
chrono            = "0.4"
chrono-tz         = "0.10"  # to name time zones, e.g. chrono_tz::Europe::London
fallible-iterator = "0.3"   # the iterators are fallible iterators
```

<!-- cargo-rdme start -->

A scheduling and recurrence library built on flexible mini-language specs for dates,
times, and combined datetimes. Supports business day processing, timezone awareness,
and fallible iteration.

It is made for operations: payment runs, batch jobs, reports and deadlines
that must land on the right business day. It does not define financial
instruments (calculation periods, stubs, fixing offsets, day counts); see
[INTRODUCTION.md](https://github.com/govinda-attal/tkone/blob/main/crates/tkone-schedule/INTRODUCTION.md#what-it-doesnt-do-yet).

## Core Concepts

The library is organised around three independent spec types, each with a
corresponding recurrence iterator:

| Module | Spec type | Iterator item | Use when… |
|--------|-----------|---------------|-----------|
| [`date`] | `"YY-1M-31L"` | `Occurrence<DateTime<Tz>>` | calendar-day recurrence |
| [`time`] | `"1H:00:00"` | `DateTime<Tz>` | intra-day time recurrence |
| [`datetime`] | `"YY-1M-31L~NBT11:00:00"` | `Occurrence<DateTime<Tz>>` | combined date + time |

## Quick Start

### 1 — Find the first matching date, then iterate from it

The most common pattern is to derive a concrete *start datetime* from the spec
itself — i.e. "what is the very next occurrence?" — and then hand that datetime
back to the iterator so it becomes the first item of the series.

```rust
// Step 1: find the first occurrence strictly after now
let start = SpecIteratorBuilder::new_after("YY-1M-L", bdp.clone(), now)
    .build().unwrap().next().unwrap().unwrap()
    .observed().clone();

// Step 2: iterate from that start date (inclusive)
let iter = SpecIteratorBuilder::new_with_start("YY-1M-L", bdp, start)
    .build().unwrap();

// r.observed() → settlement date   r.actual() → raw calendar date
```
*Run `cargo run -p tkone-schedule --example date_recurrence` for the full program.*

### 2 — Combined date + time recurrence

Append `T<time_spec>` to a date spec to create a [`datetime`] schedule.
The iterator visits each valid calendar date in order and emits every
matching time within that day before advancing. By default each date's
times follow the start's time of day; see [`TimeAnchor`].

```rust
// "~NB" = shift to next business day if the date falls on a weekend
let start = SpecIteratorBuilder::new_after("YY-1M-L~NBT11:00:00", bdp.clone(), now)
    .build().unwrap().next().unwrap().unwrap()
    .observed().clone();

let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~NBT11:00:00", bdp, start)
    .build().unwrap();

```
*Run `cargo run -p tkone-schedule --example datetime_recurrence` for the full program.*

### 3 — Time-only recurrence

```rust
// Every 30 minutes starting at 09:00
let iter = SpecIteratorBuilder::new_with_start("HH:30M:00", start).build().unwrap();
// → 09:00, 09:30, 10:00, 10:30
```
*Run `cargo run -p tkone-schedule --example time_recurrence` for the full program.*

## Running the Bundled Examples

The crate ships runnable examples under `examples/`. Run any of them with:

```text
cargo run -p tkone-schedule --example date_recurrence
cargo run -p tkone-schedule --example datetime_recurrence
cargo run -p tkone-schedule --example time_recurrence
```

## Spec Syntax Reference

### Date Spec — `<years>-<months>-<days>[/<weeks>][~<adj>]`

#### Years

| Token | Meaning |
|-------|---------|
| `YY`  | Every calendar year |
| `_`   | Same as `YY` |
| `nY`  | Every *n* years, aligned to the iterator start date |
| `4Y[1,3]` | Years 1 and 3 of every 4-year cycle from the start |
| `2025` | Exactly year 2025 |
| `[2024,2025]` | Years 2024 or 2025 |
| `[2024-2026]` | Years 2024 to 2026 (ranges work in every bracketed list) |

#### Months

| Token | Meaning |
|-------|---------|
| `MM`  | Every calendar month |
| `_`   | Same as `MM` |
| `nM`  | Every *n* months, aligned to the iterator start date |
| `3M[1-2]` | Months 1 and 2 of every 3-month cycle from the start |
| `03`  | March only |
| `[01,06,12]` | January, June, or December |
| `[01-06]` | January to June |

#### Days

| Token | Meaning |
|-------|---------|
| `DD` or `_` | Every calendar day |
| `15`  | 15th of the month |
| `[5,10,15]` | 5th, 10th, and 15th |
| `L`   | Last day of the month |
| `31L` | 31st, or last day if the month is shorter |
| `31N` | 31st, or 1st of next month if the month is shorter |
| `31O` | 31st, or overflow remainder days into next month (e.g. → Mar 3 when Feb has 28 days) |
| `nD`  | Advance *n* calendar days |
| `8D[1-4]` | Days 1–4 of every 8-day cycle from the start (four on, four off) |
| `nBD` | Advance *n* business days |
| `nWD` | Advance *n* weekdays (Mon–Fri) |
| `MON` / `TUE` / … | Every occurrence of that weekday in the month |
| `[MON,FRI]` | Every Monday and Friday |
| `[MON-FRI]` | Every weekday (a range runs Monday to Sunday and does not wrap) |
| `[01-10,15]` | 1st to 10th, and 15th |
| `WED#2` | 2nd Wednesday of the month |
| `FRI#L` | Last Friday of the month |
| `THU#2L` | 2nd-to-last Thursday of the month |
| `BD#3` | 3rd business day of the month |
| `BD#L` | Last business day of the month |
| `BD#2L` | 2nd-to-last business day of the month |
| `BD#[1-5]` / `BD#[1,L]` | Several business days: the first five; the first and last |
| `WD#3` / `WD#L` | Same, counting weekdays (Mon–Fri) instead of business days |
| `EAS` / `EAS-2` / `EAS+1` | Easter Sunday · Good Friday · Easter Monday (days from Easter Sunday) |
| `OEAS` / `OEAS+1` | The same for Orthodox Easter |

#### Weeks (`/nW` after the day rule)

| Token | Meaning |
|-------|---------|
| `[TUE,THU]/2W` | Tuesday and Thursday in every 2nd week |
| `[MON-FRI]/4W[1,2]` | Weekdays in weeks 1 and 2 of every 4-week cycle |

Weeks run Monday to Sunday; week 1 is the week containing the start.

#### Business Day Adjustment (`~`)

Applied after the raw calendar date is resolved. Directional variants
(`~NB`, `~PB`, `~B`, `~MF`, `~MP`, `~SB`, `~NW`, `~PW`, `~W`) are **conditional** — they only act
when the raw date is not already a business/week day. Numeric variants
(`~nP`, `~nN`) are **unconditional** offsets.

| Token | Meaning |
|-------|---------|
| `~NB` | Next business day (shift forward if not already a business day) |
| `~PB` | Previous business day (shift back if not already a business day) |
| `~MF` | Modified Following: next business day, unless that is in the next month — then previous |
| `~MP` | Modified Preceding: previous business day, unless that is in the previous month — then next |
| `~SB` | Skip: drop the date if it is not a business day (nothing moves) |
| `~B`  | Nearest business day (shift to whichever direction is closer) |
| `~NW` | Next weekday — Mon–Fri (shift forward if not already a weekday) |
| `~PW` | Previous weekday — Mon–Fri (shift back if not already a weekday) |
| `~W`  | Nearest weekday — Mon–Fri (shift to whichever direction is closer) |
| `~3P` | 3 business days earlier (unconditional) |
| `~2N` | 2 business days later (unconditional) |
| `~MON` | Nearest Monday, at most 3 days away (any day: `~TUE`, …, `~SUN`) |
| `~NMON` | Next Monday, unless it is one |
| `~PMON` | Previous Monday, unless it is one |

### Time Spec — `<hours>:<minutes>:<seconds>`

| Token | Meaning |
|-------|---------|
| `HH`  | ForEach — drives if no `Every` is present |
| `_`   | AsIs — keep current hour |
| `nH`  | Every *n* hours |
| `09`  | At hour 9 (two-digit) |
| `MM`  | ForEach minute |
| `_`   | AsIs minute |
| `nM`  | Every *n* minutes |
| `30`  | At minute 30 |
| `SS`  | ForEach second |
| `_`   | AsIs second |
| `nS`  | Every *n* seconds |
| `00`  | At second 0 |
| `[09,12,17]` | A list: any of these values (any field) |
| `09-16` | A range, inclusive: hours 9 to 16 (any field) |
| `[09-11,14]` | Lists can contain ranges |

**ForEach driving rule**: when no `Every` component exists, the finest
`ForEach` field becomes `Every(1)` for its unit; coarser `ForEach` fields
carry their current value unchanged.

**Lists and ranges** fix a field to several values. Coarser than the step
they form a window (`09-16:15M:00` = every 15 minutes from 09:00 to 16:45);
finer, one grid per value (`HH:[00,30]:00` = at :00 and :30); with no step,
several times of day (`[09,12,17]:00:00`).

### DateTime Spec — `<date_spec>T<time_spec>`

The `T` separator is detected by the pattern that follows it (`HH:`, `nH:`,
or a two-digit clock hour `dd:`), so weekday tokens like `TUE` and `THU`
in the date spec are not confused with the separator.

```text
"YY-1M-31L~NBT11:00:00"  →  date="YY-1M-31L~NB"   time="11:00:00"
"YY-MM-DDT1H:00:00"      →  date="YY-MM-DD"         time="1H:00:00"
"YY-MM-THUT09:30:00"     →  date="YY-MM-THU"         time="09:30:00"
```

### Schedules: rules in context

A spec is one rule. What surrounds it — the start it is anchored to, the
time zone, the business days, an end or a count, several rules, and
exclusions — is set on a [`schedule::Schedule`], or written in a schedule
document (YAML or JSON, the `serde` feature). See the [`schedule`] module.

For tests that step through a schedule, such as checking a loan's interest
and repayments over ten years: `Schedule::slots_between(range)` always
ends, even with no `until`, and `Schedule::periods()` gives the time from
one slot to the next with both adjusted and unadjusted boundaries. The day
count and the money stay with the caller. tkone-calendar's
`Calendar::fixed` writes a calendar out in full so expected dates don't
change with its holiday data; its `tests/loan.rs` puts them together.
A `schedule::Timeline` reads several schedules, each on its own calendar
and in its own zone, as one stream of moments (`docs/timeline.md`).

The iterator builders also take an end (`with_end`), a count
(`with_count`) and exclusions (`with_exclusions`) for a single rule.

### Count — `with_count(n)`

`.with_count(n)` stops after *n* results counted from the start. Excluded
occurrences are not counted, so `with_count(12)` always yields 12 (unlike
iCalendar's `COUNT` with `EXDATE`).

```rust
use tkone_schedule::biz_day::WeekendSkipper;
use tkone_schedule::date::SpecIteratorBuilder;
use chrono::{TimeZone, Utc};
use fallible_iterator::FallibleIterator;

let start = Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap();
// nothing in July and August
let dates: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-15", WeekendSkipper::new(), start)
    .with_exclusions("YY-[07-08]-DD")
    .with_count(12)
    .build()?
    .map(|o| Ok(o.observed().format("%Y-%m").to_string()))
    .collect()?;
assert_eq!(dates.len(), 12);
assert_eq!(dates[6], "2025-09");
assert_eq!(dates[11], "2026-02");
```

### Checking a spec — `describe()`

Every spec type has a `describe()` that renders it in plain English, so a
spec can be shown to a user, or checked in a review, before it is
scheduled. It catches near-misses like `HH:MM:00` vs `1H:MM:00`:

```rust
use tkone_schedule::{date, datetime, time};
let t = |s: &str| s.parse::<time::Spec>().unwrap().describe();
assert_eq!(t("HH:MM:00"), "every minute, at second 00");
assert_eq!(t("1H:MM:00"), "every hour, at the start's minute and second 00");

let d: date::Spec = "YY-MM-L~PB".parse().unwrap();
assert_eq!(d.describe(), "on the last day of every month, moved to the previous business day when it is not one");

let dt: datetime::Spec = "YY-MM-FRI#LT16:30:00".parse().unwrap();
assert_eq!(dt.describe().unwrap(), "on the last Friday of every month; at 16:30:00");
```

"The start" in a description is the iterator's anchor: the `new_with_start`
start or the `new_after` / `resume_after` cursor.

## `Occurrence` and Business Day Adjustments

Date and datetime iterators yield [`Occurrence<T>`] rather than plain `T`.
This distinguishes unadjusted occurrences from ones where the business day
rule moved the settlement date:

- [`Occurrence::Exact`] — no adjustment; `actual == observed`.
- [`Occurrence::AdjustedEarlier`] — rule moved the date *earlier*.
- [`Occurrence::AdjustedLater`] — rule moved the date *later*.

Use `.observed()` for the settlement date and `.actual()` for the raw
calendar date.

## Daylight Saving Time

Schedules are computed on the wall clock (like cron), then mapped to real
instants in the iterator's timezone.

- **Gaps** (spring-forward, skipped days): a local time that does not exist
  moves later by the length of the gap. Date and datetime iterators report it
  as `AdjustedLater(label, shifted)`, where `label` shows the time as
  scheduled and `shifted` the time it is observed at. Both are the same
  instant.
- **Overlaps** (fall-back): the repeated hour is visited once, at the earlier
  offset.
- **Actual times strictly increase**: a later result whose actual time
  lands on an instant already given is dropped. Observed times can repeat:
  `YY-MM-DD~NBT09:00:00` gives Monday 09:00 for Saturday, Sunday and
  Monday. A [`schedule::Schedule`] merges those into one slot.

[`DstPolicy::Strict`] turns gaps and overlaps into errors instead. On the
time and datetime iterators, [`IntervalMode::Elapsed`] measures steps in
real elapsed time rather than on the wall clock.

```text
"1H:00:00" in America/New_York, 2025-03-09 (02:00 → 03:00):
  00:00 EST, 01:00 EST, 03:00 EDT, 04:00 EDT

"YY-MM-DD" in Pacific/Apia, which skipped 2011-12-30:
  Exact(Dec 29), AdjustedLater(Dec 30 -10:00, Dec 31 +14:00), Exact(Jan 1)
```

## Persisting and resuming a schedule

An application that stores schedules in a database does not keep an
iterator alive. It stores the next occurrence, runs the action when that is
due, then computes the following occurrence with `resume_after` from what it
stored. The result is the same series a single long-running iterator would
produce.

Per schedule, store:

| Column | Value | Used for |
|--------|-------|----------|
| spec | `"YY-1M-L~NBT17:00:00"` | rebuilding the iterator |
| timezone | IANA name, e.g. `"America/New_York"` (not an offset) | rebuilding the iterator |
| options | `TimeAnchor`, `IntervalMode`, `DstPolicy`, `with_exclusions`, `with_count` if set | rebuilding with the same behaviour |
| cursor | `occurrence.actual().to_rfc3339()` | resuming |
| due at | `occurrence.observed()` | when to run the action |
| end (optional) | an RFC 3339 end datetime | `.with_end(end)` when resuming |
| start (for positions, counts) | the schedule's start, RFC 3339 | `resume_with_start` — required for a cycle with positions (`/4W[1,2]`, `8D[1-4]`) or `with_count` |

```rust
struct Row {
    spec: String,
    tz: String,
    cursor: String, // RFC 3339 `actual` of the stored occurrence
    due_at: String, // RFC 3339 `observed`: when the action runs
}

impl Row {
    fn set(&mut self, occ: &Occurrence<DateTime<Tz>>) {
        self.cursor = occ.actual().to_rfc3339();
        self.due_at = occ.observed().to_rfc3339();
    }

    /// After the action has run: store the next occurrence, or report that
    /// the schedule has finished.
    fn advance(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        let tz: Tz = self.tz.parse()?;
        let cursor = DateTime::parse_from_rfc3339(&self.cursor)?;
        let next = SpecIteratorBuilder::resume_after(&self.spec, WeekendSkipper::new(), tz, cursor)
            .build()?
            .next()?;
        Ok(match next {
            Some(occ) => { self.set(&occ); true }
            None => false,
        })
    }
}

// Creating the schedule: store its first occurrence.
let tz = chrono_tz::America::New_York;
let created = tz.with_ymd_and_hms(2025, 5, 20, 12, 0, 0).unwrap();
let spec = "YY-1M-L~NBT17:00:00"; // last day of the month, next business day, 17:00
let first = SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), created)
    .build()?
    .next()?
    .expect("the spec has occurrences");
let mut row = Row { spec: spec.into(), tz: tz.name().into(), cursor: String::new(), due_at: String::new() };
row.set(&first);
// May 31 2025 is a Saturday: due Monday Jun 2, cursor stays on May 31
assert_eq!(row.due_at, "2025-06-02T17:00:00-04:00");
assert_eq!(row.cursor, "2025-05-31T17:00:00-04:00");

// When the action has run:
assert!(row.advance()?);
assert_eq!(row.due_at, "2025-06-30T17:00:00-04:00");
```

Rules that keep resumed results exact:

- **Resume from `actual`, never `observed`.** With `~PB`, Saturday's
  observed date is Friday; resuming from Friday would yield Saturday again,
  forever. With daily `~NB`, resuming from observed Monday would skip
  Sunday's occurrence.
- **Store `actual` with its UTC offset** (RFC 3339, or a timestamp plus
  offset). An occurrence moved by a DST gap is stored as its scheduled
  wall-clock time — `2025-03-09T02:30:00-05:00` for a daily 02:30 that
  happened at 03:30 EDT — so the schedule resumes at 02:30, not 03:30. The
  offset also tells fall-back's two 01:00s apart.
- **Rebuild with the same options** (`with_time_anchor`,
  `with_interval_mode`, `with_dst_policy`) every time.
- **`Ok(None)` means the schedule has finished**: a finite spec ran out
  (e.g. `2025-MM-01` after December), or the next occurrence is after the
  end you passed with `.with_end(end)` (inclusive). Store an end as a
  resolved datetime; `with_end_spec` is resolved from the original start, so
  it is not available when resuming.
- Cadences (`3M`, `2Y`, `1Y-3M`) and step rules (`7D`, `5BD`) stay aligned,
  because every stored occurrence lies on the cadence.
- **A cycle with positions** other than the first (`/4W[1,2]`, `8D[1-4]`,
  `3M[1-2]`) **needs its start**: a stored occurrence can be anywhere in its
  cycle. Store the start (the `new_with_start` start, or the first
  occurrence's `actual`) and resume with
  `resume_with_start(spec, bdp, tz, start, cursor)`; `resume_after` returns
  [`Error::UnsupportedResume`]. `resume_with_start` also works for other
  specs, with the same results as `resume_after`.
- **A counted schedule (`with_count`) needs its start too.** Resume it with
  `resume_with_start`, which replays it from the start (at most *n*
  results) to know how many have run. `resume_after` with `with_count`
  returns [`Error::UnsupportedResume`].
- A [`schedule::Schedule`] needs none of this: it is anchored at its start,
  and `slots_after(t)` gives the same slots from any `t`.

The same `resume_after` and `resume_with_start` exist on
[`date::SpecIteratorBuilder`] and [`time::SpecIteratorBuilder`].

<!-- cargo-rdme end -->

## Schedules: several rules, exclusions, until and count

A spec is one rule. A [`schedule::Schedule`](src/schedule.rs) holds what surrounds it: the start
it is anchored to (which also gives the time zone), the business days, an inclusive `until`, a
`count`, several named rules and exclusions. Its rules are datetime specs.

```rust
let schedule = Schedule::builder(start, bdp)              // start: DateTime<chrono_tz::Tz>
    .named_rule("close", "YY-MM-DD~SBT16:30:00")
    .named_rule("month-end", "YY-MM-L~PBT18:00:00")
    .until(until)
    .count(100)
    .exclude_date(NaiveDate::from_ymd_opt(2026, 11, 27).unwrap())
    .exclude_rule("YY-12-24")
    .build()?;

let mut slots = schedule.slots_after(&last_fired)?;   // a FallibleIterator
while let Some(slot) = slots.next()? {
    // slot.at: when it happens; slot.sources: the rules that produced it, with actual times
}
```

`slots()` iterates from the start, and `slots_after(t)` gives the same slots after any `t`, so a
stored schedule needs no cursor rules. With the `serde` feature, the same schedule is a document
([`ScheduleDoc`](src/schedule/document.rs)), read from YAML or JSON and resolved with a
`CalendarResolver`:

```yaml
version: 1
time_zone: America/New_York
calendar: XNYS
start: 2026-01-02T00:00:00
until: 2026-12-31
rules:
  - id: close
    rule: YY-MM-DD~SBT16:30:00
  - id: month-end
    rule: YY-MM-L~PBT18:00:00
exclude:
  dates: [2026-11-27]
  rules: [YY-12-24]
```

## Further reading

- [Introduction to the format](INTRODUCTION.md): why it exists, the syntax in five minutes,
  and a comparison with cron, Quartz and RRULE
- [Grammar and rules](GRAMMAR.md): the formal grammar, validity rules and evaluation rules,
  for validating the format or implementing it elsewhere
- [The format as an Internet-Draft](ietf/README.md): the ABNF, the draft, and the tooling that
  keeps them in sync with GRAMMAR.md
- [Date spec reference](src/date/date-spec.md): every date token, with worked examples
- [Time spec reference](src/time/time-spec.md): the time grid, steps, windows and lists
- [Combined date and time reference](src/datetime/date-time-spec.md): how dates and times
  combine, time anchors and edge cases
- Daylight saving worked examples: [date](src/date/date-spec.md#daylight-saving-time),
  [time](src/time/time-spec.md#daylight-saving-time),
  [datetime](src/datetime/date-time-spec.md#daylight-saving-time)
- [Schedule documents](../../docs/schedule-document.md): the design of `Schedule` and
  `ScheduleDoc`
- [Timelines](../../docs/timeline.md): several schedules read as one, with examples

## Builder options

These are set when building an iterator for a single rule, not written in the spec.

| Option | Iterators | Default | Effect |
|--------|-----------|---------|--------|
| `::new_after(spec, …, dtm)` / `::new(spec, …, tz)` | all | — | Results strictly after `dtm` (or now) |
| `::new_with_start(spec, …, start)` | all | — | `start` is the first result, as-is; cadences count from it |
| `::resume_after(spec, …, tz, last)` / `::resume_with_start(…, start, last)` | all | — | Continue a stored schedule (see below) |
| `.with_end(end)` / `.with_end_spec(spec)` | all | — | Inclusive upper bound (`with_end_spec` only with `new_with_start`) |
| `.with_count(n)` | all | — | Stop after `n` results counted from the start; excluded occurrences are not counted |
| `.with_exclusions(spec)` | all | — | Drop occurrences matching `spec`: a date spec (date, datetime) matched against the actual date, or a time spec (time). One spec per call; call it again for more |
| `.with_dst_policy(…)` | all | `Adjust` | `Strict` turns DST gaps and overlaps into errors |
| `.with_interval_mode(…)` | time, datetime | `WallClock` | `Elapsed` measures steps in real time across DST |
| `.with_time_anchor(…)` | time, datetime | `Start` | `Midnight` counts steps from each day's 00:00 (clock-aligned, like cron) instead of from the start's time of day |

`new_with_start(start)` always yields `start` first, unchanged and as `Exact`, even if it does
not match the spec. That lets you feed an already-adjusted (observed) date back in as the start.

To continue a schedule stored in a database, use `resume_after(spec, …, tz, last_actual)` with
the RFC 3339 `actual` of the last occurrence, plus `.with_end(end)` if it has an end. If the spec
has a cycle with positions (`/4W[1,2]`, `8D[1-4]`) or a count, resume it with
`resume_with_start(spec, …, tz, start, last_actual)` and the stored start.
See [Persisting and resuming a schedule](#persisting-and-resuming-a-schedule).

## API docs

```sh
cargo doc -p tkone-schedule --open
```

## Keeping README in sync

Everything between the `cargo-rdme` markers is generated from the `//!` doc comments in
`src/lib.rs` via [`cargo-rdme`](https://github.com/orium/cargo-rdme). Edit those, not the
README, then regenerate:

```sh
cargo install cargo-rdme
cargo rdme --workspace-project tkone-schedule
```
