# Date Schedule Specification

## Format

```text
YEAR-MONTH-DAY[/WEEKS][~ADJUSTMENT]
SPEC | SPEC | …  ! EXCLUSION | EXCLUSION | …  @START ..END xCOUNT
```

All three components are required. The optional `/WEEKS` suffix keeps only some weeks ([Week cycles](#week-cycles-optional)), and the optional `~ADJUSTMENT` suffix shifts the result to a nearby business day or weekday. Several specs can be combined with `|` ([Union](#union-optional)), dates removed after `!` ([Exclusions](#exclusions-optional)), and a start, end, time zone, count and calendar added at the end ([Start, end, time zone and calendar](#start-end-time-zone-and-calendar-optional)).

---

## Component Reference

### Year

| Syntax | Meaning |
|--------|---------|
| `YY` | Every year (wildcard) |
| `nY` | Every *n* years from the start year (e.g. `1Y`, `2Y`, `3Y`) |
| `nY[P,...]` | Years at positions *P* of every *n*-year cycle from the start year (e.g. `4Y[1,3]`) |
| `YYYY` | Specific year only (e.g. `2025`) |
| `[Y1,Y2,...]` | Enumerated years only (e.g. `[2025,2026]`) |
| `[Y1-Y2,...]` | Inclusive ranges inside a list (e.g. `[2025-2027]`, `[2025-2027,2030]`) |

### Month

| Syntax | Meaning |
|--------|---------|
| `MM` | Every month (wildcard) |
| `nM` | Every *n* months from the start month (e.g. `1M`, `3M`, `6M`) |
| `nM[P,...]` | Months at positions *P* of every *n*-month cycle from the start month (e.g. `3M[1-2]`) |
| `MM` (2-digit) | Specific month only (e.g. `01`, `06`, `12`) |
| `[M1,M2,...]` | Enumerated months only (e.g. `[01,06]`, `[03,09]`) |
| `[M1-M2,...]` | Inclusive ranges inside a list (e.g. `[01-06]`, `[01-03,10-12]`) |

### Day

| Syntax | Meaning |
|--------|---------|
| `DD` | Every calendar day |
| `nD` | Every *n* calendar days (e.g. `4D`, `7D`, `14D`) |
| `nD[P,...]` | Days at positions *P* of every *n*-day cycle from the start (e.g. `8D[1-4]` = four on, four off) |
| `nBD` | Every *n* business days (using configured processor) |
| `nWD` | Every *n* weekdays (Mon–Fri only, using WeekendSkipper) |
| `DD` (2-digit) | Fixed day-of-month (e.g. `01`, `15`) |
| `DDL` | Fixed day; clamp to last day of month on overflow (e.g. `31L`) |
| `DDN` | Fixed day; roll to 1st of next month on overflow (e.g. `31N`) |
| `DDO` | Fixed day; overflow into next month by remainder days (e.g. `31O`) |
| `L` | Last day of month |
| `DAY` | Every occurrence of a weekday, where `DAY` is `MON` … `SUN` (e.g. `MON`, `FRI`) |
| `[DAY,DAY,...]` | Every occurrence of any listed weekday (e.g. `[MON,WED,FRI]`) |
| `[DAY-DAY,...]` | Weekday ranges, Monday to Sunday, no wrap-around (e.g. `[MON-FRI]`, `[MON-WED,FRI]`) |
| `DAY#N` | *N*th occurrence of a weekday in the month (e.g. `MON#1`, `WED#2`) |
| `DAY#L` | Last occurrence of a weekday in the month (e.g. `FRI#L`) |
| `DAY#NL` | *N*th-from-last occurrence of a weekday in the month (e.g. `WED#2L`) |
| `BD#N` | *N*th business day of the month, using the configured processor (e.g. `BD#1`, `BD#3`) |
| `BD#L` | Last business day of the month |
| `BD#NL` | *N*th-from-last business day of the month (e.g. `BD#2L`) |
| `BD#[P,...]` | Several business-day positions: numbers and ranges from the start, `L` / `nL` from the end (e.g. `BD#[1-5]`, `BD#[1,L]`, `BD#[1-3,2L,L]`) |
| `WD#N` / `WD#L` / `WD#NL` / `WD#[…]` | Same as `BD#…`, counting weekdays (Mon–Fri) and ignoring holidays |
| `[D1,D2,...]` | Enumerated days of month (e.g. `[01,15]`, `[01,10,20,25]`) |
| `[D1-D2,...]` | Day ranges inside a list (e.g. `[01-10]`, `[01-05,15]`) |
| `EAS` / `EAS±n` | Western Easter Sunday, or *n* calendar days after (`+`) or before (`-`) it (e.g. `EAS-2` Good Friday, `EAS+1` Easter Monday, `EAS+39` Ascension) |
| `OEAS` / `OEAS±n` | The same for Orthodox Easter (e.g. `OEAS+1` Orthodox Easter Monday) |

Ranges are only written inside brackets, because `-` also separates the year, month and day.
They are inclusive and must run forwards (`[06-01]` and `[FRI-MON]` are rejected). When a spec is
written back out, consecutive values become ranges and lists are sorted: `[MON,TUE,WED,THU,FRI]`
is shown as `[MON-FRI]`, and `[15,01]` as `[01,15]`.

An Easter rule is one date a year. The offset is -80 to +250 days, which keeps every Western
Easter date in its own year, and the sign is required (`EAS2` is rejected). The month rule still
filters: `YY-04-EAS` is Easter Sunday only in the years it falls in April. Easter is computed
(Gregorian computus for `EAS`, Julian for `OEAS`, given as a Gregorian date), so it needs no
holiday data and works for any year.

### Business Day Adjustment (optional)

| Syntax | Meaning |
|--------|---------|
| `~NW` | If result is a weekend, move to **next** weekday (WeekendSkipper) |
| `~PW` | If result is a weekend, move to **previous** weekday (WeekendSkipper) |
| `~W`  | If result is a weekend, move to **nearest** weekday (Mon or Fri, WeekendSkipper) |
| `~NB` | If result is not a biz day, move to **next** biz day (configured processor) |
| `~PB` | If result is not a biz day, move to **previous** biz day (configured processor) |
| `~B`  | If result is not a biz day, move to **nearest** biz day (configured processor) |
| `~MF` | **Modified Following**: if not a biz day, move to the **next** biz day, unless that is in the next month — then the **previous** one |
| `~MP` | **Modified Preceding**: if not a biz day, move to the **previous** biz day, unless that is in the previous month — then the **next** one |
| `~SB` | **Skip**: if not a biz day, **drop** the date — no result, nothing moved (configured processor) |
| `~nN` | **Unconditional**: add *n* biz days (e.g. `~2N`) |
| `~nP` | **Unconditional**: subtract *n* biz days (e.g. `~3P`) |

`~NW`/`~PW`/`~W` always use the built-in WeekendSkipper (Sat/Sun only).  
`~NB`/`~PB`/`~B`/`~MF`/`~MP`/`~SB` use the configured `BizDayProcessor` and can account for custom holidays.  
`~nN`/`~nP` are unconditional — they shift every result regardless of day-of-week.

### Ways to use `~SB`

`~SB` drops any date that is not a business day, instead of moving it. It works on top of any
calendar day rule, so it turns "these dates" into "these dates, when they are business days".
The examples below use a holiday calendar where 26 May, 25 December and 26 December 2025 are
holidays, as well as every weekend.

| Use | Spec | Results |
|-----|------|---------|
| Every business day | `YY-MM-DD~SB` | Thu 22 May, Fri 23 May, Tue 27 May, … (weekend and 26 May skipped) |
| A fixed day, only when it is a business day | `YY-MM-26~SB` | Wed 26 Feb, Wed 26 Mar, Thu 26 Jun, … (Jan, Apr, May, Jul, Oct, Dec skipped) |
| Chosen weekdays, except holidays | `YY-MM-[MON,WED,FRI]~SB` | Mon 19 May, Wed 21 May, Fri 23 May, Wed 28 May, … (Mon 26 May skipped) |
| A weekday-of-month rule, only when it is a business day | `YY-MM-MON#L~SB` | Mon 27 Jan, Mon 24 Feb, …, Mon 28 Apr, Mon 30 Jun (Mon 26 May skipped) |
| Business days every other week | `YY-MM-DD/2W~SB` | Mon 19 – Fri 23 May, Mon 2 – Fri 6 Jun, … |
| A rotation, on business days only | `YY-MM-8D[1-4]~SB` | Thu 22, Fri 23 May, Fri 30 May, Mon 2 Jun, … (on-days that fall on weekends skipped) |
| A month-end rule, checked on the date it lands on | `YY-MM-31N~SB` | Fri 31 Jan, Mon 31 Mar, Thu 1 May, … (Sat 1 Mar and Sat 31 May skipped) |
| Business days at a time of day | `YY-MM-DD~SBT09:00:00` | Fri 23 May 09:00, Tue 27 May 09:00, … |
| Intraday, on business days only | `YY-MM-DD~SBT09-16:30M:00` | …, Fri 23 May 16:30, Tue 27 May 09:00, … (whole days skipped) |
| A fixed number of business-day occurrences | `YY-MM-26~SB @2025-01-01 x6` | six results, Feb to Nov; skipped dates are not counted |
| Business days, except a period | `YY-MM-DD~SB ! YY-12-[29-31]` | Mon 22, Tue 23, Wed 24 Dec, Thu 1 Jan, … |
| One part of a union | `YY-MM-[01,15]~SBT09:00:00 \| YY-MM-L~PBT17:00:00` | 1st and 15th at 09:00 when they are business days; last business day at 17:00 |

How it compares with the other ways of avoiding non-business days:

| You want | Use | Example |
|----------|-----|---------|
| The date to move to a business day | `~NB`, `~PB`, `~B`, `~MF`, `~MP` | `YY-MM-MON#L~NB`: 26 May becomes Tue 27 May |
| The date to move off weekends only, ignoring holidays | `~NW`, `~PW`, `~W` | `YY-MM-15~W` |
| The date to disappear when it is not a business day | `~SB` | `YY-MM-MON#L~SB`: no result for May |
| Particular dates to disappear, whatever the calendar says | `! <exclusion>` | `YY-MM-DD ! YY-12-[24-31]` |
| To count business days | `nBD`, `BD#n`, `BD#[…]` | `YY-MM-2BD`, `YY-MM-BD#[1-5]` |

Notes:

- **The observed date is checked.** With `31N` or `31O`, `~SB` looks at the date the rule lands
  on in the next month, not at the 31st.
- **It uses the business-day calendar**, holidays included. For weekends only, use a weekday
  list such as `[MON-FRI]` instead.
- **It has no effect on rules that already produce business days**: `nBD`, `BD#n`, `BD#[…]`.
- **One adjustment per spec.** `~SB` can't be combined with `~NB` in the same rule, for example
  "skip holidays but move weekends". Use a union of two rules instead, each with its own
  adjustment, or an exclusion.
- **Dropped dates are not counted** by `xN` or `with_count`, and a time part has no ticks on a
  dropped date.

> *Library:* a start given to the builder (`new_with_start`) is still returned as-is, even when it
> is not a business day. A start written in the spec (`@…`) is skipped like any other date.

### Week cycles (optional)

`/nW` after the day rule keeps only the dates in every *n*th week, and `/nW[P,...]` the dates in
weeks at positions *P* of every *n*-week cycle. Weeks run Monday to Sunday; week 1 is the week
containing the start, and the count carries across months and years.

| Spec | Meaning |
|------|---------|
| `YY-MM-[TUE,THU]/2W` | Tuesday and Thursday, every other week |
| `YY-MM-FRI/2W` | Every other Friday |
| `YY-MM-MON/2W[2]` | Mondays in the weeks *not* containing the start |
| `YY-MM-[MON-FRI]/4W[1,2]` | Weekdays in weeks 1 and 2 of every 4 (two on, two off) |

A week cycle works with any calendar day rule, adjustments and month or year filters, but not
after a day step (`7D/2W` is rejected).

**Weeks that begin on another day.** The week start only matters for a week cycle with several
days that include Sunday: `YY-MM-[SUN,TUE]/2W @2025-01-05` gives Sun 5, Tue 14, Sun 19, Tue 28
January, because each Sunday ends a Monday-to-Sunday week and its Tuesday falls in the next one.
A single day (`SUN/2W`) and plain weekday lists (`[MON-THU,SUN]`) are not affected. To use a
week that begins on Sunday, or any other day, count days from a start on that day with
[positions](#positions-inside-a-cycle): day *k* of the cycle is the start's weekday plus
*k* − 1.

| Spec | Meaning |
|------|---------|
| `YY-MM-14D[1,3] @2025-01-05` | Sunday and Tuesday, every other week (Sun 5, Tue 7, Sun 19, Tue 21, …) |
| `YY-MM-14D[1-5] @2025-01-05` | Sunday to Thursday, every other week |
| `YY-MM-21D[1-5] @2025-01-05` | Sunday to Thursday, one week in three |
| `YY-MM-14D[1-4] @2025-01-03` | Friday to Monday, every other week (the start is a Friday) |

### Positions inside a cycle

`nY[…]`, `nM[…]` and `nD[…]` pick positions of a cycle counted from the start (1 is the start's
year, month or day), written like any list (`[1,3]`, `[1-4]`, `[1,2,5-7]`). They express
rotations and shift patterns:

| Spec | Meaning |
|------|---------|
| `YY-MM-8D[1-4]` | Days 1–4 of every 8 (four on, four off) |
| `YY-MM-14D[1,2,5-7,10,11]` | The 2-2-3 pattern over 14 days |
| `YY-3M[1-2]-15` | The 15th in months 1 and 2 of every 3 |
| `4Y[1,3]-06-15` | 15 June in years 1 and 3 of every 4 |

Unlike a plain `nD` step, a day cycle with positions never restarts: a month filter
(`YY-[01-02]-8D[1-4]`) only removes dates. A year cycle with positions cannot be combined with a
month cycle, and `nBD[…]` / `nWD[…]` are not supported.

> *Library:* a cycle with positions other than the first (`/4W[1,2]`, `8D[1-4]`) can be at any
> position when an occurrence is stored, so resuming needs the schedule's start: written in the
> spec (`@…`), or given to `resume_with_start`.

## Combining and bounding specs

These apply to whole specs, after the date part: combine several with `|`, remove dates after
`!`, and end with a start, end, time zone and count.

### Union (optional)

`<spec> | <spec> | …` yields the dates of every part, in order: `YY-MM-BD#3 | YY-MM-L~PB` is the
3rd and the last business day of each month. A date produced by two parts is yielded once (the
first listed part's occurrence). Each part keeps its own cadence from the start, so resume a
union with `resume_with_start`, which also takes the schedule's start. Describe a union with
`date::describe(spec)`.

### Exclusions (optional)

`<spec> ! <exclusion>` drops the dates of `<spec>` that match `<exclusion>`, another date spec,
which can itself be a union: `YY-MM-DD ! YY-12-25 | YY-01-01` is every day except 25 December
and 1 January. A date is matched by its **actual** (unadjusted) date, and the exclusion applies
to everything before the `!`, including every part of a union. An exclusion spec cannot use day
steps (`7D`, `5BD`) or have its own `!`.

> *Library:* `.with_exclusions(spec)` on the builder adds exclusions the same way, combined with
> any written in the spec. `.with_count(n)` stops after `n` results counted from the start
> (excluded dates are not counted); a counted schedule is resumed with `resume_with_start`.

### Start, end, time zone and calendar (optional)

`<spec> @<start> ..<end>` ends the spec with a start and/or an end, each a date or datetime with
an optional UTC offset and an optional time zone in RFC 9557 brackets:

```text
YY-3M-15~NB @2025-02-15 ..2027-02-15
YY-MM-[TUE,THU]/2W @2025-01-07T09:00:00[Europe/London]
YY-MM-15 ..2025-12-31
```

- **`@start`** anchors the spec (cadences, week cycles, positions and steps count from it) and is
  an inclusive lower bound: the first result is the first match at or after it.
  `YY-MM-15 @2025-01-10` begins on 15 January. A step rule (`7D`) begins on the start itself,
  or on the next business day for `nBD`/`nWD`.
- **`..end`** is an inclusive upper bound; a date-only end covers the whole day.
- **`..+duration`** ends a duration after the start instead, the end itself excluded: years,
  months, weeks and days in that order (`..+30Y`, `..+5Y6M`, `..+2W`). `YY-1M-31L~MF @2025-01-31
  ..+30Y` is exactly 360 payments. Months are added like `31L` (2024-02-29 plus `1Y` is
  2025-02-28). It counts from the builder's start if there is one, else the written start; with
  neither, building fails with `Error::InvalidSpecBounds`.
- **`[Zone]`** names the IANA time zone the start and end are read in; a `+hh:mm` offset fixes a
  single instant instead.
- **`xN`** stops after *N* results, counted from the start (`YY-MM-01~MF @2025-04-01 x36`:
  36 payments). Excluded dates are not counted; with an end too, whichever comes first stops
  it. It is a separate, space-separated token, and works without a start (counted from the
  first result, or from a builder start), which makes the spec a reusable template.
- **`[cal=CODE]`** names the holiday calendar the spec's business days come from:
  `YY-MM-BD#3 ~MF [cal=GBLO]`, or several, all of which must be open, with `+`
  (`[cal=GBLO+USNY]`). It makes the text self-contained: stored or sent, it means the same dates
  everywhere. The builder fails with `Error::Calendar` unless the business-day processor it is
  given has the same code (`BizDayProcessor::code`); `tkone-calendar`'s
  `Calendars::for_spec(spec)` returns the right one. Without it, any processor is used, so the
  same text can serve several markets. Business days are judged on dates in the spec's time
  zone; a datetime spec whose run falls on another date in the calendar's own zone fails with
  `Error::Calendar` (see `BizDayProcessor::time_zone`), so write it in the calendar's zone.

They apply to the whole spec, after any `|` and `!`. With a written start, a stored schedule is
resumed with plain `resume_after`, even for a union, a cycle with positions or a count.

> *Library:* a start, end or count given to the builder (`new_with_start`, `resume_with_start`,
> `with_end`, `with_end_spec`, `with_count`) takes precedence over the text. `new_after(dtm)` and
> `resume_after(last)` take a cursor, not a start, so the written start still anchors the
> schedule. `SpecIteratorBuilder::from_spec(spec, bdp)` builds from the text alone, in its
> time zone (UTC if none is named).

---

## Result Types

- **`Exact(date)`** — the date is exactly as computed, no adjustment needed.
- **`AdjustedLater(actual, observed)`** — `actual` is the computed date; `observed` is a later adjusted date.
- **`AdjustedEarlier(actual, observed)`** — `actual` is the computed date; `observed` is an earlier adjusted date.

`AdjustedLater` is also used when the computed time falls in a DST gap — see [Daylight saving time](#daylight-saving-time).

---

## Checking a spec

> *Library behaviour: set when building the iterator, not written in the spec.*

[`Spec::describe`](crate::date::Spec::describe) renders a spec in plain English:

```text
YY-3M-15     →  on the 15th of every 3rd month, counted from the start's month
YY-MM-31     →  on the 31st of every month (skipped in shorter months)
YY-01-4D     →  every 4 days in January, counted from the start, restarting on the 1st of the next eligible month after a gap
YY-MM-L~2P   →  on the last day of every month, moved 2 business days earlier
```

---

## Examples

### 1. `YY-MM-DD` — Every calendar day

**Start:** 2025-01-01

```text
 1. 2025-01-01
 2. 2025-01-02
 3. 2025-01-03
 4. 2025-01-04
 5. 2025-01-05
 6. 2025-01-06
 7. 2025-01-07
 8. 2025-01-08
 9. 2025-01-09
10. 2025-01-10
11. 2025-01-11
12. 2025-01-12
13. 2025-01-13
14. 2025-01-14
15. 2025-01-15
```

---

### 2. `YY-MM-7D` — Every 7 calendar days (rolling, crosses month boundaries)

**Start:** 2025-01-01

```text
 1. 2025-01-01
 2. 2025-01-08
 3. 2025-01-15
 4. 2025-01-22
 5. 2025-01-29
 6. 2025-02-05
 7. 2025-02-12
 8. 2025-02-19
 9. 2025-02-26
10. 2025-03-05
11. 2025-03-12
12. 2025-03-19
13. 2025-03-26
14. 2025-04-02
15. 2025-04-09
```

---

### 3. `YY-MM-14D` — Every 14 calendar days (bi-weekly, rolling)

**Start:** 2025-01-01

```text
 1. 2025-01-01
 2. 2025-01-15
 3. 2025-01-29
 4. 2025-02-12
 5. 2025-02-26
 6. 2025-03-12
 7. 2025-03-26
 8. 2025-04-09
 9. 2025-04-23
10. 2025-05-07
11. 2025-05-21
12. 2025-06-04
13. 2025-06-18
14. 2025-07-02
15. 2025-07-16
```

---

### 4. `YY-MM-1BD` — Every business day

**Start:** 2025-01-01 (Wednesday)

```text
 1. 2025-01-01  Wed
 2. 2025-01-02  Thu
 3. 2025-01-03  Fri
 4. 2025-01-06  Mon  ← weekend skipped
 5. 2025-01-07  Tue
 6. 2025-01-08  Wed
 7. 2025-01-09  Thu
 8. 2025-01-10  Fri
 9. 2025-01-13  Mon
10. 2025-01-14  Tue
11. 2025-01-15  Wed
12. 2025-01-16  Thu
13. 2025-01-17  Fri
14. 2025-01-20  Mon
15. 2025-01-21  Tue
```

---

### 5. `YY-MM-5BD` — Every 5 business days (approx. weekly)

**Start:** 2025-01-01 (Wednesday)

```text
 1. 2025-01-01
 2. 2025-01-08
 3. 2025-01-15
 4. 2025-01-22
 5. 2025-01-29
 6. 2025-02-05
 7. 2025-02-12
 8. 2025-02-19
 9. 2025-02-26
10. 2025-03-05
11. 2025-03-12
12. 2025-03-19
13. 2025-03-26
14. 2025-04-02
15. 2025-04-09
```

---

### 6. `YY-MM-1WD` — Every weekday (Mon–Fri, WeekendSkipper)

**Start:** 2025-01-01 (Wednesday)

Identical to `1BD` when no custom holidays are configured.

```text
 1. 2025-01-01  Wed
 2. 2025-01-02  Thu
 3. 2025-01-03  Fri
 4. 2025-01-06  Mon
 5. 2025-01-07  Tue
 6. 2025-01-08  Wed
 7. 2025-01-09  Thu
 8. 2025-01-10  Fri
 9. 2025-01-13  Mon
10. 2025-01-14  Tue
11. 2025-01-15  Wed
12. 2025-01-16  Thu
13. 2025-01-17  Fri
14. 2025-01-20  Mon
15. 2025-01-21  Tue
```

---

### 7. `YY-1M-15` — 15th of every month

**Start:** 2025-01-15

```text
 1. 2025-01-15
 2. 2025-02-15
 3. 2025-03-15
 4. 2025-04-15
 5. 2025-05-15
 6. 2025-06-15
 7. 2025-07-15
 8. 2025-08-15
 9. 2025-09-15
10. 2025-10-15
11. 2025-11-15
12. 2025-12-15
13. 2026-01-15
14. 2026-02-15
15. 2026-03-15
```

---

### 8. `YY-1M-L` — Last day of every month

**Start:** 2025-01-31

```text
 1. 2025-01-31
 2. 2025-02-28
 3. 2025-03-31
 4. 2025-04-30
 5. 2025-05-31
 6. 2025-06-30
 7. 2025-07-31
 8. 2025-08-31
 9. 2025-09-30
10. 2025-10-31
11. 2025-11-30
12. 2025-12-31
13. 2026-01-31
14. 2026-02-28
15. 2026-03-31
```

---

### 9. `YY-1M-31L` — 31st; clamp to last day of month if shorter

**Start:** 2025-01-31

Produces identical output to `YY-1M-L` when started on the last day.
The `L` suffix means: if day 31 does not exist this month, use the last day instead.

```text
 1. 2025-01-31
 2. 2025-02-28  ← clamped (Feb has 28 days)
 3. 2025-03-31
 4. 2025-04-30  ← clamped (Apr has 30 days)
 5. 2025-05-31
 6. 2025-06-30  ← clamped
 7. 2025-07-31
 8. 2025-08-31
 9. 2025-09-30  ← clamped
10. 2025-10-31
11. 2025-11-30  ← clamped
12. 2025-12-31
13. 2026-01-31
14. 2026-02-28  ← clamped
15. 2026-03-31
```

---

### 10. `YY-1M-31N` — 31st; roll to 1st of next month if shorter

**Start:** 2025-01-31

The `N` suffix means: when day 31 overflows, `actual` = last day of month, `observed` = 1st of the following month.

```text
 1. Exact         2025-01-31
 2. AdjustedLater actual=2025-02-28  observed=2025-03-01
 3. Exact         2025-03-31
 4. AdjustedLater actual=2025-04-30  observed=2025-05-01
 5. Exact         2025-05-31
 6. AdjustedLater actual=2025-06-30  observed=2025-07-01
 7. Exact         2025-07-31
 8. Exact         2025-08-31
 9. AdjustedLater actual=2025-09-30  observed=2025-10-01
10. Exact         2025-10-31
11. AdjustedLater actual=2025-11-30  observed=2025-12-01
12. Exact         2025-12-31
13. Exact         2026-01-31
14. AdjustedLater actual=2026-02-28  observed=2026-03-01
15. Exact         2026-03-31
```

---

### 11. `YY-1M-31O` — 31st; overflow by remainder days into next month

**Start:** 2025-01-31

The `O` suffix means: when day 31 overflows by *k* days, `observed` = 1st of next month + (*k* − 1) days.
- February (28 days): overflow = 3 → observed = Mar 1 + 2 = **Mar 3**
- 30-day months: overflow = 1 → observed = 1st of next month + 0 = **1st of next month**

```text
 1. Exact         2025-01-31
 2. AdjustedLater actual=2025-02-28  observed=2025-03-03
 3. Exact         2025-03-31
 4. AdjustedLater actual=2025-04-30  observed=2025-05-01
 5. Exact         2025-05-31
 6. AdjustedLater actual=2025-06-30  observed=2025-07-01
 7. Exact         2025-07-31
 8. Exact         2025-08-31
 9. AdjustedLater actual=2025-09-30  observed=2025-10-01
10. Exact         2025-10-31
11. AdjustedLater actual=2025-11-30  observed=2025-12-01
12. Exact         2025-12-31
13. Exact         2026-01-31
14. AdjustedLater actual=2026-02-28  observed=2026-03-03
15. Exact         2026-03-31
```

---

### 12. `YY-MM-MON` — Every Monday

**Start:** 2025-01-06 (Monday)

```text
 1. 2025-01-06
 2. 2025-01-13
 3. 2025-01-20
 4. 2025-01-27
 5. 2025-02-03
 6. 2025-02-10
 7. 2025-02-17
 8. 2025-02-24
 9. 2025-03-03
10. 2025-03-10
11. 2025-03-17
12. 2025-03-24
13. 2025-03-31
14. 2025-04-07
15. 2025-04-14
```

---

### 13. `YY-1M-MON#1` — First Monday of every month

**Start:** 2025-01-06

```text
 1. 2025-01-06
 2. 2025-02-03
 3. 2025-03-03
 4. 2025-04-07
 5. 2025-05-05
 6. 2025-06-02
 7. 2025-07-07
 8. 2025-08-04
 9. 2025-09-01
10. 2025-10-06
11. 2025-11-03
12. 2025-12-01
13. 2026-01-05
14. 2026-02-02
15. 2026-03-02
```

---

### 14. `YY-1M-FRI#L` — Last Friday of every month

**Start:** 2025-01-31 (Friday)

```text
 1. 2025-01-31
 2. 2025-02-28
 3. 2025-03-28
 4. 2025-04-25
 5. 2025-05-30
 6. 2025-06-27
 7. 2025-07-25
 8. 2025-08-29
 9. 2025-09-26
10. 2025-10-31
11. 2025-11-28
12. 2025-12-26
13. 2026-01-30
14. 2026-02-27
15. 2026-03-27
```

---

### 15. `YY-1M-WED#2L` — 2nd-to-last Wednesday of every month

**Start:** 2025-01-22

```text
 1. 2025-01-22  (last Wed=Jan 29, 2nd-to-last=Jan 22)
 2. 2025-02-19  (last Wed=Feb 26, 2nd-to-last=Feb 19)
 3. 2025-03-19  (last Wed=Mar 26, 2nd-to-last=Mar 19)
 4. 2025-04-23  (last Wed=Apr 30, 2nd-to-last=Apr 23)
 5. 2025-05-21  (last Wed=May 28, 2nd-to-last=May 21)
 6. 2025-06-18  (last Wed=Jun 25, 2nd-to-last=Jun 18)
 7. 2025-07-23  (last Wed=Jul 30, 2nd-to-last=Jul 23)
 8. 2025-08-20  (last Wed=Aug 27, 2nd-to-last=Aug 20)
 9. 2025-09-17  (last Wed=Sep 24, 2nd-to-last=Sep 17)
10. 2025-10-22  (last Wed=Oct 29, 2nd-to-last=Oct 22)
11. 2025-11-19  (last Wed=Nov 26, 2nd-to-last=Nov 19)
12. 2025-12-24  (last Wed=Dec 31, 2nd-to-last=Dec 24)
13. 2026-01-21  (last Wed=Jan 28, 2nd-to-last=Jan 21)
14. 2026-02-18  (last Wed=Feb 25, 2nd-to-last=Feb 18)
15. 2026-03-18  (last Wed=Mar 25, 2nd-to-last=Mar 18)
```

---

### 16. `YY-MM-[01,15]` — 1st and 15th of every month

**Start:** 2025-01-01

```text
 1. 2025-01-01
 2. 2025-01-15
 3. 2025-02-01
 4. 2025-02-15
 5. 2025-03-01
 6. 2025-03-15
 7. 2025-04-01
 8. 2025-04-15
 9. 2025-05-01
10. 2025-05-15
11. 2025-06-01
12. 2025-06-15
13. 2025-07-01
14. 2025-07-15
15. 2025-08-01
```

---

### 17. `YY-MM-[MON,WED,FRI]` — Monday, Wednesday, and Friday every week

**Start:** 2025-01-06 (Monday)

```text
 1. 2025-01-06  Mon
 2. 2025-01-08  Wed
 3. 2025-01-10  Fri
 4. 2025-01-13  Mon
 5. 2025-01-15  Wed
 6. 2025-01-17  Fri
 7. 2025-01-20  Mon
 8. 2025-01-22  Wed
 9. 2025-01-24  Fri
10. 2025-01-27  Mon
11. 2025-01-29  Wed
12. 2025-01-31  Fri
13. 2025-02-03  Mon
14. 2025-02-05  Wed
15. 2025-02-07  Fri
```

---

### 18. `YY-3M-15` — 15th every quarter

**Start:** 2025-01-15

```text
 1. 2025-01-15
 2. 2025-04-15
 3. 2025-07-15
 4. 2025-10-15
 5. 2026-01-15
 6. 2026-04-15
 7. 2026-07-15
 8. 2026-10-15
 9. 2027-01-15
10. 2027-04-15
11. 2027-07-15
12. 2027-10-15
13. 2028-01-15
14. 2028-04-15
15. 2028-07-15
```

---

### 19. `YY-3M-L` — Last day of each quarter

**Start:** 2025-01-31

```text
 1. 2025-01-31
 2. 2025-04-30
 3. 2025-07-31
 4. 2025-10-31
 5. 2026-01-31
 6. 2026-04-30
 7. 2026-07-31
 8. 2026-10-31
 9. 2027-01-31
10. 2027-04-30
11. 2027-07-31
12. 2027-10-31
13. 2028-01-31
14. 2028-04-30
15. 2028-07-31
```

---

### 20. `YY-6M-01` — 1st of every half-year

**Start:** 2025-01-01

```text
 1. 2025-01-01
 2. 2025-07-01
 3. 2026-01-01
 4. 2026-07-01
 5. 2027-01-01
 6. 2027-07-01
 7. 2028-01-01
 8. 2028-07-01
 9. 2029-01-01
10. 2029-07-01
11. 2030-01-01
12. 2030-07-01
13. 2031-01-01
14. 2031-07-01
15. 2032-01-01
```

---

### 21. `1Y-06-15` — June 15th every year

**Start:** 2025-06-15

```text
 1. 2025-06-15
 2. 2026-06-15
 3. 2027-06-15
 4. 2028-06-15
 5. 2029-06-15
 6. 2030-06-15
 7. 2031-06-15
 8. 2032-06-15
 9. 2033-06-15
10. 2034-06-15
11. 2035-06-15
12. 2036-06-15
13. 2037-06-15
14. 2038-06-15
15. 2039-06-15
```

---

### 22. `2Y-06-15` — June 15th every 2 years

**Start:** 2025-06-15

```text
 1. 2025-06-15
 2. 2027-06-15
 3. 2029-06-15
 4. 2031-06-15
 5. 2033-06-15
 6. 2035-06-15
 7. 2037-06-15
 8. 2039-06-15
 9. 2041-06-15
10. 2043-06-15
11. 2045-06-15
12. 2047-06-15
13. 2049-06-15
14. 2051-06-15
15. 2053-06-15
```

---

### 23. `1Y-3M-15` — Combined year+month clock: year advances by 1 AND month advances by 3 each tick

**Start:** 2025-06-15

The two clocks compose: each tick adds 1 year AND 3 months to the calendar position, so the month drifts forward each year.

```text
 1. 2025-06-15
 2. 2026-09-15
 3. 2027-12-15
 4. 2029-03-15  ← Dec+3 wraps to Mar, carrying an extra year
 5. 2030-06-15
 6. 2031-09-15
 7. 2032-12-15
 8. 2034-03-15
 9. 2035-06-15
10. 2036-09-15
11. 2037-12-15
12. 2039-03-15
13. 2040-06-15
14. 2041-09-15
15. 2042-12-15
```

---

### 24. `2Y-[03,09]-01` — 1st of March and September every 2 years

**Start:** 2025-03-01

Year cycle = every 2 years from start; within each valid year, both months are visited.

```text
 1. 2025-03-01
 2. 2025-09-01
 3. 2027-03-01
 4. 2027-09-01
 5. 2029-03-01
 6. 2029-09-01
 7. 2031-03-01
 8. 2031-09-01
 9. 2033-03-01
10. 2033-09-01
11. 2035-03-01
12. 2035-09-01
13. 2037-03-01
14. 2037-09-01
15. 2039-03-01
```

---

### 25. `2025-MM-01` — 1st of every month, year 2025 only

**Start:** 2025-01-01

The iterator exhausts after December 2025 (12 results total).

```text
 1. 2025-01-01
 2. 2025-02-01
 3. 2025-03-01
 4. 2025-04-01
 5. 2025-05-01
 6. 2025-06-01
 7. 2025-07-01
 8. 2025-08-01
 9. 2025-09-01
10. 2025-10-01
11. 2025-11-01
12. 2025-12-01
   (iterator ends — year 2025 exhausted)
```

---

### 26. `[2025,2026]-MM-[01,15]` — 1st and 15th of every month across 2025 and 2026

**Start:** 2025-01-01

Iterator exhausts after 2026-12-15 (48 results total). First 15 shown:

```text
 1. 2025-01-01
 2. 2025-01-15
 3. 2025-02-01
 4. 2025-02-15
 5. 2025-03-01
 6. 2025-03-15
 7. 2025-04-01
 8. 2025-04-15
 9. 2025-05-01
10. 2025-05-15
11. 2025-06-01
12. 2025-06-15
13. 2025-07-01
14. 2025-07-15
15. 2025-08-01
```

---

### 27. `YY-01-4D` — Every 4 days, January only; sequence restarts each year

**Start:** 2025-01-01

After Jan 29 the remaining step (Jan 33) is outside January, so the sequence resets to Jan 1 of the next year.

```text
 1. 2025-01-01
 2. 2025-01-05
 3. 2025-01-09
 4. 2025-01-13
 5. 2025-01-17
 6. 2025-01-21
 7. 2025-01-25
 8. 2025-01-29
 9. 2026-01-01  ← sequence restarts from Jan 1
10. 2026-01-05
11. 2026-01-09
12. 2026-01-13
13. 2026-01-17
14. 2026-01-21
15. 2026-01-25
```

---

### 28. `1Y-[01,06]-7D` — Every 7 days in January and June, every year; sequence restarts each month

**Start:** 2025-01-01

Within each constrained month the 7-day sequence starts fresh from the 1st. After the month is exhausted the iterator moves to the next valid month (or Jan of the next valid year).

```text
 1. 2025-01-01
 2. 2025-01-08
 3. 2025-01-15
 4. 2025-01-22
 5. 2025-01-29
 6. 2025-06-01  ← restarts from Jun 1
 7. 2025-06-08
 8. 2025-06-15
 9. 2025-06-22
10. 2025-06-29
11. 2026-01-01  ← restarts from Jan 1 of next year
12. 2026-01-08
13. 2026-01-15
14. 2026-01-22
15. 2026-01-29
```

---

### 29. `YY-1M-15~NW` — 15th monthly; if weekend, advance to next weekday

**Start:** 2025-01-15

`~NW` is conditional: only adjusts when the target date falls on Saturday or Sunday.

```text
 1. Exact         2025-01-15  Wed
 2. AdjustedLater actual=2025-02-15 Sat  observed=2025-02-17 Mon
 3. AdjustedLater actual=2025-03-15 Sat  observed=2025-03-17 Mon
 4. Exact         2025-04-15  Tue
 5. Exact         2025-05-15  Thu
 6. AdjustedLater actual=2025-06-15 Sun  observed=2025-06-16 Mon
 7. Exact         2025-07-15  Tue
 8. Exact         2025-08-15  Fri
 9. Exact         2025-09-15  Mon
10. Exact         2025-10-15  Wed
11. AdjustedLater actual=2025-11-15 Sat  observed=2025-11-17 Mon
12. Exact         2025-12-15  Mon
13. Exact         2026-01-15  Thu
14. AdjustedLater actual=2026-02-15 Sun  observed=2026-02-16 Mon
15. AdjustedLater actual=2026-03-15 Sun  observed=2026-03-16 Mon
```

---

### 30. `YY-1M-15~PW` — 15th monthly; if weekend, move back to previous weekday

**Start:** 2025-01-15

```text
 1. Exact           2025-01-15  Wed
 2. AdjustedEarlier actual=2025-02-15 Sat  observed=2025-02-14 Fri
 3. AdjustedEarlier actual=2025-03-15 Sat  observed=2025-03-14 Fri
 4. Exact           2025-04-15  Tue
 5. Exact           2025-05-15  Thu
 6. AdjustedEarlier actual=2025-06-15 Sun  observed=2025-06-13 Fri
 7. Exact           2025-07-15  Tue
 8. Exact           2025-08-15  Fri
 9. Exact           2025-09-15  Mon
10. Exact           2025-10-15  Wed
11. AdjustedEarlier actual=2025-11-15 Sat  observed=2025-11-14 Fri
12. Exact           2025-12-15  Mon
13. Exact           2026-01-15  Thu
14. AdjustedEarlier actual=2026-02-15 Sun  observed=2026-02-13 Fri
15. AdjustedEarlier actual=2026-03-15 Sun  observed=2026-03-13 Fri
```

---

### 31. `YY-1M-15~3P` — 15th monthly; unconditionally subtract 3 business days

**`new_after`:** 2025-01-14 (with `new_with_start` the start itself would be returned as-is, unadjusted)

`~3P` is **unconditional** — every result is shifted back 3 business days (WeekendSkipper), regardless of what day the 15th falls on.

```text
 1. AdjustedEarlier actual=2025-01-15 Wed  observed=2025-01-10 Fri
 2. AdjustedEarlier actual=2025-02-15 Sat  observed=2025-02-12 Wed
 3. AdjustedEarlier actual=2025-03-15 Sat  observed=2025-03-12 Wed
 4. AdjustedEarlier actual=2025-04-15 Tue  observed=2025-04-10 Thu
 5. AdjustedEarlier actual=2025-05-15 Thu  observed=2025-05-12 Mon
 6. AdjustedEarlier actual=2025-06-15 Sun  observed=2025-06-11 Wed
 7. AdjustedEarlier actual=2025-07-15 Tue  observed=2025-07-10 Thu
 8. AdjustedEarlier actual=2025-08-15 Fri  observed=2025-08-12 Tue
 9. AdjustedEarlier actual=2025-09-15 Mon  observed=2025-09-10 Wed
10. AdjustedEarlier actual=2025-10-15 Wed  observed=2025-10-10 Fri
11. AdjustedEarlier actual=2025-11-15 Sat  observed=2025-11-12 Wed
12. AdjustedEarlier actual=2025-12-15 Mon  observed=2025-12-10 Wed
13. AdjustedEarlier actual=2026-01-15 Thu  observed=2026-01-12 Mon
14. AdjustedEarlier actual=2026-02-15 Sun  observed=2026-02-11 Wed
15. AdjustedEarlier actual=2026-03-15 Sun  observed=2026-03-11 Wed
```

---

### 32. `YY-1M-L~2N` — Last day of month; unconditionally add 2 business days

**`new_after`:** 2025-01-30 (with `new_with_start` the start itself would be returned as-is, unadjusted)

`~2N` is **unconditional** — every result is pushed forward 2 business days.

```text
 1. AdjustedLater actual=2025-01-31 Fri  observed=2025-02-04 Tue
 2. AdjustedLater actual=2025-02-28 Fri  observed=2025-03-04 Tue
 3. AdjustedLater actual=2025-03-31 Mon  observed=2025-04-02 Wed
 4. AdjustedLater actual=2025-04-30 Wed  observed=2025-05-02 Fri
 5. AdjustedLater actual=2025-05-31 Sat  observed=2025-06-03 Tue
 6. AdjustedLater actual=2025-06-30 Mon  observed=2025-07-02 Wed
 7. AdjustedLater actual=2025-07-31 Thu  observed=2025-08-04 Mon
 8. AdjustedLater actual=2025-08-31 Sun  observed=2025-09-02 Tue
 9. AdjustedLater actual=2025-09-30 Tue  observed=2025-10-02 Thu
10. AdjustedLater actual=2025-10-31 Fri  observed=2025-11-04 Tue
11. AdjustedLater actual=2025-11-30 Sun  observed=2025-12-02 Tue
12. AdjustedLater actual=2025-12-31 Wed  observed=2026-01-02 Fri
13. AdjustedLater actual=2026-01-31 Sat  observed=2026-02-03 Tue
14. AdjustedLater actual=2026-02-28 Sat  observed=2026-03-03 Tue
15. AdjustedLater actual=2026-03-31 Tue  observed=2026-04-02 Thu
```

---

### 33. `YY-MM-BD#3` — 3rd business day of every month

Counts the business days of each month with the configured processor and picks the 3rd. Unlike
`YY-MM-01~2N`, this is right whether or not the 1st is a business day. (`WeekendSkipper`
shown; with a holiday calendar, holidays are not counted.)

**Start:** 2025-01-03 (Friday)

```text
 1. 2025-01-03 Fri
 2. 2025-02-05 Wed   (Feb 1 is a Saturday: Mon 3, Tue 4, Wed 5)
 3. 2025-03-05 Wed
 4. 2025-04-03 Thu
 5. 2025-05-05 Mon
 6. 2025-06-04 Wed
 7. 2025-07-03 Thu
 8. 2025-08-05 Tue
 9. 2025-09-03 Wed
10. 2025-10-03 Fri
11. 2025-11-05 Wed
12. 2025-12-03 Wed
```

All results are `Exact`: the rule only ever picks business days.

---

### 34. `YY-MM-BD#L` — Last business day of every month

Same result as `YY-MM-L~PB`, but reported as `Exact` on the business day itself rather than
`AdjustedEarlier(last_day, business_day)`. Use `~PB` when the calendar month end matters too
(for example, a statement dated the 31st).

**Start:** 2025-01-31 (Friday)

```text
 1. 2025-01-31 Fri
 2. 2025-02-28 Fri
 3. 2025-03-31 Mon
 4. 2025-04-30 Wed
 5. 2025-05-30 Fri
 6. 2025-06-30 Mon
 7. 2025-07-31 Thu
 8. 2025-08-29 Fri
 9. 2025-09-30 Tue
10. 2025-10-31 Fri
11. 2025-11-28 Fri
12. 2025-12-31 Wed
```

`BD#2L` is the 2nd-to-last business day; `WD#…` counts weekdays and ignores holidays.

---

### 35. `YY-1M-L~MF` — Last day of every month, Modified Following

The usual convention for month-end payment runs: roll forward to the next business day, but
never into the next month.

**`new_after`:** 2025-05-01

```text
 1. AdjustedEarlier actual=2025-05-31 Sat  observed=2025-05-30 Fri   (Jun 2 is next month: roll back)
 2. Exact           2025-06-30 Mon
 3. Exact           2025-07-31 Thu
 4. AdjustedEarlier actual=2025-08-31 Sun  observed=2025-08-29 Fri
 5. Exact           2025-09-30 Tue
 6. Exact           2025-10-31 Fri
 7. AdjustedEarlier actual=2025-11-30 Sun  observed=2025-11-28 Fri
 8. Exact           2025-12-31 Wed
```

Compare `~NB`, which would give 2025-06-02 for May. A mid-month date such as `YY-MM-15~MF`
behaves exactly like `~NB`, because rolling forward from the 15th never leaves the month.
`~MP` is the mirror image: `YY-MM-01~MP` on Saturday 2025-03-01 gives Monday 2025-03-03, since
the previous business day (Feb 28) is in the previous month.

---

### 36. `YY-MM-[TUE,THU]/2W @2025-01-07` — Tuesday and Thursday, every other week

A week cycle keeps only the dates in every 2nd week, counted from the week containing the start
(Monday to Sunday). The count carries across months.

```text
 1. 2025-01-07 Tue
 2. 2025-01-09 Thu
 3. 2025-01-21 Tue
 4. 2025-01-23 Thu
 5. 2025-02-04 Tue
 6. 2025-02-06 Thu
 7. 2025-02-18 Tue
 8. 2025-02-20 Thu
```

---
### 37. `YY-MM-[MON-FRI]/4W[1,2] @2025-01-06` — Weekdays, two weeks on and two weeks off

Weeks 1 and 2 of every 4-week cycle: 6–17 January, then 3–14 February, and so on.

```text
 1. 2025-01-06 Mon
 2. 2025-01-07 Tue
 3. 2025-01-08 Wed
 4. 2025-01-09 Thu
 5. 2025-01-10 Fri
 6. 2025-01-13 Mon
 7. 2025-01-14 Tue
 8. 2025-01-15 Wed
 9. 2025-01-16 Thu
10. 2025-01-17 Fri
11. 2025-02-03 Mon
12. 2025-02-04 Tue
```

---
### 38. `YY-MM-8D[1-4] @2025-01-01` — Four days on, four days off

Days 1–4 of every 8-day cycle from the start, regardless of weekday or month.

```text
 1. 2025-01-01 Wed
 2. 2025-01-02 Thu
 3. 2025-01-03 Fri
 4. 2025-01-04 Sat
 5. 2025-01-09 Thu
 6. 2025-01-10 Fri
 7. 2025-01-11 Sat
 8. 2025-01-12 Sun
 9. 2025-01-17 Fri
10. 2025-01-18 Sat
11. 2025-01-19 Sun
12. 2025-01-20 Mon
```

---
### 39. `YY-3M[1-2]-15 @2025-01-15` — The 15th in months 1 and 2 of every 3

Positions on a month cycle: January and February, then April and May, and so on.

```text
 1. 2025-01-15 Wed
 2. 2025-02-15 Sat
 3. 2025-04-15 Tue
 4. 2025-05-15 Thu
 5. 2025-07-15 Tue
 6. 2025-08-15 Fri
```

---
### 40. `YY-MM-BD#3 | YY-MM-L~PB @2025-01-01` — Union: 3rd and last business day of each month

Each part keeps its own rule; the results are merged in date order.

```text
 1. 2025-01-03 Fri
 2. 2025-01-31 Fri
 3. 2025-02-05 Wed
 4. 2025-02-28 Fri
 5. 2025-03-05 Wed
 6. 2025-03-31 Mon
 7. 2025-04-03 Thu
 8. 2025-04-30 Wed
```

---
### 41. `YY-MM-DD ! YY-12-25 | YY-12-26 | YY-01-01 @2025-12-22` — Every day except the holidays

The exclusion is itself a union of date specs, matched against each actual date.

```text
 1. 2025-12-22 Mon
 2. 2025-12-23 Tue
 3. 2025-12-24 Wed
 4. 2025-12-27 Sat
 5. 2025-12-28 Sun
 6. 2025-12-29 Mon
 7. 2025-12-30 Tue
 8. 2025-12-31 Wed
```

---
### 42. `YY-MM-01~MF @2025-04-01 x36` — A self-contained loan schedule: 36 payments

Start, adjustment and count are all in the text. Payments that fall on a weekend move to the next
business day within the month; the 36th is 1 March 2028.

```text
 1. 2025-04-01 Tue
 2. 2025-05-01 Thu
 3. AdjustedLater   actual=2025-06-01 Sun  observed=2025-06-02 Mon
 4. 2025-07-01 Tue
 5. 2025-08-01 Fri
 6. 2025-09-01 Mon
 7. 2025-10-01 Wed
 8. AdjustedLater   actual=2025-11-01 Sat  observed=2025-11-03 Mon
...
34. AdjustedLater   actual=2028-01-01 Sat  observed=2028-01-03 Mon
35. 2028-02-01 Tue
36. 2028-03-01 Wed
```

---

### 43. `YY-MM-BD#[1-3,L] @2025-01-01` — First three and last business day of each month

Several positions in one rule: numbers and ranges count from the start of the month, `L` and `nL`
from the end. A date named twice (in a month with very few business days) is yielded once.

```text
 1. 2025-01-01 Wed
 2. 2025-01-02 Thu
 3. 2025-01-03 Fri
 4. 2025-01-31 Fri
 5. 2025-02-03 Mon
 6. 2025-02-04 Tue
 7. 2025-02-05 Wed
 8. 2025-02-28 Fri
 9. 2025-03-03 Mon
10. 2025-03-04 Tue
11. 2025-03-05 Wed
12. 2025-03-31 Mon
```

---

### 44. `YY-MM-15~SB @2025-01-01` — The 15th, only when it is a business day

`~SB` drops a date that is not a business day instead of moving it. February, March, June and
November 2025 have the 15th on a weekend, so they have no occurrence. Dropped dates are not
counted by `xN`.

```text
 1. 2025-01-15 Wed
 2. 2025-04-15 Tue
 3. 2025-05-15 Thu
 4. 2025-07-15 Tue
 5. 2025-08-15 Fri
 6. 2025-09-15 Mon
 7. 2025-10-15 Wed
 8. 2025-12-15 Mon
```

---

### 45. `YY-1M-31L~MF @2025-01-31 ..+30Y` — Repayments collected at month end for 30 years

The term is in the text: the schedule ends just before 31 January 2055, so it has exactly 360
payments. A month end on a weekend moves back to the Friday, since Modified Following never
moves into the next month.

```text
  1. 2025-01-31 Fri
  2. 2025-02-28 Fri
  3. 2025-03-31 Mon
  4. 2025-04-30 Wed
  5. AdjustedEarlier  actual=2025-05-31 Sat  observed=2025-05-30 Fri
  6. 2025-06-30 Mon
...
358. AdjustedEarlier  actual=2054-10-31 Sat  observed=2054-10-30 Fri
359. 2054-11-30 Mon
360. 2054-12-31 Thu
```

---

### 46. `YY-MM-EAS-2 | YY-MM-EAS+1 | YY-MM-OEAS+1 @2025-01-01 x8` — Easter holidays

Good Friday and Easter Monday, plus Orthodox Easter Monday. In 2025 both Easters fall on
20 April, so the two Mondays are one date and appear once.

```text
 1. 2025-04-18 Fri
 2. 2025-04-21 Mon
 3. 2026-04-03 Fri
 4. 2026-04-06 Mon
 5. 2026-04-13 Mon
 6. 2027-03-26 Fri
 7. 2027-03-29 Mon
 8. 2027-05-03 Mon
```

---

## Behaviour Notes

### How a spec is evaluated

The year and month rules pick the **eligible months**; the day rule picks dates inside them.

- **Calendar day rules** (`DD`, `15`, `[01,15]`, `L`, `31L/N/O`, `MON`, `[MON,FRI]`, `WED#2`, `FRI#L`, `EAS-2`) list the matching days of each eligible month; the next occurrence is the first one after the previous.
- **Step day rules** (`nD`, `nBD`, `nWD`) advance from the previous occurrence — see the two notes below.

`nY` / `nM` cadences are aligned to the iterator's start (or the `new_after` cursor).

### Step rules inside eligible months (`YY-01-4D`, `1Y-[01,06]-7D`, `2Y-MM-7D`)

Without an `nM` cadence, the step **keeps rolling while it lands in an eligible month**, and **restarts at the 1st** of the next eligible month once it leaves them. For `nBD` / `nWD` the restart is the first business day / weekday on or after the 1st.

- `YY-MM-7D` — every month is eligible, so the step rolls forever.
- `YY-01-4D` — Jan 29 + 4 = Feb 2 is not eligible → restart on Jan 1 of the next year.
- `YY-[01,02]-7D` — Jan 29 + 7 = Feb 5 is eligible → keeps rolling through February.
- `2Y-MM-7D` — rolls through every month of an eligible year, restarts on Jan 1 of the next one.

### Year cadence with every month (`2Y-MM-15`, `1Y-MM-L`)

`nY` with `MM` visits **every month** of each eligible year: `2Y-MM-15` from 2024-10-15 gives Oct 15, Nov 15, Dec 15 2024, then Jan 15 2026, …

### Combined year+month clocks (`1Y-3M-15`)

When both year and month use `NextNth`, each tick advances **both** counters simultaneously: `nY-mM` is a single cadence of `12·n + m` months. Month overflow carries into the year counter, which is why `1Y-3M-15` produces Dec→Mar with a 2-year gap rather than 1-year.

The same clock applies to step rules under an `nM` cadence: `YY-1M-7D` adds 1 month **and** 7 days per tick (Dec 1 → Jan 8 → Feb 15 → …).

### `DDL` vs `DDN` vs `DDO`

All three handle the case where the target day does not exist in a given month:

| Suffix | Behaviour on overflow |
|--------|-----------------------|
| `L` | Clamp to last day; `Exact(last_day)` |
| `N` | `AdjustedLater(last_day, first_of_next_month)` |
| `O` | `AdjustedLater(last_day, first_of_next_month + (overflow_days − 1))` |

### Iterator termination

Specs with finite year sets (`2025-MM-01`, `[2025,2026]-MM-15`) terminate once all valid dates are exhausted. Callers should handle `None` from the iterator.

Out-of-range values (month `13`, day `32`, `FRI#6`, `BD#24`, step `0M` / `0D`) are rejected when parsing with `Error::InvalidDateSpec`. Specs that parse but can never match (`YY-02-30`) return `Error::IteratorNotConverged` after a bounded search instead of looping forever.

### Daylight saving time

Dates keep the time-of-day of the start (or `new_after` cursor) and are mapped to real instants with the builder's `DstPolicy` (`Adjust` by default).

When that local time does not exist, it moves later by the length of the gap and the result is `AdjustedLater(label, shifted)`. `label` shows the date as scheduled, with the offset in force before the gap; `shifted` shows it in the real offset. Both are the same instant, so `actual == observed` when compared. Results always strictly increase: a later date that lands on the same instant is dropped.

Pacific/Apia skipped 2011-12-30 entirely (−10:00 → +14:00). `YY-MM-DD` from 2011-12-29:

```text
Exact(2011-12-29 00:00 -10:00)
AdjustedLater(2011-12-30 00:00 -10:00, 2011-12-31 00:00 +14:00)   ← Dec 31's own occurrence is dropped
Exact(2012-01-01 00:00 +14:00)
```

With `DstPolicy::Strict` the skipped date returns `Error::AmbiguousLocalTime` instead.

### Resuming a stored schedule

> *Library behaviour: set when building the iterator, not written in the spec.*

To continue from an occurrence stored in a database, use
`SpecIteratorBuilder::resume_after(spec, bdp, tz, last_actual)`, where `last_actual` is the
stored ``actual()`` parsed back from RFC 3339. Store it with its UTC offset and rebuild with the
same options; add `.with_end(end)` if the schedule has a stored end. The crate docs' section *Persisting and resuming a schedule* has the full
pattern and the rules that keep resumed results identical to a continuous iterator.

### Start-date inclusivity

> *Library behaviour: set when building the iterator, not written in the spec.*

When constructed with `new_with_start`, the start date is always returned as the **first result**, as `Exact` and unchanged — even when it does not match the spec, and without applying any `~` adjustment (a Saturday start stays Saturday). This lets you feed an already-adjusted (observed) date back in as the start. Use `new_after` to begin strictly after a given point in time.

### End bound

> *Library behaviour: set when building the iterator, not written in the spec.*

`with_end(end)` is **inclusive**: occurrences up to and including `end` are returned, and the iterator stops at the first occurrence whose actual or observed date is after `end`. Nothing is invented at the boundary. `with_end_spec(spec)` uses the first occurrence of `spec` after the start (its observed date) as `end`.

### Conditional adjustments keep every raw date

Each raw date produces one result, even when the adjustment makes observed dates repeat or go back. `YY-MM-DD~NB` from a Thursday gives:

```text
Exact(Fri)
AdjustedLater(Sat, Mon)
AdjustedLater(Sun, Mon)
Exact(Mon)
```

and `YY-MM-DD~PB` gives `Exact(Fri)`, `AdjustedEarlier(Sat, Fri)`, `AdjustedEarlier(Sun, Fri)`, `Exact(Mon)`. `actual` always increases; `observed` may repeat or go back. For one result per business day, use a day rule that only hits business days (`1BD`).

---

## Edge cases and invalid specs

The following combinations are either silently broken, produce no results, or loop indefinitely. **Avoid them.**

---

### `YY-02-30`, `YY-02-31` — Fixed day that never exists in February (without overflow suffix)

**Examples:** `YY-02-30`, `YY-02-31`

**Problem:** Without an overflow suffix (`L`, `N`, or `O`), day 30 or 31 in February (`02`) simply does not exist. `YY-02-30` can never match, so the iterator returns `Error::IteratorNotConverged` after a bounded search.

```text
YY-02-30  →  no February result (day 30 never exists)
YY-MM-31  →  skips all months with fewer than 31 days (Apr, Jun, Sep, Nov, Feb)
```

**Safe alternatives:**
- `YY-02-L` — last day of February (28 or 29 on leap years)
- `YY-02-28L` — 28th, clamped to last day on overflow (same effect as `L` here)
- `YY-02-28N` — 28th, with `AdjustedLater → 1 Mar` on non-leap years
- `YY-MM-31L` — 31st every month, clamped to last day when the month is shorter

---

### `DAY#0` — Zero-th occurrence of a weekday

**Example:** `YY-MM-MON#0`

**Problem:** There is no "zeroth" weekday occurrence in any month, and no month has a sixth. `MON#0` and `MON#6` (and `MON#0L`, `MON#6L`) are rejected when parsing with `Error::InvalidDateSpec`.

**Safe alternative:** Use `MON#1` (first occurrence) or `MON#L` (last occurrence).

The same applies to `BD#0` / `WD#0`, which are rejected. `BD#n` / `WD#n` accept 1–23; a month with fewer business days (or weekdays) than *n* is skipped.

---

### `YYYY-02-29` — Feb 29 in a non-leap year

**Example:** `2025-02-29`

**Problem:** February 29 does not exist in 2025 (not a leap year). The iterator finds no valid date and the spec produces **zero results**.

**Note:** `2024-02-29` works correctly because 2024 is a leap year.

**Safe alternative:** `2025-02-28L` (clamped to the last day of Feb 2025, which is the 28th).

---

### `[YYYY,...]-02-29` — Feb 29 enumeration including non-leap years

**Example:** `[2024,2025,2026]-02-29`

**Problem:** Only the leap years in the list (e.g. 2024) will produce a result. Non-leap years are silently skipped. The iterator terminates after exhausting the year list; if no year in the list is a leap year, zero results are produced.

**Safe alternative:** Either restrict the list to known leap years, or use `YY-02-29` which naturally skips non-leap years (zero results per year, not an error) if you only want Feb-29 dates — or use `YY-02-L` for end-of-February every year.

---

### Summary table

| Combination | Problem | Safe alternative |
|-------------|---------|-----------------|
| `YY-02-30` / `YY-02-31` | Day never exists in February; `IteratorNotConverged` | `YY-02-L` or `YY-02-28L` |
| `YY-MM-31` (no suffix) | Silently skips short months | `YY-MM-31L` (clamp) |
| `MON#0`, `MON#6` | Occurrence never exists; rejected at parse time | `MON#1` or `MON#L` |
| `BD#23` | Most months have fewer business days; they are skipped | `BD#L` for the last business day |
| `YYYY-02-29` (non-leap year) | Zero results | `YYYY-02-28L` |
| `[YYYY,...]-02-29` (mix) | Non-leap years silently skipped | Filter list to leap years only |
