# tkone schedule format: grammar and rules

This document defines the schedule format precisely enough to validate it, or to implement it in
another language. It has four parts:

1. [Grammar](#1-grammar): what a well-formed schedule looks like.
2. [Validity rules](#2-validity-rules): well-formed schedules that are still rejected.
3. [Evaluation rules](#3-evaluation-rules): which occurrences a schedule produces.
4. [Conformance examples](#4-conformance-examples): accepted and rejected inputs.

For a gentler introduction, read [INTRODUCTION.md](INTRODUCTION.md). The Rust library in this
crate is the reference implementation; where this document and the library disagree, that is a
bug in one of them.

---

## 1. Grammar

### Notation

The grammar is written in ABNF ([RFC 5234](https://www.rfc-editor.org/rfc/rfc5234)), with
case-sensitive strings written `%s"…"` ([RFC 7405](https://www.rfc-editor.org/rfc/rfc7405)).
`DIGIT` and `SP` are the RFC 5234 core rules.<!-- not-in-draft --> The ABNF blocks
below are copied from [ietf/tkone-schedule.abnf](ietf/tkone-schedule.abnf), the single source of
the grammar; run `python3 ietf/build.py` after changing it.<!-- /not-in-draft -->

- Matching is **case-sensitive**: `YY-MM-DD`, not `yy-mm-dd`.
- **No whitespace is allowed** inside a date, time or datetime spec, except where `OWS`
  (optional whitespace) is shown.
- **Where alternatives overlap, the first one listed applies.** ABNF itself does not order
  alternatives, so an implementation that tries them in a different order can accept the same
  text but read it differently. For example, `BD#3` is a business-day position, not the start of
  a `3BD` step.

### Shared tokens

<!-- abnf:shared -->
```abnf
number        = 1*DIGIT
span          = number [ "-" number ]
list          = "[" span *( "," span ) "]"
OWS           = *SP
```
<!-- /abnf -->

### Schedule

A schedule is exactly one rule: a date spec, a time spec or a datetime spec. The kind of
schedule (date, time or datetime) is decided by the caller, not by the text.

<!-- abnf:schedule -->
```abnf
schedule      = datetime-spec / date-spec / time-spec
```
<!-- /abnf -->

Everything around the rule is supplied with it, not written in it: the start it is counted
from, the time zone it is read in, the holiday calendar its business days come from, an end or a
number of occurrences, other rules it is combined with, and dates it leaves out.<!-- not-in-draft -->
In this crate they are set on the `schedule::Schedule` builder or written in a schedule document
(YAML or JSON); see [docs/schedule-document.md](../../docs/schedule-document.md).<!-- /not-in-draft -->

### Date spec

<!-- abnf:date -->
```abnf
date-spec     = years "-" months "-" days [ weeks ]
                [ OWS adjustment ]

years         = list / %s"YY" / "_" / number %s"Y" [ list ] / number
months        = list / %s"MM" / "_" / number %s"M" [ list ] / number

days          = easter
              / weekday-list
              / list
              / biz-nth
              / weekday "#" occurrence
              / weekday
              / number %s"D" list
              / number ( %s"BD" / %s"WD" / %s"D" )
              / %s"DD" / "_" / %s"L"
              / number [ %s"L" / %s"N" / %s"O" ]

weekday       = %s"MON" / %s"TUE" / %s"WED" / %s"THU" / %s"FRI"
              / %s"SAT" / %s"SUN"
weekday-span  = weekday [ "-" weekday ]
weekday-list  = "[" weekday-span *( "," weekday-span ) "]"
occurrence    = %s"L" / number [ %s"L" ]

biz-nth       = ( %s"BD" / %s"WD" ) "#"
                ( occurrence / "[" biz-pos *( "," biz-pos ) "]" )
biz-pos       = %s"L" / number %s"L" / span

easter        = ( %s"OEAS" / %s"EAS" ) [ ( "+" / "-" ) number ]

weeks         = "/" number %s"W" [ list ]

adjustment    = "~" ( [ %s"N" / %s"P" ] weekday
                    / %s"SB" / %s"MF" / %s"MP" / %s"PW" / %s"NW"
                    / %s"PB" / %s"NB" / %s"B" / %s"W"
                    / [ number ] ( %s"P" / %s"N" ) )
```
<!-- /abnf -->

An adjustment that names a weekday (`~MON`, `~NMON`, `~PMON`) is tried before the others, so
`~NWED` is the next Wednesday and `~NW` the next weekday.

`number %s"D" list` is only valid for calendar days: `5BD[…]` and `3WD[…]` do not parse.

`easter` counts calendar days from Easter Sunday: `EAS` is Western Easter, `OEAS` Orthodox
Easter. `EAS-2` is Good Friday and `EAS+1` Easter Monday. The offset needs its sign: `EAS2` does
not parse.

### Time spec

<!-- abnf:time -->
```abnf
time-spec     = hour-field ":" minute-field ":" second-field
hour-field    = values / %s"HH" / "_" / number %s"H" / number
minute-field  = values / %s"MM" / "_" / number %s"M" / number
second-field  = values / %s"SS" / "_" / number %s"S" / number
values        = list / number "-" number
```
<!-- /abnf -->

In each field, the forms are, in order: a list or range (*values*), the wildcard (*for each*),
`_` (*as is*), a step (*every n*), and a single fixed value (*at*). A list or range that names a
single value is the same as that value: `[09]` and `09-09` both mean `09`.

### Datetime spec

<!-- abnf:datetime -->
```abnf
datetime-spec = date-spec %s"T" time-spec
```
<!-- /abnf -->

The `T` that separates the parts is the **first** `T` that is immediately followed by the start
of a time spec, described by this regular expression:

```text
T(?:HH|_|\[[0-9,\-]+\]|[0-9]{1,2}H|[0-2]?[0-9](?:-[0-2]?[0-9])?):
```

This keeps the `T` in `TUE`, `THU` and `SAT` from being taken as the separator. `YY-MM-THUT09:00:00`
splits into `YY-MM-THU` and `09:00:00`.

---

## 2. Validity rules

A schedule that matches the grammar is still rejected if any of these rules is broken.

### Date specs

| Rule | Rejected example |
|------|------------------|
| A year or month step (`nY`, `nM`) is at least 1 | `0Y-MM-01`, `YY-0M-01` |
| Months are 1–12 | `YY-13-01`, `YY-[01-13]-01` |
| Days of the month are 1–31 | `YY-MM-32`, `YY-MM-[00-05]` |
| A day step (`nD`, `nBD`, `nWD`) is at least 1 | `YY-MM-0D` |
| A weekday occurrence (`DAY#n`, `DAY#nL`) is 1–5 | `YY-MM-MON#0`, `YY-MM-FRI#6` |
| A business-day position (`BD#n`, `BD#nL`, `BD#[…]`) is 1–23 | `YY-MM-BD#0`, `YY-MM-BD#24` |
| A range runs forwards | `YY-[06-01]-01`, `YY-MM-[FRI-MON]`, `YY-MM-BD#[3-1]` |
| A range in brackets covers at most 10,000 values | `[1-4000000000]-MM-01` |
| A cycle with positions (`nY[…]`, `nM[…]`, `nD[…]`, `/nW[…]`) has a length of at least 1 and positions within 1…n | `YY-3M[4]-15`, `YY-MM-8D[0]`, `YY-MM-MON/2W[3]` |
| A year cycle with positions is not combined with a month step or month cycle | `4Y[1]-3M-15` |
| A month cycle with positions is not combined with a year step | `1Y-3M[1]-15` |
| A week cycle does not follow a day step | `YY-MM-7D/2W` |
| An Easter offset is -80 to +250 days | `YY-MM-EAS-81`, `YY-MM-EAS+251` |
| A week cycle does not follow an Easter rule | `YY-MM-EAS/2W` |

### Time specs

| Rule | Rejected example |
|------|------------------|
| A fixed or listed hour is 0–23; a minute or second is 0–59 | `24:00:00`, `HH:60:00`, `[09,24]:00:00` |
| A step (`nH`, `nM`, `nS`) is at least 1 and at most 255 | `0H:00:00`, `HH:0M:00`, `300H:00:00` |
| A range runs forwards | `16-09:00:00` |

A step may exceed the field's range: `25H:00:00` and `HH:90M:00` are valid.

---

## 3. Evaluation rules

These rules decide which occurrences a valid schedule produces. Each occurrence has an **actual**
time (what the rule produced) and an **observed** time (after any `~` adjustment); they differ
only when an adjustment moved it.

### 3.1 The start

Every schedule is evaluated from a start, which the system running the schedule supplies.
Cadences (`nY`, `nM`), cycles with positions (`nY[…]`, `nM[…]`, `nD[…]`, `/nW[…]`), day steps
(`nD`, `nBD`, `nWD`) and time steps all count from the start. The start is an inclusive lower
bound: the first occurrence is the first one at or after it.

### 3.2 Date specs

A date spec is evaluated in layers:

1. **Eligible months.** The year rule and the month rule decide which calendar months are
   eligible:
   - `YY` / `MM` / `_`: every year / every month.
   - A number or list: only those years or months.
   - `nY` / `nM`: every *n*th year or month, counted from the start's year or month.
     `nY-mM` together is a single cadence of `12n + m` months (`1Y-3M` = every 15 months).
   - `nY[…]` / `nM[…]`: the listed positions of an *n*-long cycle; the start's year or month is
     position 1.
2. **Days.** The day rule picks dates in eligible months:
   - **Calendar rules** depend only on the month: `DD`, a day number or list, `L`, `31L` / `31N` /
     `31O`, weekdays, `DAY#n`, `BD#n`, `BD#[…]`, `nD[…]` and Easter rules.
     - A day the month does not have is skipped (`31`), clamped to the last day (`31L`), moved to
       the 1st of the next month (`31N`), or carried into the next month by the missing days
       (`31O`). With `N` and `O` the occurrence is *adjusted later*: actual is the last day,
       observed the next-month date.
     - `BD#…` counts the month's business days using the holiday calendar; `WD#…` counts
       weekdays (Monday to Friday). A position the month does not have produces nothing.
     - `nD[…]` keeps days whose position in an *n*-day cycle from the start is listed.
     - `EAS±n` / `OEAS±n` is Easter Sunday plus or minus *n* calendar days, with Easter Sunday
       computed by the Gregorian computus (`EAS`) or the Julian computus given as a Gregorian
       date (`OEAS`). It is one date a year, kept when it falls in an eligible month: `YY-04-EAS`
       is Easter Sunday only in the years it is in April. The offset range keeps every Western
       Easter date in its own year; a late Orthodox Easter plus a large offset can fall in
       January of the next year.
   - **Step rules** (`nD`, `nBD`, `nWD`) advance from the previous occurrence. With an `nM` month
     cadence, the month step and the day step form a single clock (`YY-1M-7D` = +1 month and +7
     days each time). Otherwise the step continues while it stays in eligible months, and
     restarts on the first day (or first business day) of the next eligible month after a gap.
3. **Weeks.** A week cycle keeps only dates in the listed weeks of each *n*-week cycle. Weeks run
   Monday to Sunday; week 1 is the week containing the start. `/nW` is `/nW[1]`.
4. **Adjustment.** `~NB`, `~PB`, `~B` move a date that is not a business day to the next, previous
   or nearest business day; `~NW`, `~PW`, `~W` do the same for weekends only, ignoring holidays.
   `~MF` and `~MP` move like `~NB` and `~PB`, but never into another month (they go the other way
   instead). `~nN` and `~nP` always move *n* business days later or earlier. `~SB` drops a date
   whose observed date is not a business day. `~DAY` (`DAY` is `MON` to `SUN`) moves a date that
   is not that day of the week to the nearest one, at most 3 days away (7 is odd, so there is
   never a tie); `~NDAY` and `~PDAY` move it to the next or previous one. These three count
   calendar days and ignore the holiday calendar: `YY-05-24~PMON` is the Monday on or before
   24 May. Only one adjustment applies per spec.

The schedule always continues from actual dates, so an adjustment never shifts the dates that
follow it. Every actual date produces at most one occurrence, even when several observed dates
coincide or go backwards.

### 3.3 Time specs

A time spec describes a grid of times:

- **Step.** The sum of the step fields (`1H:30M:00` = 90 minutes). With no step field, one unit
  of the finest wildcard field (`HH:MM:00` = 1 minute). The unit of the finest step field, or of
  that wildcard, is the *driver*.
- **Phase.** The grid passes through the start, with fixed fields finer than the driver applied.
  Finer wildcard and `_` fields keep the start's value. A list in a finer field gives one grid per
  value (`HH:[00,30]:00` is a grid at :00 and one at :30).
- **Window.** Fixed fields coarser than the driver limit the grid to those hours or minutes
  (`09-16:15M:00` = every 15 minutes, only from 09:00 to 16:59). Coarser wildcard and `_` fields
  are unrestricted.

A spec with no step and no wildcard names **fixed times of day**: every combination of its
fixed values, with `_` fields taken from the start. Times are read on the wall clock of the
schedule's time zone and have whole-second resolution.

### 3.4 Datetime specs

The date part produces dates; on each observed date, the time part produces times within that
day (00:00 to before the next midnight). An adjustment on the date applies to every time on it:
the actual time is on the actual date, the observed time on the observed date. By default the
time grid's phase comes from the start's time of day, the same on every date.

### 3.5 Holiday calendar and time zone

- The holiday calendar supplied with a schedule is used by every business-day rule: `BD#…`,
  `nBD`, `~NB`, `~PB`, `~B`, `~MF`, `~MP`, `~SB`, `~nN` and `~nP`. Weekday rules (`WD#…`, `nWD`,
  `~W`, `~NW`, `~PW`) only know weekends and do not use it, and `~DAY`, `~NDAY` and `~PDAY`
  count calendar days.
- Times are read on the wall clock of the time zone supplied with the schedule, and business
  days are judged on dates in that zone. When an implementation knows the calendar's own time
  zone, it SHOULD reject a datetime occurrence whose date there is a different date (02:00 UTC
  is the previous evening in New York), rather than judge the business day on the wrong date.

### 3.6 Clock changes

- A local time that does not exist (a spring-forward gap) moves later by the length of the gap,
  and the occurrence is *adjusted later*: actual is the scheduled wall-clock time, observed the
  time it actually happens.
- A local time that happens twice (fall-back) happens once, at the earlier offset.
- Steps move the wall clock: every 6 hours from 00:00 is 00:00, 06:00, 12:00 and 18:00, on every
  day.
- Two scheduled times that land on the same instant produce one occurrence.

---

## 4. Conformance examples

### Accepted

| Schedule | Kind |
|----------|------|
| `YY-MM-DD` · `_-_-_` · `YY-MM-_` · `YY-3-15` · `2025-6-1` | date |
| `YY-1M-31L~NB` · `YY-MM-15 ~NB` · `YY-MM-DD~P` · `YY-MM-L~2P` · `YY-MM-BD#3~MF` | date |
| `YY-01-29~MON` · `YY-05-24~PMON` · `YY-05-18~NMON` · `YY-MM-DD~NWED` · `YY-MM-15~SUN` | date |
| `YY-MM-[05]` · `YY-[01-03,12]-[01-05,15]` · `[2025-2027,2030]-12-L` | date |
| `YY-MM-[MON-WED,FRI]` · `YY-MM-WED#2` · `YY-MM-FRI#L` · `YY-MM-THU#2L` | date |
| `YY-MM-BD#3` · `YY-MM-BD#[1-3,2L,L]` · `YY-MM-WD#[1,L]` | date |
| `YY-MM-5BD` · `YY-1M-7D` · `YY-MM-8D[1-4]` · `YY-3M[1-2]-15` · `4Y[1,3]-06-15` | date |
| `YY-MM-[TUE,THU]/2W` · `YY-MM-[MON-FRI]/4W[1,2]~SB` | date |
| `YY-MM-EAS` · `YY-MM-EAS-2` · `YY-MM-OEAS+1~NB` · `YY-04-EAS` · `YY-MM-EAS+39` | date |
| `09:30:00` · `9:30:00` · `HH:30M:00` · `1H:30M:15S` · `25H:00:00` · `_:_:_` | time |
| `[09,12,17]:00:00` · `09-16:15M:00` · `0-23:00:00` · `[09-11,14]:[00,30]:00` | time |
| `YY-MM-DDT09:00:00` · `YY-MM-THUT9:00:00` · `YY-MM-SATT9-16:00:00` | datetime |
| `YY-MM-DD~NBT09:00:00` · `YY-MM-[TUE,THU]/2WT18:00:00` · `YY-MM-DDT_:_:_` | datetime |
| `YY-MM-EAS-2T09:00:00` · `YY-MM-BD#1T09:00:00` · `YY-05-24~PMONT09:00:00` | datetime |

### Rejected

| Schedule | Why |
|----------|-----|
| `yy-mm-dd` | case-sensitive |
| `YY-1Y-15` | a year step in the month field |
| `YY-MM-[MON,15]` | a list mixes weekdays and day numbers |
| `YY-MM-[31]L` | a suffix after a day list |
| `YY-MM-DD /2W` | whitespace before a week cycle |
| `YY-MM-DD~NA` | not an adjustment |
| `YY-MM-DD~XMON` · `YY-MM-DD~NMONDAY` · `YY-MM-DD~2MON` | not a weekday adjustment |
| `YY-MM-BD#[1-3L]` | a range counted from the end |
| `YY-MM-EAS2` · `YY-MM-EASTER` · `YY-MM-EAS+-1` | an Easter offset is a sign and a number |
| `HH:MM` | a time spec has three fields |
| `YY-MM-DD T09:00:00` | whitespace before `T` |
| `YY-MM-01 \| YY-MM-15` | one rule only: several rules belong to the schedule around it |
| `YY-MM-DD ! YY-12-25` | one rule only: exclusions belong to the schedule around it |
| `YY-MM-15 @2025-01-01` · `YY-MM-15 ..2025-12-31` · `YY-MM-15 ..+30Y` · `YY-MM-15 x3` | one rule only: the start, end and count belong to the schedule around it |
| `YY-MM-15 [cal=GBLO]` · `09:00:00 @2025-01-06T09:00[Europe/Paris]` | one rule only: the holiday calendar and time zone belong to the schedule around it |
| everything in the [validity rules](#2-validity-rules) tables | as stated there |
