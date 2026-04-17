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

### 2. Time spec provides intra-day ticks

For each valid calendar date, the time spec is applied repeatedly within the half-open window
`[date_midnight, next_midnight)`.  Each call to `apply_time_spec(cursor)` produces the next
datetime in the time sequence.  When the result falls on or after `next_midnight` the day window
is exhausted and the date iterator advances to the next valid date.

### 3. `current_date_end` window

An internal `current_date_end` value (exclusive `next_midnight`) prevents the date iterator from
re-advancing while time ticks remain within the current day.  This is how "every Monday, every
30 minutes" emits 48 ticks on each Monday before moving to the following Monday.

### 4. First tick on a new (non-initial) date

For each new date entered via the date iterator, the first tick is found by:

1. Computing `spec_delta` — the natural step size of the time spec's driving component
   (e.g. 1 h for `1H:00:00`, 30 min for `HH:30M:00`, 1 s for `HH:MM:SS`).
2. Stepping back one `spec_delta` from `date_midnight` and applying the time spec once:
   `first_tick = apply_time_spec(date_midnight − spec_delta)`.
3. If the result is `≥ date_midnight`, use it; otherwise fall back to
   `apply_time_spec(date_midnight)`.

This guarantees midnight itself is the first tick for `Every`- and `ForEach`-driven specs
(e.g. `1H:00:00` → back 1 h → 23:00 → +1 h → **00:00**; `HH:30M:00` → back 30 min →
23:30 → +30 min → **00:00**).  `At`-only specs (e.g. `11:00:00`) land in the past after
stepping back, so the fallback puts them at their correct fixed time (11:00 same day).

### 5. Initial-day asymmetry

The very first date is treated specially:

- **`new_with_start`**: the `start` datetime is returned as the first result unconditionally
  (passthrough), and the time cursor begins at `start` for subsequent ticks on that date.
  Only ticks *strictly after* `start` are emitted for the rest of that day.
- **`new_after`**: no passthrough.  The first tick must be strictly after `dtm`.  If the
  time spec's first tick on the current date has already passed, the entire initial date is
  skipped and the first result is from the *next* valid date.

All dates after the initial emit ticks from midnight (`00:00:00`) onward, so the initial date
typically has fewer ticks than subsequent dates when started mid-day.

### 6. `AdjustedLater` / `AdjustedEarlier` propagation

When the date spec applies a business-day adjustment (e.g. `~NB`), the adjustment metadata is
carried forward to the **first tick only** on the adjusted (observed) date.  All subsequent
intra-day ticks on the same date are emitted as `Single`.

```text
May 31 (Sat) ~NB → Jun 2 (Mon), time spec HH:30M:00:

 first tick  → AdjustedLater(actual=2025-05-31 00:00, observed=2025-06-02 00:00)
 second tick → Single(2025-06-02 00:30)
 third tick  → Single(2025-06-02 01:00)
 ...
```

The `actual` date in the result is always the *calendar* date before adjustment; its time
component is set to the same value as the observed tick's time (not a "natural" time for the
pre-adjustment date).

---

## Result Types

Same three variants as the date iterator, now carrying `NaiveDateTime` or `DateTime<Tz>`:

| Variant | Meaning |
|---------|---------|
| `Single(dt)` | No adjustment; `dt` is exactly as computed. |
| `AdjustedLater(actual, observed)` | `actual` is the natural date + time; `observed` is the adjusted (later) date + same time. Only on the first tick of an adjusted date. |
| `AdjustedEarlier(actual, observed)` | As above but the adjustment moved the date earlier. |

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
 1. Single        2025-01-31 11:00:00  (Fri)
 2. Single        2025-02-28 11:00:00  (Fri)
 3. Single        2025-03-31 11:00:00  (Mon)
 4. Single        2025-04-30 11:00:00  (Wed)
 5. AdjustedLater actual=2025-05-31 11:00:00  observed=2025-06-02 11:00:00  (Sat→Mon)
 6. Single        2025-06-30 11:00:00  (Mon)
 7. Single        2025-07-31 11:00:00  (Thu)
 8. AdjustedLater actual=2025-08-31 11:00:00  observed=2025-09-01 11:00:00  (Sun→Mon)
 9. Single        2025-09-30 11:00:00  (Tue)
10. Single        2025-10-31 11:00:00  (Fri)
11. AdjustedLater actual=2025-11-30 11:00:00  observed=2025-12-01 11:00:00  (Sun→Mon)
12. Single        2025-12-31 11:00:00  (Wed)
13. AdjustedLater actual=2026-01-31 11:00:00  observed=2026-02-02 11:00:00  (Sat→Mon)
14. AdjustedLater actual=2026-02-28 11:00:00  observed=2026-03-02 11:00:00  (Sat→Mon)
15. Single        2026-03-31 11:00:00  (Tue)
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

`spec_delta` = 6 h.  The initial date starts from 06:00 (passthrough) then advances by 6 h.
From day 2 onwards all four of the 6-hourly boundaries — including midnight — are emitted.

```text
 1. 2025-01-01 06:00:00  ← start (passthrough)
 2. 2025-01-01 12:00:00
 3. 2025-01-01 18:00:00
 4. 2025-01-02 00:00:00  ← first tick of new day = midnight (6 h delta fix)
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

`spec_delta` = 4 h.  Initial month starts at 08:00; all subsequent months start at midnight.

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

`spec_delta` = 30 min.  Within each valid date, up to 48 ticks are emitted (00:00–23:30).
The initial date (Jan 31) starts at 08:00, so it emits 32 ticks (08:00–23:30).

Business-day adjustment: **only the first tick** of an adjusted date is `AdjustedLater`;
all remaining same-day ticks are `Single`.

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
    (Feb 28: 00:00, 00:30, 01:00, ... 48 ticks, all Single)
    (May 31 Sat → Jun 2 Mon: AdjustedLater on 00:00, then 47 Single ticks)
```

---

### 10. `YY-MM-1BDT4H:00:00` — Every business day, every 4 hours

**Start:** 2025-01-01 09:00:00 (Wednesday)

`1BD` visits every business day (Mon–Fri).  Weekends are skipped.

```text
 1. 2025-01-01 09:00:00  ← Wed, passthrough
 2. 2025-01-01 13:00:00
 3. 2025-01-01 17:00:00
 4. 2025-01-01 21:00:00
 5. 2025-01-02 00:00:00  ← Thu, midnight included
 6. 2025-01-02 04:00:00
 7. 2025-01-02 08:00:00
 8. 2025-01-02 12:00:00
 9. 2025-01-02 16:00:00
10. 2025-01-02 20:00:00
11. 2025-01-03 00:00:00  ← Fri
12. 2025-01-03 04:00:00
13. 2025-01-03 08:00:00
14. 2025-01-03 12:00:00
15. 2025-01-03 16:00:00
    (2025-01-03 20:00)
    (Sat Jan 4, Sun Jan 5: skipped)
    (Mon Jan 6: 00:00, 04:00, 08:00, 12:00, 16:00, 20:00)
```

---

### Group 4 — Overflow and business-day adjustment with time

---

### 11. `YY-1M-31NT11:00:00` — 31st-or-next-month at 11:00

**Start:** 2025-01-31 11:00:00

`31N` overflows to 1st of next month when the month has fewer than 31 days.
One tick per date at 11:00; result type carries the overflow metadata.

```text
 1. Single        2025-01-31 11:00:00
 2. AdjustedLater actual=2025-02-28 11:00:00  observed=2025-03-01 11:00:00
 3. Single        2025-03-31 11:00:00
 4. AdjustedLater actual=2025-04-30 11:00:00  observed=2025-05-01 11:00:00
 5. Single        2025-05-31 11:00:00
 6. AdjustedLater actual=2025-06-30 11:00:00  observed=2025-07-01 11:00:00
 7. Single        2025-07-31 11:00:00
 8. Single        2025-08-31 11:00:00
 9. AdjustedLater actual=2025-09-30 11:00:00  observed=2025-10-01 11:00:00
10. Single        2025-10-31 11:00:00
11. AdjustedLater actual=2025-11-30 11:00:00  observed=2025-12-01 11:00:00
12. Single        2025-12-31 11:00:00
13. Single        2026-01-31 11:00:00
14. AdjustedLater actual=2026-02-28 11:00:00  observed=2026-03-01 11:00:00
15. Single        2026-03-31 11:00:00
```

---

### 12. `YY-1M-31L~NBTH:30M:00` (sub-daily on adjusted date)

**Start:** 2025-01-31 08:00:00

When May 31 adjusts to Jun 2, `AdjustedLater` is set only on the **first** tick of Jun 2.
All 47 remaining 30-minute ticks on Jun 2 are `Single`.

Illustrative slice around the May/June boundary (Jan–Apr and Jun–Dec are analogous to
example 9):

```text
  (... Jan 31, Feb 28, Mar 31, Apr 30 as in example 9 ...)

  May 31 (Sat) → observed Jun 2 (Mon):
  AdjustedLater actual=2025-05-31 00:00:00  observed=2025-06-02 00:00:00
  Single        2025-06-02 00:30:00
  Single        2025-06-02 01:00:00
  ...
  Single        2025-06-02 23:30:00  (47 Single ticks follow the first AdjustedLater)

  Jun 30 (Mon) → Single, no adjustment:
  Single        2025-06-30 00:00:00
  ...
```

---

### Group 5 — `new_after` semantics

---

### 13. `YY-1M-15T11:00:00` via `new_after` — skipping a past time on the initial date

**`new_after` from:** 2025-01-15 12:00:00

The 11:00 slot for Jan 15 has already passed at 12:00.  `is_valid` fails for the initial date
(`first_time = 11:00 ≤ initial_dtm = 12:00`), so Jan 15 is skipped entirely.

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

`At(0)` transforms run before `Every(1H)`:
`09:30 → sec→0 → 09:30 → min→0 → 09:00 → hours+1 → 10:00`.
The first result is 10:00, not 09:30.  Subsequent results are 11:00, 12:00 … (exact 1 h gaps).

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

`_:_:_` (all `AsIs`) is a true no-op on time.  On the initial date the passthrough preserves
14:30.  On every subsequent date `spec_delta` falls back to 1 s, the `date_midnight − 1s`
candidate lands in the previous day, the fallback applies `AsIs` to midnight → **00:00:00**.
The original time is **not** propagated forward.

```text
 1. 2025-01-31 14:30:00  ← passthrough (initial time preserved)
 2. 2025-02-28 00:00:00  ← ⚠ 14:30 lost; midnight carried forward
 3. 2025-03-31 00:00:00
 4. 2025-04-30 00:00:00
 5. 2025-05-31 00:00:00
 6. 2025-06-30 00:00:00
 7. 2025-07-31 00:00:00
 8. 2025-08-31 00:00:00
 9. 2025-09-30 00:00:00
10. 2025-10-31 00:00:00
```

**Intended use of `_:_:_`:** combine it with a date spec that advances one day at a time
(`YY-MM-DD`) if you genuinely want "the same wall-clock time as right now" — but note that
even then the carry only works because consecutive days share the same `spec_delta` mechanics.
**Safe alternative:** embed a literal `At` time (`T14:30:00`) to pin the time on every date.

---

### 16. `YY-1M-15T1H:MM:SS` — Hourly with minute/second carry; carry lost on date gaps

**Start:** 2025-01-15 09:22:45

`Every(1H)` drives; `MM` and `SS` (ForEach) carry.  On the initial date the carry is the
start's `:22:45`.  On subsequent months `spec_delta = 1 h`, so the synthetic cursor is
`date_midnight − 1h = 23:00:00` (minutes=0, seconds=0).  The carry context of `:22:45` is
**not** preserved across the date gap.

```text
 1. 2025-01-15 09:22:45  ← passthrough
 2. 2025-01-15 10:22:45  ← carry :22:45 preserved on initial date
 3. 2025-01-15 11:22:45
 4. 2025-01-15 12:22:45
 5. 2025-01-15 13:22:45
 6. 2025-01-15 14:22:45
 7. 2025-01-15 15:22:45
 8. 2025-01-15 16:22:45
 9. 2025-01-15 17:22:45
10. 2025-01-15 18:22:45
11. 2025-01-15 19:22:45
12. 2025-01-15 20:22:45
13. 2025-01-15 21:22:45
14. 2025-01-15 22:22:45
15. 2025-01-15 23:22:45
    (2025-02-15 00:00:00  ← ⚠ carry :22:45 lost; ticks at :00:00 on all new dates)
    (2025-02-15 01:00:00)
    (2025-02-15 02:00:00)
    ...
```

**Why:** The synthetic cursor `23:00:00` has `:00:00` for minutes and seconds.  `ForEach`
carry takes those values, giving `00:00:00` for the first tick on Feb 15 and `:00:00` for all
subsequent hours.

**Safe alternative:** Use `1H:30:00` (pin minutes to a fixed value) or `1H:00:00` if carry is
not required.

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
Jan 2:  00:00, 01:00, ..., 23:00         (24 ticks — full day from midnight)
Jan 3:  00:00, 01:00, ..., 23:00         (24 ticks)
```

Callers that assume a constant tick count per date must account for this asymmetry on the first
date.

---

### `_` (AsIs) does NOT preserve time across date gaps

**Spec:** any time spec mixing `_` with non-consecutive date specs

`AsIs` preserves the cursor's time field.  But the cursor for a new date is synthesised from
`date_midnight − spec_delta`, **not** from the previous occurrence.  For non-consecutive date
specs (monthly, weekly, etc.) there is no way to carry the previous tick's exact time to the
new date.

```text
YY-1M-15T_:30M:00  (AsIs hours, Every(30) minutes, At(0) seconds)
  start=2025-01-15 09:00:00

Jan 15:  09:00 (passthrough), 09:30, 10:00, ...  ← hours carry from start
Feb 15:  first cursor = Feb 15 00:00 − 30 min = Jan 31 23:30
         apply_time_spec(23:30) → sec→0=23:30, min+30=00:00 Feb 15
         hours AsIs = 23:??  ← wrong; the AsIs hour came from 23:30 cursor, not the 09:xx from Jan
```

The observable hour on Feb 15's first tick is 00 (from the cursor), not 09 (from January).
If the intent is "keep the hour constant", use an explicit `At` hour (`09:30M:00` or the
literal `09`).

---

### `AdjustedLater` appears only on the first intra-day tick

**Spec:** any date spec with business-day adjustment and a sub-daily time spec

Only the **first** tick of an adjusted date is wrapped in `AdjustedLater`.  All subsequent
ticks on the same observed date are returned as `Single`.

```text
YY-1M-31L~NBTHH:30M:00  (last biz day monthly, every 30 min)

May 31 (Sat) → Jun 2 (Mon):
 tick  1: AdjustedLater(actual=May 31 00:00, observed=Jun 2 00:00)
 tick  2: Single(Jun 2 00:30)   ← adjustment metadata dropped
 tick  3: Single(Jun 2 01:00)
 ...
 tick 48: Single(Jun 2 23:30)
```

Callers that inspect result types to detect business-day adjustments must check only the first
tick per date, or track the date change across consecutive results.

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

### `ForEach`/`Every` carry context is lost on date gaps

**Spec:** time spec with carry components (`1H:MM:SS`, `HH:30M:SS`, etc.) combined with
any date spec where valid dates are not consecutive

Carry-component values (minutes, seconds in `1H:MM:SS`) are derived from the synthetic cursor
`date_midnight − spec_delta`, which has fixed, predictable values (not the actual
minutes/seconds of the previous tick).  On a monthly date spec the carry resets each month.

```text
YY-1M-15T1H:MM:SS  start=2025-01-15 09:22:45

Jan 15:  09:22:45, 10:22:45, 11:22:45, ...  ← :22:45 preserved
Feb 15:  00:00:00, 01:00:00, 02:00:00, ...  ← ⚠ :22:45 lost; resets to :00:00
Mar 15:  00:00:00, 01:00:00, ...            ← same reset pattern
```

For a **consecutive-day** date spec (`YY-MM-DD`), there is no gap, so this reset does not
occur.  For any date spec that skips days, carry context is reset on each new date.

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

Within a datetime spec the time part is applied to the first tick of each date using
`apply_time_spec(date_midnight − delta)`.  For an all-`At` spec the delta is 1 s, and
`apply_time_spec(23:59:59)` produces `hh:mm:ss` of the previous day — which is `< midnight`,
so the fallback `apply_time_spec(midnight)` is used.  This correctly produces one tick per day
at the fixed time.

Using an all-`At` spec as the **time iterator** in isolation (no date spec) now terminates
safely via the **no-progress guard** in `NaiveSpecIterator`: because `At` transforms can move
a field backwards (e.g. pinning hours to 11 when the cursor is already at 12:00 moves time
backward), the computed `next` is ≤ the current cursor and `Ok(None)` is returned.

- `new_with_start` from `11:00:00` → yields `11:00:00` (passthrough) then terminates.
- `new_after` from `11:00:00` → yields nothing (guard fires on first tick).
- `new_after` from `10:00:00` → yields `11:00:00` once, then terminates.

All-`At` time specs remain most useful when:

1. Used as the time component of a datetime spec (where the date iterator drives progression).
2. Used as an `end_spec` boundary (evaluated exactly once).

---

### Summary table

| Combination | Surprising behaviour | Safe alternative |
|-------------|----------------------|-----------------|
| Sub-daily start mid-day (`new_with_start`) | Initial date has fewer ticks than later dates | Expected; document for callers |
| `_:_:_` or `_`-component with date gaps | Time resets to midnight / cursor values on each new date; initial time not preserved | Use `At` components (`hh:mm:ss`) for constant time |
| Adjusted date + sub-daily time spec | Only the first tick is `AdjustedLater`; rest are `Single` | Scan first tick per date for adjustment metadata |
| `AdjustedLater.actual.time()` | Same as observed time; not the "natural" time for the unadjusted date | Treat `actual.date()` as the logical date; `observed` as the real datetime |
| `new_after` with past daily time | Entire initial date silently skipped | Use `new_with_start` if start inclusivity is required |
| `1H:MM:SS` / `HH:nM:SS` with non-daily date spec | Carry `:mm:ss` resets on each new date to derived-from-cursor values | Use pinned `At` for minutes/seconds (`1H:30:00`) |
| `HH:MM:SS` or `HH:MM:00` without end bound | 86 400 / 1 440 results per day | Always set a `with_end` boundary |
| All-`At` time spec as standalone iterator | Terminates via no-progress guard (one tick with `new_with_start`, zero with `new_after` from spec time) | Use as datetime time component or `end_spec` for clearest intent |
