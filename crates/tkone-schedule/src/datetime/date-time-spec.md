# Datetime Schedule Specification

## Format

```text
<date_spec>T<time_spec>
```

The `T` separator joins a [date spec](../date/spec.md) and a [time spec](../time/spec.md).
Both parts are required. Refer to those documents for the full syntax of each component.

### Separator detection

The `T` character is only treated as the separator when it is immediately followed by a
time-like token: `HH:`, `_:`, a bare `<n>H:`, or a two-digit leading hour `<0–2><0–9>:`.
This prevents weekday names like `TUE` and `THU` from being confused with the separator.

```text
YY-MM-THUT11:00:00  →  date="YY-MM-THU"  time="11:00:00"   ✓ THU not confused
YY-MM-TUET11:00:00  →  date="YY-MM-TUE"  time="11:00:00"   ✓ TUE not confused
YY-MM-DDTHH:30M:00  →  date="YY-MM-DD"   time="HH:30M:00"  ✓ HH: recognised
YY-MM-DDT_:_:_      →  date="YY-MM-DD"   time="_:_:_"       ✓ _: recognised
```

---

## How Combining Works

### 1. Date spec provides valid calendar dates

The date iterator advances through valid calendar dates according to the date spec — monthly,
weekly, every-N-days, weekday-of-month, etc. It knows nothing about time.

`nY` / `nM` cadences are aligned to the date of the start (or the `new_after` cursor): `YY-3M-01T09:00:00`
started on 2025-01-01 visits Jan, Apr, Jul, Oct.

### 2. Time spec provides intra-day ticks

For each date, the time spec produces its times inside the window `[date 00:00, next 00:00)`.
When the next time would fall outside the window, the iterator moves to the next date. This is
how "every Monday, every 30 minutes" emits 48 ticks on each Monday before moving on.

A time spec is a regular grid (see the [time spec semantics](crate::time)): a step, a phase, and
optional windows for fixed coarser fields. Each date gets every grid point inside its window.

### 3. Which times each date gets — `TimeAnchor`

The grid's phase, and the values of carried fields (`ForEach` finer than the step, `AsIs`), come
from an anchor chosen with `.with_time_anchor(…)`:

| `TimeAnchor` | Anchor on each date | `1H:MM:SS` from 09:22:45 | `2H:00:00` from 09:00 | `_:_:_` from 14:30 |
|--------------|---------------------|--------------------------|-----------------------|--------------------|
| `Start` (default) | the start's time of day | hh:22:45 every hour | 01:00, 03:00, … 23:00 | 14:30 |
| `Midnight` | 00:00:00 | hh:00:00 every hour | 00:00, 02:00, … 22:00 | 00:00 |

With `Start`, every date gets the same times, and steps keep one phase across midnight
(`2H` from 19:15 → 21:00, 23:00, 01:00, …). With `Midnight`, every date — including the start
date — restarts the time spec from midnight.

### 4. The start date

- **`new_with_start`**: `start` is returned first, as-is (see below), and only times *strictly
  after* it are emitted for the rest of that date.
- **`new_after`**: the first result must be strictly after `dtm`. If no time on the current date
  is still ahead, the first result comes from the *next* valid date.

A mid-day start therefore usually has fewer ticks on its first date than on later dates.

`start` is yielded as `Exact` even when it does not match the spec, and no `~` adjustment is
applied to it. This lets you feed an already-adjusted (observed) datetime back in as the start.

### 5. `AdjustedLater` / `AdjustedEarlier` propagation

When the date spec applies an adjustment (e.g. `~NB`, or `31N` overflow), **every** tick on the
adjusted (observed) date carries it. `actual` is the raw date at the tick's time; `observed` is
the tick itself.

```text
May 31 (Sat) ~NB → Jun 2 (Mon), time spec HH:30M:00:

 AdjustedLater(actual=2025-05-31 00:00, observed=2025-06-02 00:00)
 AdjustedLater(actual=2025-05-31 00:30, observed=2025-06-02 00:30)
 AdjustedLater(actual=2025-05-31 01:00, observed=2025-06-02 01:00)
 ...
```

Every raw date is kept, even when its observed date repeats or goes back. `YY-MM-DD~PBT12H:00:00`
from a Friday gives Fri 12:00, then Sat 00:00 / 12:00 and Sun 00:00 / 12:00 — all observed on
Friday — then Mon 00:00. `actual` always increases; `observed` may not.

DST gaps also produce `AdjustedLater(label, shifted)`, on any tick whose local time does not
exist. See [Daylight Saving Time](#daylight-saving-time).

### 6. End bound

`with_end(end)` is **inclusive**: ticks up to and including `end` are yielded, and the iterator
stops at the first tick whose actual or observed time is after `end`. Nothing is invented at the
boundary.

`with_end_spec(spec)` takes another datetime spec; `end` is its first occurrence after the start
(its observed time). `YY-MM-DDT4H:00:00` from 2025-01-15 09:00 with
`.with_end_spec("YY-MM-16T09:00:00")` yields 09:00, 13:00, 17:00, 21:00 on the 15th and 01:00,
05:00, 09:00 on the 16th.

### 7. Resuming a stored schedule

To continue from an occurrence stored in a database, use
`SpecIteratorBuilder::resume_after(spec, bdp, tz, last_actual)`, where `last_actual` is the
stored ``actual()`` parsed back from RFC 3339. Store it with its UTC offset and rebuild with the
same options; add `.with_end(end)` if the schedule has a stored end. The crate docs' section *Persisting and resuming a schedule* has the full
pattern and the rules that keep resumed results identical to a continuous iterator.

---

## Result Types

Same three variants as the date iterator, now carrying `NaiveDateTime` or `DateTime<Tz>`:

| Variant | Meaning |
|---------|---------|
| `Exact(dt)` | No adjustment; `dt` is exactly as computed. |
| `AdjustedLater(actual, observed)` | `actual` is the raw date + the tick's time; `observed` is the adjusted (later) date + same time. On every tick of an adjusted date. |
| `AdjustedEarlier(actual, observed)` | As above but the adjustment moved the date earlier. |
| `AdjustedLater(label, shifted)` | The tick's local time falls in a DST gap; see [Daylight Saving Time](#daylight-saving-time). Any tick. |

---

## Examples

All examples use `new_with_start` unless stated otherwise.
The start datetime is always returned as the first result (passthrough).

---

### Group 1 — Fixed time: one tick per date

---

### 1. `YY-1M-31L~NBT11:00:00` — Last business day of each month at 11:00

**Start:** 2025-01-31 11:00:00

The date spec visits the last calendar day of each month (`31L` = clamp to last day), then
adjusts to the next business day if it falls on a weekend (`~NB` = next business day if not
already one using the configured processor).  One tick per date at 11:00.

```text
 1. Exact         2025-01-31 11:00:00  (Fri)
 2. Exact         2025-02-28 11:00:00  (Fri)
 3. Exact         2025-03-31 11:00:00  (Mon)
 4. Exact         2025-04-30 11:00:00  (Wed)
 5. AdjustedLater actual=2025-05-31 11:00:00  observed=2025-06-02 11:00:00  (Sat→Mon)
 6. Exact         2025-06-30 11:00:00  (Mon)
 7. Exact         2025-07-31 11:00:00  (Thu)
 8. AdjustedLater actual=2025-08-31 11:00:00  observed=2025-09-01 11:00:00  (Sun→Mon)
 9. Exact         2025-09-30 11:00:00  (Tue)
10. Exact         2025-10-31 11:00:00  (Fri)
11. AdjustedLater actual=2025-11-30 11:00:00  observed=2025-12-01 11:00:00  (Sun→Mon)
12. Exact         2025-12-31 11:00:00  (Wed)
13. AdjustedLater actual=2026-01-31 11:00:00  observed=2026-02-02 11:00:00  (Sat→Mon)
14. AdjustedLater actual=2026-02-28 11:00:00  observed=2026-03-02 11:00:00  (Sat→Mon)
15. Exact         2026-03-31 11:00:00  (Tue)
```

---

### 2. `YY-MM-[MON,WED,FRI]T09:30:00` — Monday, Wednesday, Friday at 09:30

**Start:** 2025-01-06 09:30:00 (Monday)

Three fixed-time ticks per week.  Typical for a market-open notification.

```text
 1. 2025-01-06 09:30:00  Mon
 2. 2025-01-08 09:30:00  Wed
 3. 2025-01-10 09:30:00  Fri
 4. 2025-01-13 09:30:00  Mon
 5. 2025-01-15 09:30:00  Wed
 6. 2025-01-17 09:30:00  Fri
 7. 2025-01-20 09:30:00  Mon
 8. 2025-01-22 09:30:00  Wed
 9. 2025-01-24 09:30:00  Fri
10. 2025-01-27 09:30:00  Mon
11. 2025-01-29 09:30:00  Wed
12. 2025-01-31 09:30:00  Fri
13. 2025-02-03 09:30:00  Mon
14. 2025-02-05 09:30:00  Wed
15. 2025-02-07 09:30:00  Fri
```

---

### 3. `YY-MM-FRIT16:30:00` — Every Friday at 16:30

**Start:** 2025-01-03 16:30:00 (Friday)

```text
 1. 2025-01-03 16:30:00
 2. 2025-01-10 16:30:00
 3. 2025-01-17 16:30:00
 4. 2025-01-24 16:30:00
 5. 2025-01-31 16:30:00
 6. 2025-02-07 16:30:00
 7. 2025-02-14 16:30:00
 8. 2025-02-21 16:30:00
 9. 2025-02-28 16:30:00
10. 2025-03-07 16:30:00
11. 2025-03-14 16:30:00
12. 2025-03-21 16:30:00
13. 2025-03-28 16:30:00
14. 2025-04-04 16:30:00
15. 2025-04-11 16:30:00
```

---

### 4. `YY-3M-15T09:00:00` — 15th of each quarter at 09:00

**Start:** 2025-01-15 09:00:00

One tick every three months.  Useful for quarterly reporting reminders.

```text
 1. 2025-01-15 09:00:00
 2. 2025-04-15 09:00:00
 3. 2025-07-15 09:00:00
 4. 2025-10-15 09:00:00
 5. 2026-01-15 09:00:00
 6. 2026-04-15 09:00:00
 7. 2026-07-15 09:00:00
 8. 2026-10-15 09:00:00
 9. 2027-01-15 09:00:00
10. 2027-04-15 09:00:00
```

---

### 5. `YY-MM-MON#1T09:30:00` — First Monday of each month at 09:30

**Start:** 2025-01-06 09:30:00

```text
 1. 2025-01-06 09:30:00
 2. 2025-02-03 09:30:00
 3. 2025-03-03 09:30:00
 4. 2025-04-07 09:30:00
 5. 2025-05-05 09:30:00
 6. 2025-06-02 09:30:00
 7. 2025-07-07 09:30:00
 8. 2025-08-04 09:30:00
 9. 2025-09-01 09:30:00
10. 2025-10-06 09:30:00
11. 2025-11-03 09:30:00
12. 2025-12-01 09:30:00
13. 2026-01-05 09:30:00
14. 2026-02-02 09:30:00
15. 2026-03-02 09:30:00
```

---

### Group 2 — Every-N-hours: several ticks per date

---

### 6. `YY-MM-DDT6H:00:00` — Every day, every 6 hours

**Start:** 2025-01-01 06:00:00

The 6-hour grid passes through 06:00, so every date gets 00:00, 06:00, 12:00 and 18:00.
The start date only has the ticks from 06:00 (passthrough) onwards.

```text
 1. 2025-01-01 06:00:00  ← start (passthrough)
 2. 2025-01-01 12:00:00
 3. 2025-01-01 18:00:00
 4. 2025-01-02 00:00:00  ← first tick of the new day
 5. 2025-01-02 06:00:00
 6. 2025-01-02 12:00:00
 7. 2025-01-02 18:00:00
 8. 2025-01-03 00:00:00
 9. 2025-01-03 06:00:00
10. 2025-01-03 12:00:00
11. 2025-01-03 18:00:00
12. 2025-01-04 00:00:00
13. 2025-01-04 06:00:00
14. 2025-01-04 12:00:00
15. 2025-01-04 18:00:00
```

Jan 1 produces only 3 ticks (06:00–18:00) because the start time is mid-cycle.
All subsequent days produce 4 ticks (00:00, 06:00, 12:00, 18:00).

---

### 7. `YY-MM-[MON]T1H:00:00` — Every Monday, every hour on the hour

**Start:** 2025-01-06 09:00:00 (Monday)

The date spec visits only Mondays.  The time spec fills the day with 24 hourly ticks
starting at midnight — except the initial Monday, which starts at the passthrough time.

```text
 1. 2025-01-06 09:00:00  ← Mon, passthrough
 2. 2025-01-06 10:00:00
 3. 2025-01-06 11:00:00
 4. 2025-01-06 12:00:00
 5. 2025-01-06 13:00:00
 6. 2025-01-06 14:00:00
 7. 2025-01-06 15:00:00
 8. 2025-01-06 16:00:00
 9. 2025-01-06 17:00:00
10. 2025-01-06 18:00:00
11. 2025-01-06 19:00:00
12. 2025-01-06 20:00:00
13. 2025-01-06 21:00:00
14. 2025-01-06 22:00:00
15. 2025-01-06 23:00:00
    (Mon Jan 13: 00:00, 01:00, ..., 23:00 — all 24 ticks)
```

---

### 8. `YY-1M-15T4H:00:00` — 15th of each month, every 4 hours

**Start:** 2025-01-15 08:00:00

The 4-hour grid passes through 08:00, so it includes midnight. The start date only has the
ticks from 08:00 onwards.

```text
 1. 2025-01-15 08:00:00  ← passthrough
 2. 2025-01-15 12:00:00
 3. 2025-01-15 16:00:00
 4. 2025-01-15 20:00:00
 5. 2025-02-15 00:00:00  ← midnight included from Feb onwards
 6. 2025-02-15 04:00:00
 7. 2025-02-15 08:00:00
 8. 2025-02-15 12:00:00
 9. 2025-02-15 16:00:00
10. 2025-02-15 20:00:00
11. 2025-03-15 00:00:00
12. 2025-03-15 04:00:00
13. 2025-03-15 08:00:00
14. 2025-03-15 12:00:00
15. 2025-03-15 16:00:00
```

Jan 15 emits only 4 ticks (08:00–20:00).  Feb 15 onward emits 6 ticks (00:00–20:00).

---

### Group 3 — Every-N-minutes: many ticks per date

---

### 9. `YY-1M-31L~NBTHH:30M:00` — Last business day of month, every 30 minutes

**Start:** 2025-01-31 08:00:00

Within each valid date, up to 48 ticks are emitted (00:00–23:30).
The initial date (Jan 31) starts at 08:00, so it emits 32 ticks (08:00–23:30).

Business-day adjustment: **every** tick of an adjusted date is `AdjustedLater`.

```text
 1. 2025-01-31 08:00:00  ← passthrough (Fri)
 2. 2025-01-31 08:30:00
 3. 2025-01-31 09:00:00
 4. 2025-01-31 09:30:00
 5. 2025-01-31 10:00:00
 6. 2025-01-31 10:30:00
 7. 2025-01-31 11:00:00
 8. 2025-01-31 11:30:00
 9. 2025-01-31 12:00:00
10. 2025-01-31 12:30:00
11. 2025-01-31 13:00:00
12. 2025-01-31 13:30:00
13. 2025-01-31 14:00:00
14. 2025-01-31 14:30:00
15. 2025-01-31 15:00:00
    (...continues to 23:30 on Jan 31...)
    (Feb 28: 00:00, 00:30, 01:00, ... 48 ticks, all Exact)
    (May 31 Sat → Jun 2 Mon: all 48 ticks AdjustedLater)
```

---

### 10. `YY-MM-1BDT4H:00:00` — Every business day, every 4 hours

**Start:** 2025-01-01 09:00:00 (Wednesday)

`1BD` visits every business day (Mon–Fri).  Weekends are skipped. The 4-hour grid passes
through 09:00, so every date gets 01:00, 05:00, 09:00, 13:00, 17:00, 21:00.

```text
 1. 2025-01-01 09:00:00  ← Wed, passthrough
 2. 2025-01-01 13:00:00
 3. 2025-01-01 17:00:00
 4. 2025-01-01 21:00:00
 5. 2025-01-02 01:00:00  ← Thu, same phase as the start
 6. 2025-01-02 05:00:00
 7. 2025-01-02 09:00:00
 8. 2025-01-02 13:00:00
 9. 2025-01-02 17:00:00
10. 2025-01-02 21:00:00
11. 2025-01-03 01:00:00  ← Fri
12. 2025-01-03 05:00:00
13. 2025-01-03 09:00:00
14. 2025-01-03 13:00:00
15. 2025-01-03 17:00:00
    (2025-01-03 21:00)
    (Sat Jan 4, Sun Jan 5: skipped)
    (Mon Jan 6: 01:00, 05:00, 09:00, 13:00, 17:00, 21:00)
```

With `.with_time_anchor(TimeAnchor::Midnight)` every date — including Jan 1 — uses the
midnight-aligned grid instead: Jan 1 gives 09:00 (passthrough), 12:00, 16:00, 20:00, and later
dates 00:00, 04:00, … 20:00.

---

### Group 4 — Overflow and business-day adjustment with time

---

### 11. `YY-1M-31NT11:00:00` — 31st-or-next-month at 11:00

**Start:** 2025-01-31 11:00:00

`31N` overflows to 1st of next month when the month has fewer than 31 days.
One tick per date at 11:00; result type carries the overflow metadata.

```text
 1. Exact         2025-01-31 11:00:00
 2. AdjustedLater actual=2025-02-28 11:00:00  observed=2025-03-01 11:00:00
 3. Exact         2025-03-31 11:00:00
 4. AdjustedLater actual=2025-04-30 11:00:00  observed=2025-05-01 11:00:00
 5. Exact         2025-05-31 11:00:00
 6. AdjustedLater actual=2025-06-30 11:00:00  observed=2025-07-01 11:00:00
 7. Exact         2025-07-31 11:00:00
 8. Exact         2025-08-31 11:00:00
 9. AdjustedLater actual=2025-09-30 11:00:00  observed=2025-10-01 11:00:00
10. Exact         2025-10-31 11:00:00
11. AdjustedLater actual=2025-11-30 11:00:00  observed=2025-12-01 11:00:00
12. Exact         2025-12-31 11:00:00
13. Exact         2026-01-31 11:00:00
14. AdjustedLater actual=2026-02-28 11:00:00  observed=2026-03-01 11:00:00
15. Exact         2026-03-31 11:00:00
```

---

### 12. `YY-1M-31L~NBTH:30M:00` (sub-daily on adjusted date)

**Start:** 2025-01-31 08:00:00

When May 31 adjusts to Jun 2, **every** 30-minute tick on Jun 2 is `AdjustedLater`, with
`actual` on May 31 at the same time.

Illustrative slice around the May/June boundary (Jan–Apr and Jun–Dec are analogous to
example 9):

```text
  (... Jan 31, Feb 28, Mar 31, Apr 30 as in example 9 ...)

  May 31 (Sat) → observed Jun 2 (Mon):
  AdjustedLater actual=2025-05-31 00:00:00  observed=2025-06-02 00:00:00
  AdjustedLater actual=2025-05-31 00:30:00  observed=2025-06-02 00:30:00
  ...
  AdjustedLater actual=2025-05-31 23:30:00  observed=2025-06-02 23:30:00

  Jun 30 (Mon) → Exact, no adjustment:
  Exact         2025-06-30 00:00:00
  ...
```

---

### Group 5 — `new_after` semantics

---

### 13. `YY-1M-15T11:00:00` via `new_after` — skipping a past time on the initial date

**`new_after` from:** 2025-01-15 12:00:00

The 11:00 slot for Jan 15 has already passed at 12:00, so Jan 15 has no time left and the
first result is Feb 15.

```text
 1. 2025-02-15 11:00:00  ← Jan 15 silently skipped
 2. 2025-03-15 11:00:00
 3. 2025-04-15 11:00:00
 4. 2025-05-15 11:00:00
 5. 2025-06-15 11:00:00
 6. 2025-07-15 11:00:00
 ...
```

---

### 14. `YY-MM-DDT1H:00:00` via `new_after` — non-aligned cursor

**`new_after` from:** 2025-01-15 09:30:00

The hourly grid is on the hour (minutes and seconds are fixed at `00`), so the first grid point
after 09:30 is 10:00. Subsequent results are 11:00, 12:00 … (exact 1 h gaps).

```text
 1. 2025-01-15 10:00:00  ← first tick after 09:30 (not 10:30)
 2. 2025-01-15 11:00:00
 3. 2025-01-15 12:00:00
 4. 2025-01-15 13:00:00
 5. 2025-01-15 14:00:00
 ...
```

---

### Group 6 — `AsIs` (`_`) and carry edge cases

---

### 15. `YY-1M-31LT_:_:_` — AsIs time on a monthly date spec

**Start:** 2025-01-31 14:30:00

`_:_:_` (all `AsIs`) keeps the anchor's time. With the default `TimeAnchor::Start` that is the
start's 14:30 on every date:

```text
 1. 2025-01-31 14:30:00  ← passthrough
 2. 2025-02-28 14:30:00
 3. 2025-03-31 14:30:00
 4. 2025-04-30 14:30:00
 ...
```

With `TimeAnchor::Midnight` every later date gets 00:00:00 instead.

---

### 16. `YY-1M-15T1H:MM:SS` — Hourly with minute/second carry

**Start:** 2025-01-15 09:22:45

`Every(1H)` drives; `MM` and `SS` (ForEach) carry the anchor's values. With the default
`TimeAnchor::Start` every date runs at `hh:22:45`:

```text
 1. 2025-01-15 09:22:45  ← passthrough
 2. 2025-01-15 10:22:45
 ...
15. 2025-01-15 23:22:45
16. 2025-02-15 00:22:45  ← same carry on the next date
17. 2025-02-15 01:22:45
    ...
```

With `TimeAnchor::Midnight` every date — including Jan 15 after the passthrough — runs at
`hh:00:00`: 09:22:45 (passthrough), 10:00:00, 11:00:00, …, then Feb 15 00:00:00, 01:00:00, …

---

## Daylight Saving Time

Times are computed on the wall clock (like cron), then mapped to real instants with the
builder's `DstPolicy` (`Adjust` by default).

- **Gap (spring-forward, skipped day):** a tick whose local time does not exist moves later by the
  length of the gap and is reported as `AdjustedLater(label, shifted)`. `label` shows the time as
  scheduled, with the offset in force before the gap; `shifted` shows it in the real offset. Both
  are the **same instant**, so `actual == observed` when compared.
- **Overlap (fall-back):** the repeated hour is visited once, at the earlier offset.
- **No duplicates:** results strictly increase. A tick that lands on (or before) the previous
  result's instant is dropped.
- **`DstPolicy::Strict`:** any gap or overlap returns `Error::AmbiguousLocalTime` instead.
- **`IntervalMode::Elapsed`** (`.with_interval_mode(…)`): the first time on each date still comes
  from the wall clock; later times that day are whole steps of real time after the previous one.
  Across fall-back, `YY-MM-DDT1H:00:00` from 00:00 then gives 00:00 EDT, 01:00 EDT, 01:00 EST,
  02:00 EST instead of 00:00, 01:00 EDT, 02:00 EST.

### `YY-MM-DDT1H:00:00` in America/New_York on 2025-03-09

```text
Exact(00:00 -05:00)
Exact(01:00 -05:00)
AdjustedLater(02:00 -05:00, 03:00 -04:00)   ← 02:00 does not exist
Exact(04:00 -04:00)                         ← the real 03:00 is dropped as a duplicate
```

### `YY-MM-DDT02:30:00` across the same change

```text
2025-03-08  Exact(02:30 -05:00)
2025-03-09  AdjustedLater(02:30 -05:00, 03:30 -04:00)
2025-03-10  Exact(02:30 -04:00)
```

---

## Semantically Surprising Combinations

The following combinations are valid and parse without error but produce behaviour that
frequently surprises callers. They are not bugs; they are documented here so the semantics
are understood before use.

---

### Initial-date tick count differs from all subsequent dates

**Spec:** any sub-daily time spec (e.g. `1H:00:00`, `HH:30M:00`)
**Behaviour:** when using `new_with_start` at a non-midnight start time, the initial date
emits fewer ticks than subsequent dates.

```text
YY-MM-DDT1H:00:00  start=2025-01-01 09:00:00

Jan 1:  09:00, 10:00, ..., 23:00         (15 ticks — from start time)
Jan 2:  00:00, 01:00, ..., 23:00         (24 ticks — full day)
Jan 3:  00:00, 01:00, ..., 23:00         (24 ticks)
```

Callers that assume a constant tick count per date must account for this asymmetry on the first
date.

---

### `AdjustedLater.actual` carries the observed time, not the pre-adjustment time

**Spec:** date spec with adjustment and any time spec

The `actual` field's *date* is the pre-adjustment calendar date; its *time* is identical to
the *observed* tick's time.  There is no "natural time for the actual date" concept — the
time spec drives the observed date's window and the actual date simply inherits that time.

```text
YY-1M-31L~NBT11:00:00

May 31 (Sat) → Jun 2 (Mon):
 AdjustedLater(
   actual   = 2025-05-31 11:00:00,   ← date is May 31; time is the same 11:00
   observed = 2025-06-02 11:00:00,
 )
```

`actual.date() = May 31` and `actual.time() = 11:00:00` (the observed time, not a "natural"
May 31 time).

---

### `new_after` skips the entire initial date when its time window has passed

**Spec:** any spec with a fixed or early-in-day time

If the time spec's only (or first) tick on the initial date falls at or before `dtm`, the
iterator skips the initial date entirely and the first result is from the next valid date.

```text
YY-1M-15T11:00:00  via new_after from 2025-01-15 12:00:00

→ Jan 15 skipped; first result = 2025-02-15 11:00:00
```

Use `new_with_start` if you want the `dtm` itself returned as the first result.

---

### High-frequency time specs without an end bound produce unbounded output

**Spec:** sub-minute time specs (`HH:MM:SS`, `HH:MM:00`) on daily or more-frequent date specs

```text
YY-MM-DDTHH:MM:SS  →  86 400 results per day  (every second)
YY-MM-DDTHH:MM:00  →   1 440 results per day  (every minute)
YY-MM-DDTHH:30M:00 →      48 results per day  (every 30 minutes)
```

Always pair sub-minute specs with a `with_end` boundary or consume with a bounded `take`.

---

### All-`At` time spec produces one tick per date (or terminates as standalone)

**Spec:** `hh:mm:ss` (e.g. `11:00:00`, `16:30:00`)

A time spec without a step names one time of day. Within a datetime spec that time is used on
every date, giving one tick per date.

Using an all-`At` spec as the **time iterator** in isolation (no date spec) yields that time at
most once, on the current day, then terminates.

- `new_with_start` from `11:00:00` → yields `11:00:00` (passthrough) then terminates.
- `new_after` from `11:00:00` → yields nothing (guard fires on first tick).
- `new_after` from `10:00:00` → yields `11:00:00` once, then terminates.

All-`At` time specs remain most useful when:

1. Used as the time component of a datetime spec (where the date iterator drives progression).
2. Used as an `end_spec` boundary (resolves to the next such time after the start).

---

### Summary table

| Combination | Surprising behaviour | Safe alternative |
|-------------|----------------------|-----------------|
| Sub-daily start mid-day (`new_with_start`) | Initial date has fewer ticks than later dates | Expected; document for callers |
| Conditional adjustment on a frequent date spec (`YY-MM-DD~NB`, `~PB`) | Every raw date is kept, so observed times can repeat (Sat, Sun → Mon) or go back (`~PB`); `actual` always increases | Use a date spec that only hits business days (`1BD`) if one tick per observed time is needed |
| Tick inside a DST gap | `AdjustedLater(label, shifted)` with `actual == observed` as instants; a colliding later tick is dropped | Compare `naive_local()` to see the shift; use `DstPolicy::Strict` to fail instead |
| `AdjustedLater.actual.time()` | Same as observed time; not the "natural" time for the unadjusted date | Treat `actual.date()` as the logical date; `observed` as the real datetime |
| `new_after` with past daily time | Entire initial date silently skipped | Use `new_with_start` if start inclusivity is required |
| `HH:MM:SS` or `HH:MM:00` without end bound | 86 400 / 1 440 results per day | Always set a `with_end` boundary |
| All-`At` time spec as standalone iterator | At most one tick, on the current day (plus any `new_with_start` start) | Use as datetime time component or `end_spec` for clearest intent |
