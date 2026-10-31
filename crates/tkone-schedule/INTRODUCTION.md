# An introduction to the tkone schedule format

Most scheduling formats were built to answer the question *"when should this job run on this
server?"*. Business schedules ask harder questions:

- *"Pay on the 31st of every month, or on the last day when the month is shorter, then move to
  the next business day if that is a holiday."*
- *"Run the report two business days before month end."*
- *"Every three months, counted from the day the contract started."*
- *"Every 90 minutes, and do the right thing when the clocks change."*

The tkone schedule format exists to state rules like these in one short line of text, and to
compute them the same way every time.

```text
YY-1M-31L~NB                  the 31st of every month (or the last day), then the next business day
YY-MM-L~2P                    two business days before the last day of every month
YY-3M-15                      the 15th of every 3rd month, counted from the start
YY-MM-DDT1H:30M:00            every day, every 90 minutes
YY-MM-01~MF @2025-04-01 x36   36 monthly payments from April 2025: a whole schedule in one line
```

---

## The format in five minutes

A schedule is a **date part**, a **time part**, or both joined by `T`:

```text
date            <years>-<months>-<days>[~<adjustment>]
time            <hours>:<minutes>:<seconds>
date and time   <date>T<time>
```

### Date part

Read it left to right as "which years, which months, which days".

| Piece | Examples | Meaning |
|-------|----------|---------|
| Years | `YY` · `2025` · `[2025,2026]` · `[2025-2027]` · `2Y` | every year · only 2025 · those years · 2025 to 2027 · every 2nd year |
| Months | `MM` · `03` · `[03,09]` · `[01-06]` · `3M` | every month · March · March and September · January to June · every 3rd month |
| Days | `DD` · `15` · `[01,15]` · `[01-10]` · `L` | every day · the 15th · the 1st and 15th · the 1st to 10th · the last day |
| Short months | `31L` · `31N` · `31O` | the 31st, or: the last day · the 1st of next month · overflow into next month |
| Weekdays | `MON` · `[MON,FRI]` · `[MON-FRI]` · `WED#2` · `FRI#L` · `THU#2L` | every Monday · Mondays and Fridays · Monday to Friday · 2nd Wednesday · last Friday · 2nd-to-last Thursday |
| Business days | `BD#3` · `BD#L` · `BD#2L` · `BD#[1-5]` · `BD#[1,L]` | 3rd business day · last · 2nd-to-last · first five · first and last |
| Steps | `7D` · `5BD` · `3WD` | every 7 days · every 5 business days · every 3 weekdays |

Ranges such as `[01-06]` are written inside brackets, because `-` also separates the parts.

`2Y`, `3M` and steps are counted from the schedule's start (see
[Start, end, time zone and count](#start-end-time-zone-and-count)), not from January. Started in
February, `YY-3M-15` runs in February, May, August and November.

### Adjustment (optional)

What to do when the date lands on a day you can't use. A *business day* is a weekday that is not
a holiday. The holiday calendar is not written in the schedule: it is supplied alongside it, one
per market if needed, so the same schedule text works for London and New York.

| Adjustment | Meaning |
|------------|---------|
| `~NB` · `~PB` · `~B` | if not a business day: move to the next · previous · nearest one |
| `~MF` · `~MP` | Modified Following · Modified Preceding: as `~NB` · `~PB`, but never into another month |
| `~SB` | if not a business day: skip it (no occurrence) instead of moving it |
| `~NW` · `~PW` · `~W` | if on a weekend: move to the next · previous · nearest weekday |
| `~2N` · `~3P` | always move 2 business days later · 3 business days earlier |

The moving adjustments always produce a date. `~SB` instead *drops* a date that isn't a
business day, so it can sit on top of any rule: `YY-MM-15~SB` is the 15th only when it is a
business day, `YY-MM-[MON,WED,FRI]~SB` is those weekdays except holidays, and `YY-MM-DD/2W~SB`
is business days every other week.

### Time part

Each field is a wildcard (`HH`, `MM`, `SS`), a fixed value (`09`, `30`, `00`), a list or range
of values (`[09,12,17]`, `09-16`), a step (`1H`, `30M`, `15S`), or `_` to keep the start's
value.

| Time part | Meaning |
|-----------|---------|
| `09:30:00` | at 09:30 |
| `[09,12,17]:00:00` | at 09:00, 12:00 and 17:00 |
| `HH:MM:00` | every minute |
| `HH:30M:00` | every 30 minutes |
| `1H:30M:00` | every 90 minutes |
| `HH:[00,30]:00` | on the hour and half hour |
| `1H:_:_` | every hour, at the start's minute and second |
| `09:15M:00` | every 15 minutes, only during the 09:00 hour |
| `09-16:15M:00` | every 15 minutes from 09:00 to 16:45 |

Steps are counted from the schedule's start: `HH:15M:00` started at 09:07 runs at 09:22, 09:37,
and so on. To stay on fixed minutes whatever the start, name them: `HH:[00,15,30,45]:00`.

### Date and time together

Join a date part and a time part with `T`. Every date the date part produces gets every time the
time part produces:

```text
YY-MM-WED#2T08:00:00             08:00 on the 2nd Wednesday of every month
YY-MM-[MON-FRI]T09-16:15M:00     every 15 minutes within 09:00–16:59, Monday to Friday
YY-1M-31L~WT11:00:00             11:00 on the 31st (or the last day), moved off weekends
```

---

## Going further

Whole schedules can be combined, trimmed and bounded. These go at the end of a schedule, in
this order:

```text
<schedule> | <schedule> | …   ! <exclusion> | …   @<start>   ..<end>   x<count>
```

### Combining schedules

Join any number of schedules with `|` to get all their occurrences, in order. This covers the
irregular cases that one rule can't:

```text
YY-MM-BD#3 | YY-MM-L~PB                       the 3rd and the last business day of each month
09:[30,45]:00 | 10-15:15M:00 | 16:00:00       every 15 minutes from 09:30 to 16:00
YY-MM-BD#1T09:00:00 | YY-MM-FRI#LT17:00:00    09:00 on the 1st business day, 17:00 on the last Friday
```

### Excluding dates

Everything after `!` is excluded. A date or datetime schedule excludes by date; a time schedule
excludes by time. The exclusion is itself a schedule, so it can be a single date, a recurring
date, or several joined with `|`:

```text
YY-MM-DD ! YY-12-25 | YY-01-01                every day except 25 December and 1 January
YY-MM-[MON-FRI]T09:00:00 ! 2025-12-[24-31]     09:00 on weekdays, but not 24–31 December 2025
1H:00:00 ! 12:MM:SS                           every hour, except during the 12:00 hour
```

Exclusions apply to the schedule's own date (the scheduled date, before any `~` adjustment) and
to everything before the `!`, including all parts of a `|` combination.

### Weeks and rotations

A week cycle, written after the day rule (`/2W`, `/4W[1,2]`), keeps only dates in some weeks.
Weeks run Monday to Sunday, and week 1 is the week the schedule starts in:

```text
YY-MM-[TUE,THU]/2W            Tuesday and Thursday, every other week
YY-MM-FRI/2W                  every other Friday
YY-MM-[MON-FRI]/4W[1,2]       weekdays in weeks 1 and 2 of every 4 (two weeks on, two off)
```

Positions do the same for days, months and years: a cycle length, then which positions in each
cycle are on, counted from the start. This is how rotations and shift patterns are written:

```text
YY-MM-8D[1-4]                 days 1–4 of every 8 (four on, four off)
YY-MM-14D[1,2,5-7,10,11]      the 2-2-3 pattern over two weeks
YY-3M[1-2]-15                 the 15th in the first two months of every three
4Y[1,3]-06-15                 15 June in years 1 and 3 of every four
```

Months and other filters only remove dates from these cycles; they never restart the count.

### Start, end, time zone and count

A schedule can carry its own start, end, time zone and number of occurrences, so the text alone
defines it:

```text
YY-3M-15~NB @2025-02-15 ..2027-02-15                      quarterly from 15 Feb 2025, until 15 Feb 2027
YY-MM-[TUE,THU]/2WT18:00:00 @2025-01-07[Europe/London]     every other Tue and Thu at 18:00 London time
1H:00:00 @2025-01-06T09:00:00[Europe/Paris] ..2025-01-06T17:00   hourly, 09:00 to 17:00 Paris time
YY-MM-01~MF @2025-04-01 x36                                36 monthly payments from April 2025
```

- **`@start`** is where the counting begins (cadences, weeks, positions, steps and the time
  grid) and the earliest time a result can have. The first result is the first match at or after
  it: `YY-MM-15 @2025-01-10` begins on 15 January.
- **`..end`** is the latest time a result can have. A date without a time covers that whole day.
- **The time zone** goes in brackets after a date or time, as in `[Europe/London]` (the
  RFC 9557 form), and applies to the start, the end and every time in the schedule. A UTC
  offset such as `+01:00` only fixes the instant it is written on; a zone also decides how the
  clock changes.
- **`xN`** stops after *N* occurrences, counted from the start. Excluded dates don't use one up,
  so `x12` always means twelve. With an end as well, whichever comes first stops the schedule.

Each is optional. A schedule without a start counts from whenever it is first used, so
`YY-MM-01~MF x36` is a template: "36 monthly payments from whenever the loan starts". One
without an end or a count repeats indefinitely.

---

## How it behaves

### The scheduled date and the observed date are both kept

When a payment due on Saturday the 31st moves to Monday the 2nd, both dates matter: interest
accrues to the 31st, while cash moves on the 2nd. An adjustment never replaces the date the rule
produced; every occurrence has both:

- the **scheduled** date the rule produced (the 31st)
- the **observed** date after adjustment (the 2nd)

An occurrence is either exact, moved later or moved earlier. The schedule always continues from
scheduled dates, so an adjustment never shifts the dates that follow it.

### Month end is a choice, not an accident

`31L` clamps to the last day, `31N` moves to the 1st of the next month, `31O` overflows by the
missing days, and plain `31` skips short months. You pick the rule in the schedule, and nothing
is decided silently.

### Clock changes are handled the same way every time

Times are read on the wall clock of the schedule's time zone (for example `Europe/London`), with
fixed rules for daylight saving:

- **A time that doesn't exist** (02:30 when the clocks jump from 02:00 to 03:00) moves later
  by the size of the jump, and counts as moved.
- **A time that happens twice** (when the clocks fall back) happens once, not twice.
- **Steps follow the wall clock**: every 6 hours from midnight is 00:00, 06:00, 12:00 and 18:00
  on every day, including the days the clocks change.

### What a schedule needs besides its text

Only one thing is never written in a schedule:

- **a holiday calendar**, which decides what a business day is.

A start and a time zone are needed to read a schedule. They can be written in it (`@…`,
`[Zone]`) or supplied by the system that runs it. How a schedule is stored and picked up again
later is up to that system. The [README](README.md) covers what the tkone library offers for
this.

### For finance

The common business-day conventions map directly onto adjustments:

| Market convention | tkone |
|-------------------|-------|
| Following | `~NB` |
| Modified Following | `~MF` |
| Preceding | `~PB` |
| Modified Preceding | `~MP` |
| Nearest business day | `~B` |
| N business days before / after | `~NP` / `~NN` |

Instalment and coupon schedules are usually defined by a start and a count or a maturity date,
which the schedule text can hold (`@2025-04-01 x36`, `..2028-03-31`).

### For rostering and operations

Rotations are written as cycles from a start: weeks on and off (`/4W[1,2]`), days on and off
(`8D[1-4]`), or a mix of patterns joined with `|`. `~SB` keeps only business days, so
`YY-MM-DD/2W~SB` is "business days, every other week". Exclusions take out holidays or term breaks,
and the time zone keeps shift times right across clock changes.

---

## Why another format?

### Cron

Cron (`0 9 * * 1-5`) is everywhere, and it is a good fit for server jobs. For business dates it
falls short:

- **No month end.** Standard cron has no "last day of the month". A job on the 31st silently
  skips every shorter month.
- **No business days.** It knows days of the week, but not holidays, and it can't move a date
  to the next working day.
- **Steps restart at every boundary.** `*/45` in the minute field means :00 and :45 of every
  hour, not every 45 minutes. Every 90 minutes needs two separate entries.
- **Cadences follow the calendar, not your start date.** `*/3` in the month field always means
  January, April, July and October. A contract that started in February has to be worked out by
  hand.
- **Surprising combinations.** When both day of month and day of week are restricted, cron
  runs when *either* matches.
- **Clock changes vary by implementation.** Whether a job in the skipped or repeated hour runs
  never, once or twice depends on the cron daemon.

Quartz-style cron adds `L` (last day), `W` (nearest weekday) and `#` (nth weekday). That
helps, but `W` only knows weekends, holiday calendars *skip* a run rather than *move* it, and
the step and calendar-alignment issues remain.

### iCalendar RRULE

RRULE (`FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=15`, used by calendar apps) is much more expressive.
It counts intervals from the start date and handles "last Friday" (`BYDAY=-1FR`). But:

- **It is long to write and hard to review.** `YY-3M-15` against
  `FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=15`.
- **No business days or holidays**, so it can't say "then the next business day".
- **Short months are dropped.** `BYMONTHDAY=31` skips months without a 31st. Falling back
  to the last day takes a different rule (`BYMONTHDAY=-1`), and you can't combine the two.
- **One date per occurrence.** There is no way to say "scheduled for the 31st, paid on the
  2nd".

### ISO 8601 repeating intervals

`R12/2025-01-31/P1M` repeats a fixed duration. It has no calendar rules at all: no weekdays,
no month end, no business days, and "one month after January 31" is left to the reader.

### Side by side

| Requirement | Cron | Quartz cron | RRULE | tkone |
|-------------|:----:|:-----------:|:-----:|:-----:|
| **Calendar** | | | | |
| Last day of the month | ✗ | ✓ `L` | ✓ | ✓ `L` |
| 31st, or the last day in shorter months | ✗ | ✗ | ✗ | ✓ `31L` |
| Last / nth weekday of the month | ✗ | ✓ | ✓ | ✓ `FRI#L`, `WED#2` |
| Every 3 months, counted from the start | ✗ | by hand | ✓ | ✓ `3M` |
| Every other week on several weekdays | ✗ | ✗ | ✓ | ✓ `[TUE,THU]/2W` |
| Rotations ("4 on, 4 off", weeks 1–2 of 4) | ✗ | ✗ | partly | ✓ `8D[1-4]`, `/4W[1,2]` |
| **Business days** | | | | |
| Nth business day of the month (with holidays) | ✗ | ✗ | ✗ | ✓ `BD#3`, `BD#[1-5]` |
| Move to the next business day (with holidays) | ✗ | ✗ | ✗ | ✓ `~NB` |
| Modified Following / Preceding | ✗ | ✗ | ✗ | ✓ `~MF`, `~MP` |
| Skip dates that aren't business days | ✗ | ✓ calendars | ✗ | ✓ `~SB` |
| N business days before a date | ✗ | ✗ | ✗ | ✓ `~2P` |
| Every N business days | ✗ | ✗ | ✗ | ✓ `5BD` |
| Both the scheduled and the adjusted date | ✗ | ✗ | ✗ | ✓ |
| **Time** | | | | |
| Several times a day, hour ranges | ✓ | ✓ | ✓ | ✓ `[09,12,17]`, `09-16` |
| Every 90 minutes | ✗ | ✗ | ✓ | ✓ `1H:30M:00` |
| Clock-change behaviour defined | ✗ | ✗ | partly | ✓ |
| **Composition and bounds** | | | | |
| Combine several rules into one schedule | ✗ | ✗ | partly | ✓ `A \| B` |
| Exclude given dates | ✗ | partly | ✓ | ✓ `! …` |
| Start, end and time zone in the text | ✗ | ✗ | ✓ | ✓ `@… ..…[Zone]` |
| Stop after N occurrences | ✗ | partly | ✓ | ✓ `x36` |

*This compares the standard formats. Some products add their own extensions.*

---

## A few real-world schedules

### Finance

| What | Schedule |
|------|----------|
| Monthly coupon on the 31st, following business day | `YY-1M-31L~NB` |
| Swap payment at month end, Modified Following | `YY-1M-L~MF` |
| Quarterly payment on the 15th, counted from the start | `YY-3M-15~NB` |
| Four quarterly coupons on the 15th | `YY-3M-15~NB @2025-03-15 x4` |
| A three-year loan: 36 payments on the 1st, Modified Following | `YY-MM-01~MF @2025-04-01 x36` |
| Payroll on the last business day of the month | `YY-MM-BD#L` |
| Settlement on the 3rd business day of the month | `YY-MM-BD#3` |
| Month-end close tasks on the first three business days and the last | `YY-MM-BD#[1-3,L]` |
| Coupon on the 15th, but only when it is a business day | `YY-MM-15~SB` |
| Quarterly expiry: 3rd Friday of Mar/Jun/Sep/Dec, preceding if a holiday | `YY-[03,06,09,12]-FRI#3~PB` |
| Weekly sweep every 5 business days | `YY-MM-5BD` |
| Every 15 months | `1Y-3M-15` |

### Reporting and operations

| What | Schedule |
|------|----------|
| Month-end report two business days early | `YY-MM-L~2P` |
| Board pack on the 2nd Wednesday at 08:00 | `YY-MM-WED#2T08:00:00` |
| Month-end close at 11:00 on the nearest weekday | `YY-1M-31L~WT11:00:00` |
| Reminders at 09:00, 12:00 and 17:00 on the 2nd business day | `YY-MM-BD#2T[09,12,17]:00:00` |
| Overnight batch at 02:00 on weekdays, except 25 December and 1 January | `YY-MM-[MON-FRI]T02:00:00 ! YY-12-25 \| YY-01-01` |
| Annual review on 15 June every two years | `2Y-06-15` |

### Intraday

| What | Schedule |
|------|----------|
| Prices every 15 minutes, 09:00–16:45, on business days | `YY-MM-1BDT09-16:15M:00` |
| Every 15 minutes during market hours, 09:30–16:00, Mon–Fri | `YY-MM-[MON-FRI]T09:[30,45]:00 \| YY-MM-[MON-FRI]T10-15:15M:00 \| YY-MM-[MON-FRI]T16:00:00` |
| Snapshot every 30 minutes on Mondays, Wednesdays and Fridays | `YY-MM-[MON,WED,FRI]THH:30M:00` |

### Rostering and timetables

| What | Schedule |
|------|----------|
| Nurses' rota: day shifts 07:00, two weeks on and two off | `YY-MM-[MON-FRI]/4W[1,2]T07:00:00 @2025-01-06[Europe/London]` |
| Plant shift pattern: four days on, four off, from 06:00 | `YY-MM-8D[1-4]T06:00:00 @2025-01-01[America/Chicago]` |
| Fortnightly tutorial on Tuesday and Thursday, term time only | `YY-MM-[TUE,THU]/2WT14:00:00 ! YY-[07-08]-DD @2025-01-07` |

---

## What it doesn't do (yet)

The format is deliberately small. Some things it leaves out:

- **Occurrences are moments, not periods.** A shift is "07:00", not "07:00 to 19:00"; there is
  no duration.
- **One start for every part.** All parts of a `|` combination count from the same start;
  a part can't be offset by a week or a day.
- **Weeks always run Monday to Sunday.** There is no Sunday-start or ISO week-number rule.
- **No holiday data.** Holiday calendars are supplied with a schedule, not written in it.

---

## Where to go next

- [README](README.md): using the tkone library: building and resuming schedules, builder
  options (`with_count`, clock alignment, daylight-saving modes) and plain-English descriptions
- [Grammar and rules](GRAMMAR.md): the precise definition, for validating or implementing the
  format
- [Date spec reference](src/date/date-spec.md)
- [Time spec reference](src/time/time-spec.md)
- [Combined date and time reference](src/datetime/date-time-spec.md)
