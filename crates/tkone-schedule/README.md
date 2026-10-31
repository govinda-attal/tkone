# tkone-schedule

A scheduling and recurrence library built on a flexible mini-language for dates, times, and
combined datetimes. Supports business-day processing, timezone awareness, and fallible iteration.

New to the schedule format? Read the [introduction](INTRODUCTION.md). It explains why the format
exists, how it compares with cron, Quartz and iCalendar RRULE, and walks through the spec with
real-world examples.

## Add to your project

```toml
[dependencies]
tkone-schedule = { path = "crates/tkone-schedule" }

[dev-dependencies]
chrono-tz = "0.10"
fallible-iterator = "0.3"
```

## Core concepts

The library is organised around three independent spec types, each with a corresponding
recurrence iterator:

| Module     | Spec string example          | Iterator item                    | Use when…                    |
|------------|------------------------------|----------------------------------|------------------------------|
| `date`     | `"YY-1M-31L"`                | `Occurrence<DateTime<Tz>>`       | calendar-day recurrence      |
| `time`     | `"1H:00:00"`                 | `DateTime<Tz>`                   | intra-day time recurrence    |
| `datetime` | `"YY-1M-31L~WT11:00:00"`    | `Occurrence<DateTime<Tz>>`       | combined date + time         |

<!-- cargo-rdme start -->

### tkone-schedule

A scheduling and recurrence library built on flexible mini-language specs for dates,
times, and combined datetimes. Supports business day processing, timezone awareness,
and fallible iteration.

#### Core Concepts

The library is organised around three independent spec types, each with a
corresponding recurrence iterator:

| Module | Spec type | Iterator item | Use when… |
|--------|-----------|---------------|-----------|
| [`date`] | `"YY-1M-31L"` | `Occurrence<DateTime<Tz>>` | calendar-day recurrence |
| [`time`] | `"1H:00:00"` | `DateTime<Tz>` | intra-day time recurrence |
| [`datetime`] | `"YY-1M-31L~NBT11:00:00"` | `Occurrence<DateTime<Tz>>` | combined date + time |

#### Quick Start

##### 1 — Find the first matching date, then iterate from it

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

##### 2 — Combined date + time recurrence

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

##### 3 — Time-only recurrence

```rust
// Every 30 minutes starting at 09:00
let iter = SpecIteratorBuilder::new_with_start("HH:30M:00", start).build().unwrap();
// → 09:00, 09:30, 10:00, 10:30
```
*Run `cargo run -p tkone-schedule --example time_recurrence` for the full program.*

#### Running the Bundled Examples

The crate ships runnable examples under `examples/`. Run any of them with:

```text
cargo run -p tkone-schedule --example date_recurrence
cargo run -p tkone-schedule --example datetime_recurrence
cargo run -p tkone-schedule --example time_recurrence
```

#### Spec Syntax Reference

##### Date Spec — `<years>-<months>-<days>[~<adj>]`

###### Years

| Token | Meaning |
|-------|---------|
| `YY`  | Every calendar year |
| `_`   | Same as `YY` |
| `nY`  | Every *n* years, aligned to the iterator start date |
| `2025` | Exactly year 2025 |
| `[2024,2025]` | Years 2024 or 2025 |
| `[2024-2026]` | Years 2024 to 2026 (ranges work in every bracketed list) |

###### Months

| Token | Meaning |
|-------|---------|
| `MM`  | Every calendar month |
| `_`   | Same as `MM` |
| `nM`  | Every *n* months, aligned to the iterator start date |
| `03`  | March only |
| `[01,06,12]` | January, June, or December |
| `[01-06]` | January to June |

###### Days

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
| `WD#3` / `WD#L` | Same, counting weekdays (Mon–Fri) instead of business days |

###### Business Day Adjustment (`~`)

Applied after the raw calendar date is resolved. Directional variants
(`~NB`, `~PB`, `~B`, `~MF`, `~MP`, `~NW`, `~PW`, `~W`) are **conditional** — they only shift
when the raw date is not already a business/week day. Numeric variants
(`~nP`, `~nN`) are **unconditional** offsets.

| Token | Meaning |
|-------|---------|
| `~NB` | Next business day (shift forward if not already a business day) |
| `~PB` | Previous business day (shift back if not already a business day) |
| `~MF` | Modified Following: next business day, unless that is in the next month — then previous |
| `~MP` | Modified Preceding: previous business day, unless that is in the previous month — then next |
| `~B`  | Nearest business day (shift to whichever direction is closer) |
| `~NW` | Next weekday — Mon–Fri (shift forward if not already a weekday) |
| `~PW` | Previous weekday — Mon–Fri (shift back if not already a weekday) |
| `~W`  | Nearest weekday — Mon–Fri (shift to whichever direction is closer) |
| `~3P` | 3 business days earlier (unconditional) |
| `~2N` | 2 business days later (unconditional) |

##### Time Spec — `<hours>:<minutes>:<seconds>`

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

##### DateTime Spec — `<date_spec>T<time_spec>`

The `T` separator is detected by the pattern that follows it (`HH:`, `nH:`,
or a two-digit clock hour `dd:`), so weekday tokens like `TUE` and `THU`
in the date spec are not confused with the separator.

```text
"YY-1M-31L~NBT11:00:00"  →  date="YY-1M-31L~NB"   time="11:00:00"
"YY-MM-DDT1H:00:00"      →  date="YY-MM-DD"         time="1H:00:00"
"YY-MM-THUT09:30:00"     →  date="YY-MM-THU"         time="09:30:00"
```

##### Union — `<spec> | <spec> | …`

Any date, time or datetime spec can be a union of specs of the same kind,
separated by `|`. The iterator yields the occurrences of every part in
order of actual time. When two parts produce the same actual time, only the
first listed part's occurrence is kept, so actual times still strictly
increase.

```text
"YY-MM-BD#3 | YY-MM-L~PB"                     3rd and last business day of each month
"09:[30,45]:00 | 10-15:15M:00 | 16:00:00"     every 15 minutes from 09:30 to 16:00
"YY-MM-BD#1T09:00:00 | YY-MM-FRI#LT17:00:00"  different dates and times per part
```

Each part keeps its own cadence and phase from the start. Fixed times of
day in a time-only union recur every day. Resuming a union needs the
schedule's start; see [Persisting and resuming a schedule](#persisting-and-resuming-a-schedule).
To describe a union, use [`date::describe`], [`time::describe`] or
[`datetime::describe`], which take the union string.

##### Exclusions — `<spec> ! <exclusion> | <exclusion> | …`

Everything after a `!` is excluded:

- date and datetime specs exclude by a **date spec**, matched against each
  occurrence's actual (unadjusted) date: `YY-MM-DD ! YY-12-25 | YY-01-01`.
  For a datetime spec every time on an excluded date is skipped. The
  exclusion spec cannot use day steps (`7D`).
- time specs exclude by a **time spec**: `1H:00:00 ! 12:MM:SS` skips the
  12:00 hour.

The exclusion applies to everything before the `!`, including every part of
a union. Builders also take `.with_exclusions(spec)`, which adds exclusions
the same way.

##### Count — `with_count(n)` (builder option)

`.with_count(n)` stops after *n* results counted from the start. It is a
builder option, not spec syntax. Excluded occurrences are not counted, so
`with_count(12)` always yields 12 (unlike iCalendar's `COUNT` with `EXDATE`).

```rust
use tkone_schedule::biz_day::WeekendSkipper;
use tkone_schedule::date::SpecIteratorBuilder;
use chrono::{TimeZone, Utc};
use fallible_iterator::FallibleIterator;

let start = Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap();
// nothing in July and August
let dates: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-15 ! YY-[07-08]-DD", WeekendSkipper::new(), start)
    .with_count(12)
    .build()?
    .map(|o| Ok(o.observed().format("%Y-%m").to_string()))
    .collect()?;
assert_eq!(dates.len(), 12);
assert_eq!(dates[6], "2025-09");
assert_eq!(dates[11], "2026-02");
```

##### Checking a spec — `describe()`

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

#### `Occurrence` and Business Day Adjustments

Date and datetime iterators yield [`Occurrence<T>`] rather than plain `T`.
This distinguishes unadjusted occurrences from ones where the business day
rule moved the settlement date:

- [`Occurrence::Exact`] — no adjustment; `actual == observed`.
- [`Occurrence::AdjustedEarlier`] — rule moved the date *earlier*.
- [`Occurrence::AdjustedLater`] — rule moved the date *later*.

Use `.observed()` for the settlement date and `.actual()` for the raw
calendar date.

#### Daylight Saving Time

Schedules are computed on the wall clock (like cron), then mapped to real
instants in the iterator's timezone.

- **Gaps** (spring-forward, skipped days): a local time that does not exist
  moves later by the length of the gap. Date and datetime iterators report it
  as `AdjustedLater(label, shifted)`, where `label` shows the time as
  scheduled and `shifted` the time it is observed at. Both are the same
  instant.
- **Overlaps** (fall-back): the repeated hour is visited once, at the earlier
  offset.
- **No duplicates**: results strictly increase; a later result that lands on
  the same instant is dropped.

[`DstPolicy::Strict`] turns gaps and overlaps into errors instead. On the
time and datetime iterators, [`IntervalMode::Elapsed`] measures steps in
real elapsed time rather than on the wall clock.

```text
"1H:00:00" in America/New_York, 2025-03-09 (02:00 → 03:00):
  00:00 EST, 01:00 EST, 03:00 EDT, 04:00 EDT

"YY-MM-DD" in Pacific/Apia, which skipped 2011-12-30:
  Exact(Dec 29), AdjustedLater(Dec 30 -10:00, Dec 31 +14:00), Exact(Jan 1)
```

#### Persisting and resuming a schedule

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
| start (for unions, counts) | the schedule's start, RFC 3339 | `resume_with_start` — required for a union spec or `with_count` |

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
- **A union spec (`A | B`) needs its start.** A stored occurrence belongs to
  one part, and the other parts' cadences, steps and time grids are not
  aligned to it. Store the start (the `new_with_start` start, or the first
  occurrence's `actual`) and resume with
  `resume_with_start(spec, bdp, tz, start, cursor)`, which positions every
  part as if iterating from the start. `resume_after` on a union returns
  [`Error::UnsupportedResume`], and so does a union in
  [`IntervalMode::Elapsed`]. `resume_with_start` also works for plain specs,
  with the same results as `resume_after`.

- **A counted schedule (`with_count`) needs its start too.** Resume it with
  `resume_with_start`, which replays it from the start (at most *n*
  results) to know how many have run; this works for any spec and mode,
  including a union in `Elapsed`. `resume_after` with `with_count` returns
  [`Error::UnsupportedResume`].

The same `resume_after` and `resume_with_start` exist on
[`date::SpecIteratorBuilder`] and [`time::SpecIteratorBuilder`].

<!-- cargo-rdme end -->

## `Occurrence` and business-day adjustments

Date and datetime iterators yield `Occurrence<T>` rather than plain `T`, distinguishing
unadjusted dates from adjusted ones:

| Variant | Meaning |
|---------|---------|
| `Exact(t)` | No adjustment; `actual == observed` |
| `AdjustedLater(actual, observed)` | Settlement moved *later* than the raw calendar date |
| `AdjustedEarlier(actual, observed)` | Settlement moved *earlier* than the raw calendar date |

Use `.observed()` for the settlement date and `.actual()` for the raw calendar date.

`AdjustedLater` is also used when a scheduled local time falls in a DST gap: `actual` shows the
time as scheduled and `observed` the time it happens at (the same instant). See
[Daylight saving time](#daylight-saving-time).

## Daylight saving time

Schedules follow the wall clock and never repeat an instant:

| Situation | Default behaviour | Options |
|-----------|-------------------|---------|
| Time doesn't exist (spring-forward, skipped day) | Moves later by the gap; reported as `AdjustedLater` in date / datetime results | `DstPolicy::Strict` returns an error |
| Time exists twice (fall-back) | Visited once, at the earlier offset | `DstPolicy::Strict` returns an error |
| Two scheduled times land on one instant | The later one is dropped | — |
| Time-spec steps (`nH`, `nM`, `nS`) | Move the wall clock, like cron | `IntervalMode::Elapsed` (time and datetime iterators) measures real elapsed time |

Worked examples for each case: [Time spec](src/time/time-spec.md#daylight-saving-time),
[DateTime spec](src/datetime/date-time-spec.md#daylight-saving-time),
[Date spec](src/date/date-spec.md#daylight-saving-time).

## Spec syntax

### Date spec — `<years>-<months>-<days>[~<adj>]`

| Component | Example tokens | Meaning |
|-----------|---------------|---------|
| Years | `YY` `nY` `2025` `[2024,2025]` `[2024-2026]` | Every year / every n years / specific / enumerated / range |
| Months | `MM` `nM` `06` `[01,06,12]` `[01-06]` | Every month / every n months / specific / enumerated / range |
| Days | `DD` `L` `15` `[01-10]` `nD` `nBD` `MON` `[MON-FRI]` `FRI#L` `BD#3` `BD#L` | Every day / last / fixed / rolling / weekday patterns / nth business day |
| Adjustment | `~W` `~B` `~NB` `~PB` `~MF` `~MP` `~NW` `~PW` `~nN` `~nP` | Conditional or unconditional business-day shift (incl. Modified Following / Preceding) |

Full syntax with all tokens and worked examples:
**[Date Spec Reference →](src/date/date-spec.md)**

### Time spec — `<hours>:<minutes>:<seconds>`

| Token type | Examples | Meaning |
|------------|----------|---------|
| `Every(n)` | `1H` `30M` `15S` | Advance by n units each tick |
| `At(v)` | `09` `30` `00` | Pin to exact value |
| `Values` | `[09,12,17]` `09-16` `[09-11,14]` | Pin to any of a list / inclusive range |
| `ForEach` | `HH` `MM` `SS` | Carry (when `Every` present) or drive by 1 unit (finest wildcard) |
| `AsIs` | `_` | No-op; preserve current value |

Full syntax, semantics, and worked examples:
**[Time Spec Reference →](src/time/time-spec.md)**

### Union — `<spec> | <spec> | …`

Any date, time or datetime spec can combine several specs of the same kind with `|`; results
from all parts are merged in time order (`"09:[30,45]:00 | 10-15:15M:00 | 16:00:00"` = every 15
minutes from 09:30 to 16:00).

### Exclusions — `<spec> ! <exclusion> | …`

Everything after `!` is excluded: a date spec for date and datetime specs (by actual date), a
time spec for time specs (`"YY-MM-DD ! YY-12-25 | YY-01-01"`, `"1H:00:00 ! 12:MM:SS"`).
`.with_exclusions(spec)` on the builder does the same.

### DateTime spec — `<date_spec>T<time_spec>`

The `T` separator is detected by what immediately follows it (`HH:`, `nH:`, `_:`, a
two-digit clock hour, an hour range `09-16:` or an hour list `[09,12]:`), so weekday tokens like `TUE` and `THU` in the date part are never
misidentified.

```
"YY-1M-31L~WT11:00:00"   →  date="YY-1M-31L~W"    time="11:00:00"
"YY-MM-DDTHH:30M:00"     →  date="YY-MM-DD"         time="HH:30M:00"
"YY-MM-THUT09:30:00"     →  date="YY-MM-THU"         time="09:30:00"
```

Full combining semantics, tick-per-day rules, and edge cases:
**[DateTime Spec Reference →](src/datetime/date-time-spec.md)**

## Builder options

These are library options set when building an iterator; they are not part of the spec text.
The [introduction](INTRODUCTION.md) covers the format itself.

| Option | Iterators | Default | Effect |
|--------|-----------|---------|--------|
| `.with_end(end)` / `.with_end_spec(spec)` | all | — | Inclusive upper bound; nothing is invented at the boundary |
| `.with_dst_policy(…)` | all | `Adjust` | `Strict` turns DST gaps and overlaps into errors |
| `.with_interval_mode(…)` | time, datetime | `WallClock` | `Elapsed` measures steps in real time across DST |
| `.with_exclusions(spec)` | all | — | Same as writing `! spec` in the spec; combined with exclusions already there |
| `.with_count(n)` | all | — | Stop after `n` results counted from the start; excluded occurrences are not counted |
| `.with_time_anchor(…)` | datetime, time | `Start` | `Midnight` counts steps from each day's 00:00 (clock-aligned, like cron) instead of from the start's time of day |

`new_with_start(start)` always yields `start` first, unchanged and as `Exact`, even if it does
not match the spec. That lets you feed an already-adjusted (observed) date back in as the start.

To continue a schedule stored in a database, use `resume_after(spec, …, tz, last_actual)` with
the RFC 3339 `actual` of the last occurrence, plus `.with_end(end)` if it has an end. A union
spec (`A | B`) or a schedule with `with_count` must be resumed with
`resume_with_start(spec, …, tz, start, last_actual)`, which also needs the schedule's stored start — see
[Persisting and resuming a schedule](#persisting-and-resuming-a-schedule).

## Runnable examples

| Example | Command |
|---------|---------|
| Date recurrence | `cargo run -p tkone-schedule --example date_recurrence` |
| Date + time recurrence | `cargo run -p tkone-schedule --example datetime_recurrence` |
| Time-only recurrence | `cargo run -p tkone-schedule --example time_recurrence` |

## API docs

```sh
cargo doc -p tkone-schedule --open
```

## Keeping README in sync

The Quick Start section is generated from the `//!` doc comments in `src/lib.rs` via
[`cargo-rdme`](https://github.com/orium/cargo-rdme). To regenerate:

```sh
cargo install cargo-rdme
cargo rdme --workspace-project tkone-schedule
```
