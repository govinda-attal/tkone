# lib-schedule

A scheduling and recurrence library built on a flexible mini-language for dates, times, and
combined datetimes. Supports business-day processing, timezone awareness, and fallible iteration.

## Add to your project

```toml
[dependencies]
lib-schedule = { path = "crates/lib-schedule" }

[dev-dependencies]
chrono-tz = "0.10"
fallible-iterator = "0.3"
```

## Core concepts

The library is organised around three independent spec types, each with a corresponding
recurrence iterator:

| Module     | Spec string example          | Iterator item                    | Use when…                    |
|------------|------------------------------|----------------------------------|------------------------------|
| `date`     | `"YY-1M-31L"`                | `NextResult<DateTime<Tz>>`       | calendar-day recurrence      |
| `time`     | `"1H:00:00"`                 | `DateTime<Tz>`                   | intra-day time recurrence    |
| `datetime` | `"YY-1M-31L~WT11:00:00"`    | `NextResult<DateTime<Tz>>`       | combined date + time         |

<!-- cargo-rdme start -->

## Quick Start

### 1 — Date recurrence with business-day adjustment

The most common pattern is to derive a concrete *start datetime* from the spec itself —
"what is the very next occurrence?" — and hand that datetime back as the first item of the series.

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

Run the full program:

```sh
cargo run -p lib-schedule --example date_recurrence
```

### 2 — Combined date + time recurrence

Append `T<time_spec>` to a date spec. The iterator visits each valid calendar date and emits
every matching time within that day before advancing.

```rust
// "~W" adjusts Saturdays/Sundays to the nearest business day
let start = SpecIteratorBuilder::new_after("YY-1M-L~WT11:00:00", bdp.clone(), now)
    .build().unwrap().next().unwrap().unwrap()
    .observed().clone();

let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~WT11:00:00", bdp, start)
    .build().unwrap();
```

Run the full program:

```sh
cargo run -p lib-schedule --example datetime_recurrence
```

### 3 — Time-only recurrence

```rust
// Every 30 minutes starting at 09:00  →  09:00, 09:30, 10:00, 10:30
let iter = SpecIteratorBuilder::new_with_start("HH:30M:00", start).build().unwrap();
```

Run the full program:

```sh
cargo run -p lib-schedule --example time_recurrence
```

<!-- cargo-rdme end -->

## `NextResult` and business-day adjustments

Date and datetime iterators yield `NextResult<T>` rather than plain `T`, distinguishing
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
| Date recurrence | `cargo run -p lib-schedule --example date_recurrence` |
| Date + time recurrence | `cargo run -p lib-schedule --example datetime_recurrence` |
| Time-only recurrence | `cargo run -p lib-schedule --example time_recurrence` |

## API docs

```sh
cargo doc -p lib-schedule --open
```

## Keeping README in sync

The Quick Start section is generated from the `//!` doc comments in `src/lib.rs` via
[`cargo-rdme`](https://github.com/orium/cargo-rdme). To regenerate:

```sh
cargo install cargo-rdme
cargo rdme -p lib-schedule
```
