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
YY-1M-31L~NB          the 31st of every month (or the last day), then the next business day
YY-MM-L~2P            two business days before the last day of every month
YY-3M-15              the 15th of every 3rd month, counted from the start
YY-MM-DDT1H:30M:00    every day, every 90 minutes
```

---

## The format in five minutes

A schedule has a **date part**, a **time part**, or both joined by `T`:

```text
<years>-<months>-<days>[~<adjustment>]  T  <hours>:<minutes>:<seconds>
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
| Business days | `BD#3` · `BD#L` · `BD#2L` | 3rd business day · last business day · 2nd-to-last business day |
| Steps | `7D` · `5BD` · `3WD` | every 7 days · every 5 business days · every 3 weekdays |

Ranges such as `[01-06]` are written inside brackets, because `-` also separates the parts.

`2Y` and `3M` are counted from the schedule's start, not from January. Started in February,
`YY-3M-15` runs in February, May, August and November.

### Adjustment (optional)

What to do when the date lands on a day you can't use. Business days come from a holiday
calendar that your application provides.

| Adjustment | Meaning |
|------------|---------|
| `~NB` · `~PB` · `~B` | if not a business day: move to the next · previous · nearest one |
| `~MF` · `~MP` | Modified Following · Modified Preceding: as `~NB` · `~PB`, but never into another month |
| `~NW` · `~PW` · `~W` | if on a weekend: move to the next · previous · nearest weekday |
| `~2N` · `~3P` | always move 2 business days later · 3 business days earlier |

### Time part

Each field is a wildcard (`HH`, `MM`, `SS`), a fixed value (`09`, `30`, `00`), a list or range
of values (`[09,12,17]`, `09-16`), a step (`1H`, `30M`, `15S`), or `_` to keep the start's
value.

| Time part | Meaning |
|-----------|---------|
| `09:30:00` | at 09:30 |
| `HH:30M:00` | every 30 minutes |
| `1H:30M:00` | every 90 minutes |
| `1H:_:_` | every hour, at the start's minute and second |
| `09:15M:00` | every 15 minutes, only during the 09:00 hour |
| `09-16:15M:00` | every 15 minutes from 09:00 to 16:45 |
| `[09,12,17]:00:00` | at 09:00, 12:00 and 17:00 |
| `HH:[00,30]:00` | on the hour and half hour |
| `HH:MM:00` | every minute |

Steps are counted from the schedule's start: `HH:15M:00` started at 09:07 runs at 09:22, 09:37,
and so on. If you want them on the clock (:00, :15, :30, :45) like cron, ask for midnight
alignment when the schedule is built.

### Combining schedules

Join any number of schedules with `|` to get all their occurrences, in order. This covers the
irregular cases that one rule can't:

```text
YY-MM-BD#3 | YY-MM-L~PB                       the 3rd and the last business day of each month
09:[30,45]:00 | 10-15:15M:00 | 16:00:00       every 15 minutes from 09:30 to 16:00
YY-MM-BD#1T09:00:00 | YY-MM-FRI#LT17:00:00    09:00 on the 1st business day, 17:00 on the last Friday
```

Not sure how a schedule will be read? The library can describe any schedule in plain English:

```text
YY-1M-31L~WT11:00:00  →  on the 31st of every month (or the last day, in shorter months),
                         moved to the nearest weekday when it falls on a weekend; at 11:00:00
```

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
| Last day of the month | ✗ | ✓ `L` | ✓ | ✓ `L` |
| 31st, or the last day in shorter months | ✗ | ✗ | ✗ | ✓ `31L` |
| Last / nth weekday of the month | ✗ | ✓ | ✓ | ✓ `FRI#L`, `WED#2` |
| Nth business day of the month (with holidays) | ✗ | ✗ | ✗ | ✓ `BD#3`, `BD#L` |
| Several times a day, hour ranges | ✓ | ✓ | ✓ | ✓ `[09,12,17]`, `09-16` |
| Every 3 months, counted from the start | ✗ | by hand | ✓ | ✓ `3M` |
| Every 90 minutes | ✗ | ✗ | ✓ | ✓ `1H:30M:00` |
| Move to the next business day (with holidays) | ✗ | ✗ | ✗ | ✓ `~NB` |
| Modified Following / Preceding | ✗ | ✗ | ✗ | ✓ `~MF`, `~MP` |
| N business days before a date | ✗ | ✗ | ✗ | ✓ `~2P` |
| Every N business days | ✗ | ✗ | ✗ | ✓ `5BD` |
| Both the scheduled and the adjusted date | ✗ | ✗ | ✗ | ✓ |
| Clock-change behaviour defined and selectable | ✗ | ✗ | partly | ✓ |
| Combine several rules into one schedule | ✗ | ✗ | partly | ✓ `A \| B` |
| Plain-English description of a schedule | ✗ | ✗ | ✗ | ✓ |

*This compares the standard formats. Some products add their own extensions.*

---

## What makes it a good fit for finance

### The scheduled date and the paid date are both kept

When a payment due on Saturday the 31st moves to Monday the 2nd, both dates matter: interest
accrues to the 31st, while cash moves on the 2nd. Every occurrence reports:

- **actual**: the date the rule produced (the 31st)
- **observed**: the date after adjustment (the 2nd)

It is also labelled *exact*, *adjusted later* or *adjusted earlier*, so the moved dates are easy
to find in a report.

### Business-day conventions are built into the text

The common conventions map directly onto adjustments:

| Market convention | tkone |
|-------------------|-------|
| Following | `~NB` |
| Modified Following | `~MF` |
| Preceding | `~PB` |
| Modified Preceding | `~MP` |
| Nearest business day | `~B` |
| N business days before / after | `~NP` / `~NN` |

The holiday calendar is supplied by your application (one per market, if needed), so the same
schedule text works for London and New York.

### Month end is a choice, not an accident

`31L` clamps to the last day, `31N` moves to the 1st of the next month, `31O` overflows by the
missing days, and plain `31` skips short months. You pick the rule in the schedule, and nothing
is decided silently.

### Clock changes are handled the same way every time

Each schedule runs in a named time zone (for example `Europe/London`) with a defined rule for
daylight saving:

- **A time that doesn't exist** (02:30 when the clocks jump from 02:00 to 03:00) moves later
  by the size of the jump, and is labelled as moved.
- **A time that happens twice** (when the clocks fall back) runs once, not twice.
- **Every 6 hours** can mean *by the clock on the wall* (the default: started at midnight, it
  runs at 00:00, 06:00, 12:00 and 18:00 every day) or *6 real hours apart*, which matters for
  things like interest accrual or monitoring.
- A strict mode turns any of these into an error instead, for systems that must never guess.

### Start, end and resume

- A schedule can have an **inclusive end**, given as a date or as another schedule ("until
  the last business day of the year").
- A schedule is **just text**. Store the text, the time zone and the last occurrence, and the
  next one can be computed at any time, on any machine. A resumed schedule gives exactly the
  same dates as one that ran without stopping, including across clock changes and business-day
  adjustments.

---

## A few real-world schedules

| What | Schedule |
|------|----------|
| Monthly coupon on the 31st, following business day | `YY-1M-31L~NB` |
| Swap payment at month end, Modified Following | `YY-1M-L~MF` |
| Quarterly payment on the 15th, counted from the start | `YY-3M-15~NB` |
| Payroll on the last business day of the month | `YY-MM-BD#L` |
| Settlement on the 3rd business day of the month | `YY-MM-BD#3` |
| Month-end report two business days early | `YY-MM-L~2P` |
| Quarterly expiry: 3rd Friday of Mar/Jun/Sep/Dec, preceding if a holiday | `YY-[03,06,09,12]-FRI#3~PB` |
| Annual review on 15 June every two years | `2Y-06-15` |
| Every 15 months | `1Y-3M-15` |
| Weekly sweep every 5 business days | `YY-MM-5BD` |
| Board pack on the 2nd Wednesday at 08:00 | `YY-MM-WED#2T08:00:00` |
| Month-end close at 11:00 on the nearest weekday | `YY-1M-31L~WT11:00:00` |
| Snapshot every 30 minutes on Mondays, Wednesdays and Fridays | `YY-MM-[MON,WED,FRI]THH:30M:00` |
| Prices every 15 minutes, 09:00–16:45, on business days | `YY-MM-1BDT09-16:15M:00` |
| Every 15 minutes during market hours, 09:30–16:00, Mon–Fri | `YY-MM-[MON-FRI]T09:[30,45]:00 \| YY-MM-[MON-FRI]T10-15:15M:00 \| YY-MM-[MON-FRI]T16:00:00` |
| Reminders at 09:00, 12:00 and 17:00 on the 2nd business day | `YY-MM-BD#2T[09,12,17]:00:00` |

---

## What it doesn't do (yet)

The format is deliberately small. Some things it leaves out:

- **No exclusion dates or occurrence counts.** A schedule is bounded by an end date or an end
  schedule, not "stop after 12 times" or "except 25 December".
- **No built-in holiday data.** Holiday calendars come from your application.

---

## Where to go next

- [README](README.md): installing and using the library
- [Date spec reference](src/date/date-spec.md)
- [Time spec reference](src/time/time-spec.md)
- [Combined date and time reference](src/datetime/date-time-spec.md)
