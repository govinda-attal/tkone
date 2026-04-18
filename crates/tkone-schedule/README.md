# tkone-schedule

A scheduling and recurrence library built on a flexible mini-language for dates, times, and
combined datetimes. Supports business-day processing, timezone awareness, and fallible iteration.

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
matching time within that day before advancing.

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
| `_`   | Keep current year (no-op) |
| `nY`  | Every *n* years, aligned to the iterator start date |
| `2025` | Exactly year 2025 |
| `[2024,2025]` | Years 2024 or 2025 |

###### Months

| Token | Meaning |
|-------|---------|
| `MM`  | Every calendar month |
| `_`   | Keep current month (no-op) |
| `nM`  | Every *n* months, aligned to the iterator start date |
| `03`  | March only |
| `[01,06,12]` | January, June, or December |

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
| `WED#2` | 2nd Wednesday of the month |
| `FRI#L` | Last Friday of the month |
| `THU#2L` | 2nd-to-last Thursday of the month |

###### Business Day Adjustment (`~`)

Applied after the raw calendar date is resolved. Directional variants
(`~NB`, `~PB`, `~B`, `~NW`, `~PW`, `~W`) are **conditional** — they only shift
when the raw date is not already a business/week day. Numeric variants
(`~nP`, `~nN`) are **unconditional** offsets.

| Token | Meaning |
|-------|---------|
| `~NB` | Next business day (shift forward if not already a business day) |
| `~PB` | Previous business day (shift back if not already a business day) |
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

**ForEach driving rule**: when no `Every` component exists, the finest
`ForEach` field becomes `Every(1)` for its unit; coarser `ForEach` fields
carry their current value unchanged.

##### DateTime Spec — `<date_spec>T<time_spec>`

The `T` separator is detected by the pattern that follows it (`HH:`, `nH:`,
or a two-digit clock hour `dd:`), so weekday tokens like `TUE` and `THU`
in the date spec are not confused with the separator.

```text
"YY-1M-31L~NBT11:00:00"  →  date="YY-1M-31L~NB"   time="11:00:00"
"YY-MM-DDT1H:00:00"      →  date="YY-MM-DD"         time="1H:00:00"
"YY-MM-THUT09:30:00"     →  date="YY-MM-THU"         time="09:30:00"
```

#### `Occurrence` and Business Day Adjustments

Date and datetime iterators yield [`Occurrence<T>`] rather than plain `T`.
This distinguishes unadjusted occurrences from ones where the business day
rule moved the settlement date:

- [`Occurrence::Exact`] — no adjustment; `actual == observed`.
- [`Occurrence::AdjustedEarlier`] — rule moved the date *earlier*.
- [`Occurrence::AdjustedLater`] — rule moved the date *later*.

Use `.observed()` for the settlement date and `.actual()` for the raw
calendar date.

<!-- cargo-rdme end -->

## `Occurrence` and business-day adjustments

Date and datetime iterators yield `Occurrence<T>` rather than plain `T`, distinguishing
unadjusted dates from adjusted ones:

| Variant | Meaning |
|---------|---------|
| `Single(t)` | No adjustment; `actual == observed` |
| `AdjustedLater(actual, observed)` | Settlement moved *later* than the raw calendar date |
| `AdjustedEarlier(actual, observed)` | Settlement moved *earlier* than the raw calendar date |

Use `.observed()` for the settlement date and `.actual()` for the raw calendar date.

## Spec syntax

### Date spec — `<years>-<months>-<days>[~<adj>]`

| Component | Example tokens | Meaning |
|-----------|---------------|---------|
| Years | `YY` `nY` `2025` `[2024,2025]` | Every year / every n years / specific / enumerated |
| Months | `MM` `nM` `06` `[01,06,12]` | Every month / every n months / specific / enumerated |
| Days | `DD` `L` `15` `nD` `nBD` `MON` `FRI#L` | Every day / last / fixed / rolling / weekday patterns |
| Adjustment | `~W` `~B` `~NW` `~PW` `~nN` `~nP` | Conditional or unconditional business-day shift |

Full syntax with all tokens and worked examples:
**[Date Spec Reference →](src/date/date-spec.md)**

### Time spec — `<hours>:<minutes>:<seconds>`

| Token type | Examples | Meaning |
|------------|----------|---------|
| `Every(n)` | `1H` `30M` `15S` | Advance by n units each tick |
| `At(v)` | `09` `30` `00` | Pin to exact value |
| `ForEach` | `HH` `MM` `SS` | Carry (when `Every` present) or drive by 1 unit (finest wildcard) |
| `AsIs` | `_` | No-op; preserve current value |

Full syntax, semantics, and worked examples:
**[Time Spec Reference →](src/time/time-spec.md)**

### DateTime spec — `<date_spec>T<time_spec>`

The `T` separator is detected by what immediately follows it (`HH:`, `nH:`, `_:`, or a
two-digit clock hour), so weekday tokens like `TUE` and `THU` in the date part are never
misidentified.

```
"YY-1M-31L~WT11:00:00"   →  date="YY-1M-31L~W"    time="11:00:00"
"YY-MM-DDTHH:30M:00"     →  date="YY-MM-DD"         time="HH:30M:00"
"YY-MM-THUT09:30:00"     →  date="YY-MM-THU"         time="09:30:00"
```

Full combining semantics, tick-per-day rules, and edge cases:
**[DateTime Spec Reference →](src/datetime/date-time-spec.md)**

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
