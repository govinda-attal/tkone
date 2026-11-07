//! A scheduling and recurrence library built on flexible mini-language specs for dates,
//! times, and combined datetimes. Supports business day processing, timezone awareness,
//! and fallible iteration.
//!
//! It is made for operations: payment runs, batch jobs, reports and deadlines
//! that must land on the right business day. It does not define financial
//! instruments (calculation periods, stubs, fixing offsets, day counts); see
//! [INTRODUCTION.md](https://github.com/govinda-attal/tkone/blob/main/crates/tkone-schedule/INTRODUCTION.md#what-it-doesnt-do-yet).
//!
//! ## Core Concepts
//!
//! The library is organised around three independent spec types, each with a
//! corresponding recurrence iterator:
//!
//! | Module | Spec type | Iterator item | Use when… |
//! |--------|-----------|---------------|-----------|
//! | [`date`] | `"YY-1M-31L"` | `Occurrence<DateTime<Tz>>` | calendar-day recurrence |
//! | [`time`] | `"1H:00:00"` | `DateTime<Tz>` | intra-day time recurrence |
//! | [`datetime`] | `"YY-1M-31L~NBT11:00:00"` | `Occurrence<DateTime<Tz>>` | combined date + time |
//!
//! ## Quick Start
//!
//! ### 1 — Find the first matching date, then iterate from it
//!
//! The most common pattern is to derive a concrete *start datetime* from the spec
//! itself — i.e. "what is the very next occurrence?" — and then hand that datetime
//! back to the iterator so it becomes the first item of the series.
//!
//! ```rust
//! # use tkone_schedule::biz_day::WeekendSkipper;
//! # use tkone_schedule::date::SpecIteratorBuilder;
//! # use chrono::{SubsecRound, Utc};
//! # use chrono_tz::America::New_York;
//! # use fallible_iterator::FallibleIterator;
//! # let bdp = WeekendSkipper::new();
//! # let now = Utc::now().with_timezone(&New_York).trunc_subsecs(0);
//! // Step 1: find the first occurrence strictly after now
//! let start = SpecIteratorBuilder::new_after("YY-1M-L", bdp.clone(), now)
//!     .build().unwrap().next().unwrap().unwrap()
//!     .observed().clone();
//!
//! // Step 2: iterate from that start date (inclusive)
//! let iter = SpecIteratorBuilder::new_with_start("YY-1M-L", bdp, start)
//!     .build().unwrap();
//!
//! # let dates: Vec<_> = iter.take(3).collect().unwrap();
//! // r.observed() → settlement date   r.actual() → raw calendar date
//! ```
//! *Run `cargo run -p tkone-schedule --example date_recurrence` for the full program.*
//!
//! ### 2 — Combined date + time recurrence
//!
//! Append `T<time_spec>` to a date spec to create a [`datetime`] schedule.
//! The iterator visits each valid calendar date in order and emits every
//! matching time within that day before advancing. By default each date's
//! times follow the start's time of day; see [`TimeAnchor`].
//!
//! ```rust
//! # use tkone_schedule::biz_day::WeekendSkipper;
//! # use tkone_schedule::datetime::SpecIteratorBuilder;
//! # use chrono::{SubsecRound, Utc};
//! # use chrono_tz::Europe::London;
//! # use fallible_iterator::FallibleIterator;
//! # let bdp = WeekendSkipper::new();
//! # let now = Utc::now().with_timezone(&London).trunc_subsecs(0);
//! // "~NB" = shift to next business day if the date falls on a weekend
//! let start = SpecIteratorBuilder::new_after("YY-1M-L~NBT11:00:00", bdp.clone(), now)
//!     .build().unwrap().next().unwrap().unwrap()
//!     .observed().clone();
//!
//! let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~NBT11:00:00", bdp, start)
//!     .build().unwrap();
//!
//! # let _: Vec<_> = iter.take(4).collect().unwrap();
//! ```
//! *Run `cargo run -p tkone-schedule --example datetime_recurrence` for the full program.*
//!
//! ### 3 — Time-only recurrence
//!
//! ```rust
//! # use tkone_schedule::time::SpecIteratorBuilder;
//! # use chrono::{TimeZone, Utc};
//! # use fallible_iterator::FallibleIterator;
//! # let start = Utc.with_ymd_and_hms(2024, 6, 1, 9, 0, 0).unwrap();
//! // Every 30 minutes starting at 09:00
//! let iter = SpecIteratorBuilder::new_with_start("HH:30M:00", start).build().unwrap();
//! # let _: Vec<_> = iter.take(4).collect().unwrap();
//! // → 09:00, 09:30, 10:00, 10:30
//! ```
//! *Run `cargo run -p tkone-schedule --example time_recurrence` for the full program.*
//!
//! ## Running the Bundled Examples
//!
//! The crate ships runnable examples under `examples/`. Run any of them with:
//!
//! ```text
//! cargo run -p tkone-schedule --example date_recurrence
//! cargo run -p tkone-schedule --example datetime_recurrence
//! cargo run -p tkone-schedule --example time_recurrence
//! ```
//!
//! ## Spec Syntax Reference
//!
//! ### Date Spec — `<years>-<months>-<days>[/<weeks>][~<adj>]`
//!
//! #### Years
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `YY`  | Every calendar year |
//! | `_`   | Same as `YY` |
//! | `nY`  | Every *n* years, aligned to the iterator start date |
//! | `4Y[1,3]` | Years 1 and 3 of every 4-year cycle from the start |
//! | `2025` | Exactly year 2025 |
//! | `[2024,2025]` | Years 2024 or 2025 |
//! | `[2024-2026]` | Years 2024 to 2026 (ranges work in every bracketed list) |
//!
//! #### Months
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `MM`  | Every calendar month |
//! | `_`   | Same as `MM` |
//! | `nM`  | Every *n* months, aligned to the iterator start date |
//! | `3M[1-2]` | Months 1 and 2 of every 3-month cycle from the start |
//! | `03`  | March only |
//! | `[01,06,12]` | January, June, or December |
//! | `[01-06]` | January to June |
//!
//! #### Days
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `DD` or `_` | Every calendar day |
//! | `15`  | 15th of the month |
//! | `[5,10,15]` | 5th, 10th, and 15th |
//! | `L`   | Last day of the month |
//! | `31L` | 31st, or last day if the month is shorter |
//! | `31N` | 31st, or 1st of next month if the month is shorter |
//! | `31O` | 31st, or overflow remainder days into next month (e.g. → Mar 3 when Feb has 28 days) |
//! | `nD`  | Advance *n* calendar days |
//! | `8D[1-4]` | Days 1–4 of every 8-day cycle from the start (four on, four off) |
//! | `nBD` | Advance *n* business days |
//! | `nWD` | Advance *n* weekdays (Mon–Fri) |
//! | `MON` / `TUE` / … | Every occurrence of that weekday in the month |
//! | `[MON,FRI]` | Every Monday and Friday |
//! | `[MON-FRI]` | Every weekday (a range runs Monday to Sunday and does not wrap) |
//! | `[01-10,15]` | 1st to 10th, and 15th |
//! | `WED#2` | 2nd Wednesday of the month |
//! | `FRI#L` | Last Friday of the month |
//! | `THU#2L` | 2nd-to-last Thursday of the month |
//! | `BD#3` | 3rd business day of the month |
//! | `BD#L` | Last business day of the month |
//! | `BD#2L` | 2nd-to-last business day of the month |
//! | `BD#[1-5]` / `BD#[1,L]` | Several business days: the first five; the first and last |
//! | `WD#3` / `WD#L` | Same, counting weekdays (Mon–Fri) instead of business days |
//! | `EAS` / `EAS-2` / `EAS+1` | Easter Sunday · Good Friday · Easter Monday (days from Easter Sunday) |
//! | `OEAS` / `OEAS+1` | The same for Orthodox Easter |
//!
//! #### Weeks (`/nW` after the day rule)
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `[TUE,THU]/2W` | Tuesday and Thursday in every 2nd week |
//! | `[MON-FRI]/4W[1,2]` | Weekdays in weeks 1 and 2 of every 4-week cycle |
//!
//! Weeks run Monday to Sunday; week 1 is the week containing the start.
//!
//! #### Business Day Adjustment (`~`)
//!
//! Applied after the raw calendar date is resolved. Directional variants
//! (`~NB`, `~PB`, `~B`, `~MF`, `~MP`, `~SB`, `~NW`, `~PW`, `~W`) are **conditional** — they only act
//! when the raw date is not already a business/week day. Numeric variants
//! (`~nP`, `~nN`) are **unconditional** offsets.
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `~NB` | Next business day (shift forward if not already a business day) |
//! | `~PB` | Previous business day (shift back if not already a business day) |
//! | `~MF` | Modified Following: next business day, unless that is in the next month — then previous |
//! | `~MP` | Modified Preceding: previous business day, unless that is in the previous month — then next |
//! | `~SB` | Skip: drop the date if it is not a business day (nothing moves) |
//! | `~B`  | Nearest business day (shift to whichever direction is closer) |
//! | `~NW` | Next weekday — Mon–Fri (shift forward if not already a weekday) |
//! | `~PW` | Previous weekday — Mon–Fri (shift back if not already a weekday) |
//! | `~W`  | Nearest weekday — Mon–Fri (shift to whichever direction is closer) |
//! | `~3P` | 3 business days earlier (unconditional) |
//! | `~2N` | 2 business days later (unconditional) |
//!
//! ### Time Spec — `<hours>:<minutes>:<seconds>`
//!
//! | Token | Meaning |
//! |-------|---------|
//! | `HH`  | ForEach — drives if no `Every` is present |
//! | `_`   | AsIs — keep current hour |
//! | `nH`  | Every *n* hours |
//! | `09`  | At hour 9 (two-digit) |
//! | `MM`  | ForEach minute |
//! | `_`   | AsIs minute |
//! | `nM`  | Every *n* minutes |
//! | `30`  | At minute 30 |
//! | `SS`  | ForEach second |
//! | `_`   | AsIs second |
//! | `nS`  | Every *n* seconds |
//! | `00`  | At second 0 |
//! | `[09,12,17]` | A list: any of these values (any field) |
//! | `09-16` | A range, inclusive: hours 9 to 16 (any field) |
//! | `[09-11,14]` | Lists can contain ranges |
//!
//! **ForEach driving rule**: when no `Every` component exists, the finest
//! `ForEach` field becomes `Every(1)` for its unit; coarser `ForEach` fields
//! carry their current value unchanged.
//!
//! **Lists and ranges** fix a field to several values. Coarser than the step
//! they form a window (`09-16:15M:00` = every 15 minutes from 09:00 to 16:45);
//! finer, one grid per value (`HH:[00,30]:00` = at :00 and :30); with no step,
//! several times of day (`[09,12,17]:00:00`).
//!
//! ### DateTime Spec — `<date_spec>T<time_spec>`
//!
//! The `T` separator is detected by the pattern that follows it (`HH:`, `nH:`,
//! or a two-digit clock hour `dd:`), so weekday tokens like `TUE` and `THU`
//! in the date spec are not confused with the separator.
//!
//! ```text
//! "YY-1M-31L~NBT11:00:00"  →  date="YY-1M-31L~NB"   time="11:00:00"
//! "YY-MM-DDT1H:00:00"      →  date="YY-MM-DD"         time="1H:00:00"
//! "YY-MM-THUT09:30:00"     →  date="YY-MM-THU"         time="09:30:00"
//! ```
//!
//! ### Union — `<spec> | <spec> | …`
//!
//! Any date, time or datetime spec can be a union of specs of the same kind,
//! separated by `|`. The iterator yields the occurrences of every part in
//! order of actual time. When two parts produce the same actual time, only the
//! first listed part's occurrence is kept, so actual times still strictly
//! increase.
//!
//! ```text
//! "YY-MM-BD#3 | YY-MM-L~PB"                     3rd and last business day of each month
//! "09:[30,45]:00 | 10-15:15M:00 | 16:00:00"     every 15 minutes from 09:30 to 16:00
//! "YY-MM-BD#1T09:00:00 | YY-MM-FRI#LT17:00:00"  different dates and times per part
//! ```
//!
//! Each part keeps its own cadence and phase from the start. Fixed times of
//! day in a time-only union recur every day. Resuming a union needs the
//! schedule's start; see [Persisting and resuming a schedule](#persisting-and-resuming-a-schedule).
//! To describe a union, use [`date::describe`], [`time::describe`] or
//! [`datetime::describe`], which take the union string.
//!
//! ### Exclusions — `<spec> ! <exclusion> | <exclusion> | …`
//!
//! Everything after a `!` is excluded:
//!
//! - date and datetime specs exclude by a **date spec**, matched against each
//!   occurrence's actual (unadjusted) date: `YY-MM-DD ! YY-12-25 | YY-01-01`.
//!   For a datetime spec every time on an excluded date is skipped. The
//!   exclusion spec cannot use day steps (`7D`).
//! - time specs exclude by a **time spec**: `1H:00:00 ! 12:MM:SS` skips the
//!   12:00 hour.
//!
//! The exclusion applies to everything before the `!`, including every part of
//! a union. Builders also take `.with_exclusions(spec)`, which adds exclusions
//! the same way.
//!
//! ### Start, end, time zone, count and calendar — `<spec> @<start> ..<end> x<count> [cal=<code>]`
//!
//! A spec can end with its own start and end, each a date or datetime with an
//! optional UTC offset and an optional IANA time zone in RFC 9557 brackets, so
//! the text alone defines the schedule:
//!
//! ```text
//! "YY-3M-15~NB @2025-02-15 ..2027-02-15"
//! "YY-MM-[TUE,THU]/2WT18:00:00 @2025-01-07[Europe/London]"
//! ```
//!
//! - `@start` anchors the schedule (cadences, week cycles, positions, steps,
//!   the time grid) and is an inclusive lower bound.
//! - `..end` is an inclusive upper bound; a date-only end covers the whole day.
//! - `..+30Y` ends a duration after the start instead (years, months, weeks and
//!   days: `+5Y6M`, `+2W`), itself excluded, so `@2025-01-31 ..+30Y` is exactly
//!   30 years: 360 monthly results. It needs a start, written or the builder's.
//! - `xN` stops after *N* results counted from the start (excluded ones are not
//!   counted); with an end too, whichever comes first stops it. Without a
//!   start it is counted from the first result, or a builder start.
//! - `[cal=GBLO]` names the holiday calendar the business days come from
//!   (`[cal=GBLO+USNY]`: open in both). The schedule then builds only with a
//!   business-day processor of that code ([`biz_day::BizDayProcessor::code`]),
//!   failing with [`Error::Calendar`] otherwise; see [`calendar_of`].
//! - A start, end or count given to the builder takes precedence. `new_after(dtm)` and
//!   `resume_after(last)` take a cursor, not a start, so a written start still
//!   anchors the schedule.
//! - `SpecIteratorBuilder::from_spec(spec, …)` builds from the text alone, in
//!   its time zone (UTC if none is named).
//!
//! ```rust
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use tkone_schedule::datetime::SpecIteratorBuilder;
//! use fallible_iterator::FallibleIterator;
//!
//! let spec = "YY-MM-8D[1-4]T06:00:00 @2025-01-01[America/Chicago] ..2025-01-12";
//! let shifts: Vec<_> = SpecIteratorBuilder::from_spec(spec, WeekendSkipper::new())?
//!     .build()?
//!     .map(|o| Ok(o.observed().format("%d %H:%M").to_string()))
//!     .collect()?;
//! assert_eq!(shifts, ["01 06:00", "02 06:00", "03 06:00", "04 06:00", "09 06:00", "10 06:00", "11 06:00", "12 06:00"]);
//! # Ok::<(), tkone_schedule::Error>(())
//! ```
//!
//! ### Count — `xN` or `with_count(n)`
//!
//! `xN` in the spec, or `.with_count(n)` on the builder (which takes
//! precedence), stops after *n* results counted from the start. Excluded occurrences are not counted, so
//! `with_count(12)` always yields 12 (unlike iCalendar's `COUNT` with `EXDATE`).
//!
//! ```rust
//! use tkone_schedule::biz_day::WeekendSkipper;
//! use tkone_schedule::date::SpecIteratorBuilder;
//! use chrono::{TimeZone, Utc};
//! use fallible_iterator::FallibleIterator;
//!
//! let start = Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap();
//! // nothing in July and August
//! let dates: Vec<_> = SpecIteratorBuilder::new_with_start("YY-MM-15 ! YY-[07-08]-DD", WeekendSkipper::new(), start)
//!     .with_count(12)
//!     .build()?
//!     .map(|o| Ok(o.observed().format("%Y-%m").to_string()))
//!     .collect()?;
//! assert_eq!(dates.len(), 12);
//! assert_eq!(dates[6], "2025-09");
//! assert_eq!(dates[11], "2026-02");
//! # Ok::<(), tkone_schedule::Error>(())
//! ```
//!
//! ### Checking a spec — `describe()`
//!
//! Every spec type has a `describe()` that renders it in plain English, so a
//! spec can be shown to a user, or checked in a review, before it is
//! scheduled. It catches near-misses like `HH:MM:00` vs `1H:MM:00`:
//!
//! ```rust
//! use tkone_schedule::{date, datetime, time};
//! let t = |s: &str| s.parse::<time::Spec>().unwrap().describe();
//! assert_eq!(t("HH:MM:00"), "every minute, at second 00");
//! assert_eq!(t("1H:MM:00"), "every hour, at the start's minute and second 00");
//!
//! let d: date::Spec = "YY-MM-L~PB".parse().unwrap();
//! assert_eq!(d.describe(), "on the last day of every month, moved to the previous business day when it is not one");
//!
//! let dt: datetime::Spec = "YY-MM-FRI#LT16:30:00".parse().unwrap();
//! assert_eq!(dt.describe().unwrap(), "on the last Friday of every month; at 16:30:00");
//! ```
//!
//! "The start" in a description is the iterator's anchor: the `new_with_start`
//! start or the `new_after` / `resume_after` cursor.
//!
//! ## `Occurrence` and Business Day Adjustments
//!
//! Date and datetime iterators yield [`Occurrence<T>`] rather than plain `T`.
//! This distinguishes unadjusted occurrences from ones where the business day
//! rule moved the settlement date:
//!
//! - [`Occurrence::Exact`] — no adjustment; `actual == observed`.
//! - [`Occurrence::AdjustedEarlier`] — rule moved the date *earlier*.
//! - [`Occurrence::AdjustedLater`] — rule moved the date *later*.
//!
//! Use `.observed()` for the settlement date and `.actual()` for the raw
//! calendar date.
//!
//! ## Daylight Saving Time
//!
//! Schedules are computed on the wall clock (like cron), then mapped to real
//! instants in the iterator's timezone.
//!
//! - **Gaps** (spring-forward, skipped days): a local time that does not exist
//!   moves later by the length of the gap. Date and datetime iterators report it
//!   as `AdjustedLater(label, shifted)`, where `label` shows the time as
//!   scheduled and `shifted` the time it is observed at. Both are the same
//!   instant.
//! - **Overlaps** (fall-back): the repeated hour is visited once, at the earlier
//!   offset.
//! - **No duplicates**: results strictly increase; a later result that lands on
//!   the same instant is dropped.
//!
//! [`DstPolicy::Strict`] turns gaps and overlaps into errors instead. On the
//! time and datetime iterators, [`IntervalMode::Elapsed`] measures steps in
//! real elapsed time rather than on the wall clock.
//!
//! ```text
//! "1H:00:00" in America/New_York, 2025-03-09 (02:00 → 03:00):
//!   00:00 EST, 01:00 EST, 03:00 EDT, 04:00 EDT
//!
//! "YY-MM-DD" in Pacific/Apia, which skipped 2011-12-30:
//!   Exact(Dec 29), AdjustedLater(Dec 30 -10:00, Dec 31 +14:00), Exact(Jan 1)
//! ```
//!
//! ## Persisting and resuming a schedule
//!
//! An application that stores schedules in a database does not keep an
//! iterator alive. It stores the next occurrence, runs the action when that is
//! due, then computes the following occurrence with `resume_after` from what it
//! stored. The result is the same series a single long-running iterator would
//! produce.
//!
//! Per schedule, store:
//!
//! | Column | Value | Used for |
//! |--------|-------|----------|
//! | spec | `"YY-1M-L~NBT17:00:00"` | rebuilding the iterator |
//! | timezone | IANA name, e.g. `"America/New_York"` (not an offset) | rebuilding the iterator |
//! | options | `TimeAnchor`, `IntervalMode`, `DstPolicy`, `with_exclusions`, `with_count` if set | rebuilding with the same behaviour |
//! | cursor | `occurrence.actual().to_rfc3339()` | resuming |
//! | due at | `occurrence.observed()` | when to run the action |
//! | end (optional) | an RFC 3339 end datetime | `.with_end(end)` when resuming |
//! | start (for unions, positions, counts) | the schedule's start, RFC 3339 — unless the spec has `@start` | `resume_with_start` — required for a union, a cycle with positions (`/4W[1,2]`, `8D[1-4]`) or `with_count` |
//!
//! ```rust
//! # use tkone_schedule::biz_day::WeekendSkipper;
//! # use tkone_schedule::datetime::SpecIteratorBuilder;
//! # use tkone_schedule::Occurrence;
//! # use chrono::{DateTime, TimeZone};
//! # use chrono_tz::Tz;
//! # use fallible_iterator::FallibleIterator;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! struct Row {
//!     spec: String,
//!     tz: String,
//!     cursor: String, // RFC 3339 `actual` of the stored occurrence
//!     due_at: String, // RFC 3339 `observed`: when the action runs
//! }
//!
//! impl Row {
//!     fn set(&mut self, occ: &Occurrence<DateTime<Tz>>) {
//!         self.cursor = occ.actual().to_rfc3339();
//!         self.due_at = occ.observed().to_rfc3339();
//!     }
//!
//!     /// After the action has run: store the next occurrence, or report that
//!     /// the schedule has finished.
//!     fn advance(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
//!         let tz: Tz = self.tz.parse()?;
//!         let cursor = DateTime::parse_from_rfc3339(&self.cursor)?;
//!         let next = SpecIteratorBuilder::resume_after(&self.spec, WeekendSkipper::new(), tz, cursor)
//!             .build()?
//!             .next()?;
//!         Ok(match next {
//!             Some(occ) => { self.set(&occ); true }
//!             None => false,
//!         })
//!     }
//! }
//!
//! // Creating the schedule: store its first occurrence.
//! let tz = chrono_tz::America::New_York;
//! let created = tz.with_ymd_and_hms(2025, 5, 20, 12, 0, 0).unwrap();
//! let spec = "YY-1M-L~NBT17:00:00"; // last day of the month, next business day, 17:00
//! let first = SpecIteratorBuilder::new_after(spec, WeekendSkipper::new(), created)
//!     .build()?
//!     .next()?
//!     .expect("the spec has occurrences");
//! let mut row = Row { spec: spec.into(), tz: tz.name().into(), cursor: String::new(), due_at: String::new() };
//! row.set(&first);
//! // May 31 2025 is a Saturday: due Monday Jun 2, cursor stays on May 31
//! assert_eq!(row.due_at, "2025-06-02T17:00:00-04:00");
//! assert_eq!(row.cursor, "2025-05-31T17:00:00-04:00");
//!
//! // When the action has run:
//! assert!(row.advance()?);
//! assert_eq!(row.due_at, "2025-06-30T17:00:00-04:00");
//! # Ok(())
//! # }
//! ```
//!
//! Rules that keep resumed results exact:
//!
//! - **Resume from `actual`, never `observed`.** With `~PB`, Saturday's
//!   observed date is Friday; resuming from Friday would yield Saturday again,
//!   forever. With daily `~NB`, resuming from observed Monday would skip
//!   Sunday's occurrence.
//! - **Store `actual` with its UTC offset** (RFC 3339, or a timestamp plus
//!   offset). An occurrence moved by a DST gap is stored as its scheduled
//!   wall-clock time — `2025-03-09T02:30:00-05:00` for a daily 02:30 that
//!   happened at 03:30 EDT — so the schedule resumes at 02:30, not 03:30. The
//!   offset also tells fall-back's two 01:00s apart.
//! - **Rebuild with the same options** (`with_time_anchor`,
//!   `with_interval_mode`, `with_dst_policy`) every time.
//! - **`Ok(None)` means the schedule has finished**: a finite spec ran out
//!   (e.g. `2025-MM-01` after December), or the next occurrence is after the
//!   end you passed with `.with_end(end)` (inclusive). Store an end as a
//!   resolved datetime; `with_end_spec` is resolved from the original start, so
//!   it is not available when resuming.
//! - Cadences (`3M`, `2Y`, `1Y-3M`) and step rules (`7D`, `5BD`) stay aligned,
//!   because every stored occurrence lies on the cadence.
//! - **A spec with `@start` written in it resumes with `resume_after` alone**,
//!   whatever it contains: the start comes from the text. The rules below are
//!   for specs without one.
//! - **A union spec (`A | B`) needs its start.** A stored occurrence belongs to
//!   one part, and the other parts' cadences, steps and time grids are not
//!   aligned to it. Store the start (the `new_with_start` start, or the first
//!   occurrence's `actual`) and resume with
//!   `resume_with_start(spec, bdp, tz, start, cursor)`, which positions every
//!   part as if iterating from the start. `resume_after` on a union returns
//!   [`Error::UnsupportedResume`], and so does a union in
//!   [`IntervalMode::Elapsed`]. `resume_with_start` also works for plain specs,
//!   with the same results as `resume_after`.
//!
//! - **So does a cycle with positions** other than the first (`/4W[1,2]`,
//!   `8D[1-4]`, `3M[1-2]`): a stored occurrence can be anywhere in its cycle.
//! - **A counted schedule (`with_count`) needs its start too.** Resume it with
//!   `resume_with_start`, which replays it from the start (at most *n*
//!   results) to know how many have run; this works for any spec and mode,
//!   including a union in `Elapsed`. `resume_after` with `with_count` returns
//!   [`Error::UnsupportedResume`].
//!
//! The same `resume_after` and `resume_with_start` exist on
//! [`date::SpecIteratorBuilder`] and [`time::SpecIteratorBuilder`].

/// The `biz_day` module contains the [`biz_day::BizDayProcessor`] trait and
/// built-in implementations for business day calculations.
pub mod biz_day;
/// The `date` module provides calendar-day recurrence via [`date::Spec`] and
/// [`date::SpecIteratorBuilder`].
pub mod date;
/// The `datetime` module combines a date spec and a time spec into a single
/// recurrence schedule via [`datetime::Spec`] and [`datetime::SpecIteratorBuilder`].
pub mod datetime;
/// The `schedule` module puts rules in context: a start and time zone,
/// business days, an end or count, several named rules and exclusions, via
/// [`schedule::Schedule`].
pub mod schedule;
/// The `time` module provides intra-day time recurrence via [`time::Spec`] and
/// [`time::SpecIteratorBuilder`].
pub mod time;

mod bounds;
mod error;
mod limits;
mod merge;
mod prelude;
mod utils;

#[cfg(test)]
mod resume_tests;
#[cfg(test)]
mod union_tests;
#[cfg(test)]
mod limits_tests;
#[cfg(test)]
mod bounds_tests;
#[cfg(test)]
mod grammar_tests;
#[cfg(test)]
mod schedule_tests;

pub use error::{Error, Result};

/// The IANA time zone written in a spec, after its start or end
/// (`09:00:00 @2026-01-05[Europe/London]`), if it names one.
///
/// ```rust
/// assert_eq!(
///     tkone_schedule::time_zone_of("09:00:00 @2026-01-05[Europe/London]").unwrap(),
///     Some(chrono_tz::Europe::London)
/// );
/// assert_eq!(tkone_schedule::time_zone_of("1H:00:00").unwrap(), None);
/// ```
///
/// # Errors
///
/// Returns an error if the start, end, count or zone is malformed.
pub fn time_zone_of(spec: &str) -> Result<Option<chrono_tz::Tz>> {
    Ok(bounds::split_bounds(spec)?.1.zone())
}

/// The holiday calendar a spec names (`[cal=GBLO+USNY]`), as written, if it
/// names one. A schedule that names a calendar only builds with that
/// calendar as its business-day processor (see
/// [`BizDayProcessor::code`](biz_day::BizDayProcessor::code)).
///
/// ```rust
/// assert_eq!(
///     tkone_schedule::calendar_of("YY-MM-BD#3 ~MF [cal=GBLO+USNY]").unwrap().as_deref(),
///     Some("GBLO+USNY")
/// );
/// assert_eq!(tkone_schedule::calendar_of("YY-MM-BD#3").unwrap(), None);
/// ```
///
/// # Errors
///
/// Returns an error if the calendar, start, end or count is malformed.
pub fn calendar_of(spec: &str) -> Result<Option<String>> {
    Ok(bounds::split_bounds(spec)?.1.calendar())
}

/// Controls how timezone-aware iterators resolve local datetimes that fall in
/// a DST transition window.
///
/// Applies to [`date::SpecIterator`], [`time::SpecIterator`], and
/// [`datetime::SpecIterator`] whenever a naive datetime produced by the
/// schedule spec must be mapped to an unambiguous `DateTime<Tz>`.
///
/// Configure via `.with_dst_policy(…)` on any [`SpecIteratorBuilder`](date::SpecIteratorBuilder).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DstPolicy {
    /// Silently resolve DST edge cases — the default.
    ///
    /// - **Spring-forward gap** (e.g. `02:30` does not exist): move later by
    ///   the length of the gap (`02:30` → `03:30` across a 1-hour gap). Works
    ///   for gaps of any length, including skipped days.
    /// - **Fall-back overlap** (e.g. `01:30` is ambiguous): take the earliest
    ///   UTC offset (still in summer time).
    #[default]
    Adjust,
    /// Return [`Error::AmbiguousLocalTime`] instead of silently resolving.
    Strict,
}

/// How the steps of a time spec (`nH`, `nM`, `nS`, or a driving `HH` / `MM` /
/// `SS`) are measured across DST changes.
///
/// Configure via `.with_interval_mode(…)` on [`time::SpecIteratorBuilder`] or
/// [`datetime::SpecIteratorBuilder`]. In a datetime spec the first time on
/// each date still comes from the wall clock; later times that day are whole
/// steps of real time after the previous one.
///
/// `1H:00:00` in New York from 00:00, across spring-forward (2025-03-09) and
/// fall-back (2025-11-02):
///
/// | Mode | Spring-forward | Fall-back |
/// |------|----------------|-----------|
/// | `WallClock` | 00:00, 01:00, 03:00, 04:00 | 00:00, 01:00 EDT, 02:00 EST |
/// | `Elapsed` | 00:00, 01:00, 03:00, 04:00 | 00:00, 01:00 EDT, 01:00 EST, 02:00 EST |
///
/// The difference shows on longer steps: `6H:00:00` from 06:00 on the day
/// before spring-forward runs 06, 12, 18, 00, 06 in `WallClock` (the last step
/// is 5 real hours) but 06, 12, 18, 00, **07** in `Elapsed`, staying an hour
/// off the clock until fall-back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IntervalMode {
    /// Steps move the wall clock, like cron — the default.
    ///
    /// A step into a spring-forward gap is resolved by [`DstPolicy`];
    /// fall-back's repeated hour is visited once.
    #[default]
    WallClock,
    /// Steps measure real elapsed time: `1H` is always 3600 seconds later.
    ///
    /// Stepped times never fall in a gap, so [`DstPolicy`] does not apply to
    /// them. Fixed fields (`09`, `30`) are read in the UTC offset of the
    /// previous occurrence.
    Elapsed,
}

/// Where a time spec's steps and carried fields are counted from.
///
/// Configure via `.with_time_anchor(…)` on [`datetime::SpecIteratorBuilder`]
/// or [`time::SpecIteratorBuilder`]. `Midnight` gives cron-like clock
/// alignment: `HH:15M:00` runs at :00, :15, :30 and :45 whatever the start.
///
/// Starting 2025-01-15 09:22:45:
///
/// | Time spec | `Start` (each date) | `Midnight` (each date) |
/// |-----------|---------------------|------------------------|
/// | `1H:MM:SS` | 00:22:45, 01:22:45, … | 00:00:00, 01:00:00, … |
/// | `2H:00:00` | 01:00, 03:00, …, 23:00 | 00:00, 02:00, …, 22:00 |
/// | `_:_:_` | 09:22:45 | 00:00:00 |
///
/// On the start date itself only times after the start are produced. A
/// time-only iterator with `Midnight` behaves the same way across days: each
/// day's times restart from that day's midnight (`5H:00:00` gives 00:00,
/// 05:00, … 20:00, then 00:00 the next day).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeAnchor {
    /// Every date follows the start's time of day: steps keep its phase and
    /// `ForEach` / `AsIs` fields carry its values — the default.
    #[default]
    Start,
    /// Every date (including the start date) restarts the time spec from
    /// 00:00:00: steps are aligned to midnight and carried fields are zero.
    Midnight,
}

/// Outcome of a single scheduling step, distinguishing raw calendar dates from
/// business-day-adjusted settlement dates.
///
/// # Variants
///
/// | Variant | Condition | `actual()` | `observed()` |
/// |---------|-----------|------------|--------------|
/// | `Exact(t)` | No adjustment applied | `t` | `t` |
/// | `AdjustedEarlier(a, o)` | Settlement moved earlier | `a` | `o` (`o < a`) |
/// | `AdjustedLater(a, o)` | Settlement moved later | `a` | `o` (`o > a`) |
///
/// # DST gaps
///
/// When the scheduled local time does not exist (spring-forward, or a skipped
/// day such as 2011-12-30 in Pacific/Apia) and [`DstPolicy::Adjust`] moves it
/// past the gap, the date and datetime iterators report
/// `AdjustedLater(label, shifted)`. `label` shows the time as scheduled, with
/// the offset in force before the gap (`2011-12-30 00:00 -10:00`); `shifted`
/// shows it in the real offset (`2011-12-31 00:00 +14:00`). Both are the same
/// instant, so `a == o` here; they differ only on the wall clock.
///
/// # Examples
///
/// ```rust
/// use tkone_schedule::Occurrence;
/// use chrono::{NaiveDate, NaiveDateTime};
///
/// let actual   = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap().and_hms_opt(0,0,0).unwrap();
/// let observed = NaiveDate::from_ymd_opt(2024, 3, 29).unwrap().and_hms_opt(0,0,0).unwrap(); // Fri
///
/// let result = Occurrence::AdjustedEarlier(actual, observed);
/// assert_eq!(result.actual(),   &actual);
/// assert_eq!(result.observed(), &observed);
/// assert!(result.observed() < result.actual());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Occurrence<T: Clone> {
    /// No business day adjustment was necessary; the raw calendar date is the
    /// settlement date.
    Exact(T),
    /// The business day rule shifted the settlement date *later* than the raw
    /// calendar date. The first field is the raw date; the second is the
    /// settlement date.
    AdjustedLater(T, T),
    /// The business day rule shifted the settlement date *earlier* than the raw
    /// calendar date. The first field is the raw date; the second is the
    /// settlement date.
    AdjustedEarlier(T, T),
}

impl<T: Clone> Occurrence<T> {
    /// Returns the inner value if this is a [`Occurrence::Exact`], otherwise `None`.
    pub fn exact(self) -> Option<T> {
        match self {
            Occurrence::Exact(t) => Some(t),
            _ => None,
        }
    }

    /// The chronologically earlier of the two dates.
    ///
    /// - `Exact(t)` → `t`
    /// - `AdjustedEarlier(a, o)` → `o` (settlement is earlier than raw)
    /// - `AdjustedLater(a, o)` → `a` (raw is earlier than settlement)
    pub fn earlier(&self) -> &T {
        match self {
            Occurrence::Exact(t)
            | Occurrence::AdjustedEarlier(_, t)
            | Occurrence::AdjustedLater(t, _) => t,
        }
    }

    /// The chronologically later of the two dates.
    ///
    /// - `Exact(t)` → `t`
    /// - `AdjustedEarlier(a, o)` → `a` (raw is later than settlement)
    /// - `AdjustedLater(a, o)` → `o` (settlement is later than raw)
    pub fn later(&self) -> &T {
        match self {
            Occurrence::Exact(t)
            | Occurrence::AdjustedEarlier(t, _)
            | Occurrence::AdjustedLater(_, t) => t,
        }
    }

    /// The settlement (post-adjustment) date.
    ///
    /// This is the date on which the event is *observed* — i.e. the result of
    /// applying any business day rule. Equal to `actual()` when no adjustment
    /// was made ([`Occurrence::Exact`]).
    pub fn observed(&self) -> &T {
        match self {
            Occurrence::Exact(t)
            | Occurrence::AdjustedEarlier(_, t)
            | Occurrence::AdjustedLater(_, t) => t,
        }
    }

    /// The raw calendar date before any business day adjustment.
    ///
    /// Equal to `observed()` when no adjustment was made ([`Occurrence::Exact`]).
    pub fn actual(&self) -> &T {
        match self {
            Occurrence::Exact(t)
            | Occurrence::AdjustedEarlier(t, _)
            | Occurrence::AdjustedLater(t, _) => t,
        }
    }

    /// Returns `(actual, observed)` as a tuple.
    ///
    /// For [`Occurrence::Exact`] both elements are the same reference.
    pub fn as_tuple(&self) -> (&T, &T) {
        match self {
            Occurrence::Exact(t) => (t, t),
            Occurrence::AdjustedEarlier(actual, adjusted)
            | Occurrence::AdjustedLater(actual, adjusted) => (actual, adjusted),
        }
    }
}
