# Time Schedule Specification

## Format

```text
HH:MM:SS
```

All three components are required. Each component is one of: a wildcard (`ForEach`), a keep-current marker (`AsIs`), a fixed value (`At`), or an interval (`Every`).

---

## Component Reference

### Hours

| Syntax | Type | Meaning |
|--------|------|---------|
| `HH` | ForEach | Advance each occurrence (see semantics below) |
| `_` | AsIs | Keep current hour unchanged |
| `nH` | Every | Advance by *n* hours each tick (e.g. `1H`, `2H`, `4H`, `12H`) |
| `hh` (2-digit) | At | Pin to an exact hour (e.g. `09`, `13`, `17`) |

Valid ranges: `00`–`23` for At; `1H`–`23H` for Every.

### Minutes

| Syntax | Type | Meaning |
|--------|------|---------|
| `MM` | ForEach | Advance each occurrence (see semantics below) |
| `_` | AsIs | Keep current minute unchanged |
| `nM` | Every | Advance by *n* minutes each tick (e.g. `5M`, `15M`, `30M`) |
| `mm` (2-digit) | At | Pin to an exact minute (e.g. `00`, `15`, `30`, `45`) |

Valid ranges: `00`–`59` for At; `1M`–`59M` for Every.

### Seconds

| Syntax | Type | Meaning |
|--------|------|---------|
| `SS` | ForEach | Advance each occurrence (see semantics below) |
| `_` | AsIs | Keep current second unchanged |
| `nS` | Every | Advance by *n* seconds each tick (e.g. `15S`, `30S`) |
| `ss` (2-digit) | At | Pin to an exact second (e.g. `00`, `15`, `30`) |

Valid ranges: `00`–`59` for At; `1S`–`59S` for Every.

---

## Semantics

A time spec describes a **regular grid of times**, and each result is the first point of the grid
after the previous one (or after the `new_after` cursor):

- **Step** — the sum of the `Every` fields (`1H:30M:00` = every 90 minutes), or 1 unit of the
  finest `ForEach` field when there is no `Every` (`HH:MM:00` = every minute). The unit of that
  field is the *driver*.
- **Phase** — the grid passes through the start (or `new_after` cursor) with the fixed fields
  finer than the driver applied: `1H:30:00` from 09:10 runs at 09:30, 10:30, …
- **Window** — fixed fields **coarser** than the driver limit the grid to a window:
  `09:30M:00` is every 30 minutes during hour 9 only (09:00, 09:30, then 09:00 the next day).

Per field:

- **`Every(n)`** contributes *n* units to the step.
- **`At(v)`** fixes the field to *v*: finer than the driver it sets the grid's phase, coarser it
  opens a window.
- **`AsIs`** (`_`) keeps the start's value when finer than the driver; coarser, it is free like
  `ForEach`. Useful primarily in combined datetime specs (e.g. `YY-1M-31T_:_:_`).
- **`ForEach`** (`HH`/`MM`/`SS`) behaves differently depending on whether any `Every` component is present:

### ForEach when an `Every` component is present

`ForEach` means **carry** — the field is not independently advanced. The `Every` component drives the tick; coarser `ForEach` fields roll over naturally as the step accumulates, and finer ones keep the start's value.

Examples: `HH:30M:00` (minutes drive, hours carry), `1H:MM:SS` (hours drive, minutes and seconds carry).

### ForEach when no `Every` component is present

`ForEach` acts as **`Every(1)`** for its own unit, using the finest (rightmost) `ForEach` component as the driver. Coarser `ForEach` components still carry.

- **`HH:MM:SS`** — all ForEach → seconds drive (finest): **every second**.
- **`HH:MM:00`** — `At(0)` for seconds, ForEach for minutes and hours → minutes drive (next finest): **every minute at second :00**.
- **`HH:00:00`** — `At(0)` for both seconds and minutes, ForEach for hours → hours drive: **every hour at :00:00**.

The rule: find the rightmost (finest) `ForEach` component; it advances by 1 of its unit. All coarser `ForEach` components carry. `At` components always pin.

To check how a spec is read, call [`Spec::describe`](crate::time::Spec::describe):

```text
HH:MM:00   →  every minute, at second 00
1H:MM:00   →  every hour, at the start's minute and second 00
HH:30M:00  →  every 30 minutes, at second 00, counted from the start
09:30M:00  →  every 30 minutes, at second 00, counted from the start, only during the 09:00 hour
```

---

## AsIs vs ForEach

| Syntax | Variant | When an `Every` exists | When no `Every` exists |
|--------|---------|------------------------|------------------------|
| `HH`/`MM`/`SS` | `ForEach` | Carry | Drive (finest advances by 1) |
| `_` | `AsIs` | No-op (keep current) | No-op (keep current) |

**Key distinction:** `ForEach` participates in the "finest driver" election; `AsIs` never does. A spec using only `AsIs` and `At` components has no step: it names **one time of day**. In a combined datetime spec that time is used on every date; as a pure time iterator it is yielded at most once (see [All-`At` specs](#hhmmss-all-at-used-as-an-iterator)).

---

## Examples

All examples use `new_with_start` semantics: the start datetime is always returned as the first result.

---

### 1. `1H:00:00` — Every hour, on the hour

**Start:** 10:00:00

Hours advance by 1; minutes and seconds are pinned to 00.

```text
 1.  10:00:00
 2.  11:00:00
 3.  12:00:00
 4.  13:00:00
 5.  14:00:00
 6.  15:00:00
 7.  16:00:00
 8.  17:00:00
 9.  18:00:00
10.  19:00:00
```

---

### 2. `2H:00:00` — Every 2 hours, on the hour

**Start:** 08:00:00

```text
 1.  08:00:00
 2.  10:00:00
 3.  12:00:00
 4.  14:00:00
 5.  16:00:00
 6.  18:00:00
 7.  20:00:00
 8.  22:00:00
 9.  00:00:00  ← day rolls over
10.  02:00:00
```

---

### 3. `4H:00:00` — Every 4 hours, on the hour

**Start:** 2025-01-01 00:00:00

Six ticks per day; day boundary crossed transparently.

```text
 1.  2025-01-01 00:00:00
 2.  2025-01-01 04:00:00
 3.  2025-01-01 08:00:00
 4.  2025-01-01 12:00:00
 5.  2025-01-01 16:00:00
 6.  2025-01-01 20:00:00
 7.  2025-01-02 00:00:00
 8.  2025-01-02 04:00:00
 9.  2025-01-02 08:00:00
10.  2025-01-02 12:00:00
```

---

### 4. `6H:00:00` — Every 6 hours

**Start:** 2025-01-01 06:00:00

Four ticks per day.

```text
 1.  2025-01-01 06:00:00
 2.  2025-01-01 12:00:00
 3.  2025-01-01 18:00:00
 4.  2025-01-02 00:00:00
 5.  2025-01-02 06:00:00
 6.  2025-01-02 12:00:00
 7.  2025-01-02 18:00:00
 8.  2025-01-03 00:00:00
```

---

### 5. `HH:30M:00` — Every 30 minutes

**Start:** 09:00:00

`Every(30)` drives; `HH` (ForEach) carries; `00` pins seconds.

```text
 1.  09:00:00
 2.  09:30:00
 3.  10:00:00
 4.  10:30:00
 5.  11:00:00
 6.  11:30:00
 7.  12:00:00
 8.  12:30:00
 9.  13:00:00
10.  13:30:00
```

---

### 6. `HH:15M:00` — Every 15 minutes

**Start:** 09:00:00

```text
 1.  09:00:00
 2.  09:15:00
 3.  09:30:00
 4.  09:45:00
 5.  10:00:00
 6.  10:15:00
 7.  10:30:00
 8.  10:45:00
 9.  11:00:00
10.  11:15:00
```

---

### 7. `HH:10M:00` — Every 10 minutes

**Start:** 09:00:00

```text
 1.  09:00:00
 2.  09:10:00
 3.  09:20:00
 4.  09:30:00
 5.  09:40:00
 6.  09:50:00
 7.  10:00:00
 8.  10:10:00
 9.  10:20:00
10.  10:30:00
```

---

### 8. `HH:5M:00` — Every 5 minutes

**Start:** 09:00:00

```text
 1.  09:00:00
 2.  09:05:00
 3.  09:10:00
 4.  09:15:00
 5.  09:20:00
 6.  09:25:00
 7.  09:30:00
 8.  09:35:00
 9.  09:40:00
10.  09:45:00
```

---

### 9. `HH:MM:30S` — Every 30 seconds

**Start:** 09:00:00

`Every(30)` drives; `HH` and `MM` (ForEach) carry.

```text
 1.  09:00:00
 2.  09:00:30
 3.  09:01:00
 4.  09:01:30
 5.  09:02:00
 6.  09:02:30
 7.  09:03:00
 8.  09:03:30
 9.  09:04:00
10.  09:04:30
```

---

### 10. `HH:MM:15S` — Every 15 seconds

**Start:** 09:00:00

```text
 1.  09:00:00
 2.  09:00:15
 3.  09:00:30
 4.  09:00:45
 5.  09:01:00
 6.  09:01:15
 7.  09:01:30
 8.  09:01:45
 9.  09:02:00
10.  09:02:15
```

---

### 11. `HH:MM:SS` — Every second (all ForEach)

**Start:** 09:00:00

No `Every` component; `SS` (ForEach) is the finest component → drives by 1 second. `MM` and `HH` carry.

```text
 1.  09:00:00
 2.  09:00:01
 3.  09:00:02
 4.  09:00:03
 5.  09:00:04
 6.  09:00:05
 7.  09:00:06
 8.  09:00:07
 9.  09:00:08
10.  09:00:09
```

Seconds overflow into minutes naturally: …09:00:59, 09:01:00, 09:01:01, …

---

### 12. `HH:MM:00` — Every minute at second :00

**Start:** 09:00:00

No `Every` component; `SS` is `At(0)` (not a driver); `MM` (ForEach) is the finest wildcard → drives by 1 minute. `HH` carries.

```text
 1.  09:00:00
 2.  09:01:00
 3.  09:02:00
 4.  09:03:00
 5.  09:04:00
 6.  09:05:00
 7.  09:06:00
 8.  09:07:00
 9.  09:08:00
10.  09:09:00
```

Minutes overflow into hours: …09:59:00, 10:00:00, 10:01:00, …

---

### 13. `HH:00:00` — Every hour at :00:00

**Start:** 09:00:00

No `Every` component; both `SS` and `MM` are `At(0)`; `HH` (ForEach) is the only wildcard → drives by 1 hour. Equivalent to `1H:00:00`.

```text
 1.  09:00:00
 2.  10:00:00
 3.  11:00:00
 4.  12:00:00
 5.  13:00:00
 6.  14:00:00
 7.  15:00:00
 8.  16:00:00
```

---

### 14. `1H:30:00` — Every hour at :30 past

**Start:** 09:30:00

Hours advance by 1; minutes are pinned to 30; seconds pinned to 00.

```text
1.  09:30:00
2.  10:30:00
3.  11:30:00
4.  12:30:00
5.  13:30:00
6.  14:30:00
7.  15:30:00
8.  16:30:00
```

---

### 15. `2H:15:00` — Every 2 hours at :15 past

**Start:** 08:15:00

```text
 1.  08:15:00
 2.  10:15:00
 3.  12:15:00
 4.  14:15:00
 5.  16:15:00
 6.  18:15:00
 7.  20:15:00
 8.  22:15:00
 9.  00:15:00  ← next day
10.  02:15:00
```

---

### 16. `3H:45:00` — Every 3 hours at :45 past

**Start:** 09:45:00

```text
1.  09:45:00
2.  12:45:00
3.  15:45:00
4.  18:45:00
5.  21:45:00
6.  00:45:00  ← next day
7.  03:45:00
8.  06:45:00
```

---

### 17. `1H:MM:00` — Every hour, same minute, seconds=0

**Start:** 09:15:00

`Every(1)` drives hours; `MM` (ForEach) carries the start minute unchanged; seconds pinned to 00.

```text
1.  09:15:00
2.  10:15:00
3.  11:15:00
4.  12:15:00
5.  13:15:00
6.  14:15:00
7.  15:15:00
8.  16:15:00
```

---

### 18. `1H:MM:SS` — Every hour, preserving minute and second

**Start:** 09:22:45

`Every(1)` drives hours; both `MM` and `SS` (ForEach) carry. Pure 1-hour rolling advance from any start time.

```text
1.  09:22:45
2.  10:22:45
3.  11:22:45
4.  12:22:45
5.  13:22:45
6.  14:22:45
7.  15:22:45
8.  16:22:45
```

---

### 19. `HH:30M:15` — Every 30 minutes at second :15

**Start:** 09:00:15

`Every(30)` drives minutes; `HH` (ForEach) carries; `15` pins seconds.

```text
1.  09:00:15
2.  09:30:15
3.  10:00:15
4.  10:30:15
5.  11:00:15
6.  11:30:15
7.  12:00:15
8.  12:30:15
```

---

### 20. `1H:00:00` bounded by end spec `13:00:00`

**Start:** 09:00:00  **End spec:** `13:00:00`

The end is the first time the end spec produces after the start → **13:00:00**. The end is
inclusive, so 13:00 is yielded.

```text
1.  09:00:00
2.  10:00:00
3.  11:00:00
4.  12:00:00
5.  13:00:00
   (iterator ends)
```

---

### 21. `HH:30M:00` bounded by end spec `3H:30M:SS`

**Start:** 09:00:00  **End spec:** `3H:30M:SS`

End computed: the first `3H:30M:SS` time after 09:00:00 (a 3h30m step, seconds carried) → **12:30:00**.

```text
1.  09:00:00
2.  09:30:00
3.  10:00:00
4.  10:30:00
5.  11:00:00
6.  11:30:00
7.  12:00:00
8.  12:30:00
   (iterator ends)
```

---

### 22. `_:_:_` — AsIs in combined datetime spec

**Context:** `YY-1M-31T_:_:_` (last day of each month, keep current time)

`AsIs` (`_`) on all components means time is preserved as-is across date advances. All time components carry; only the date portion of the combined spec changes.

This is the primary use case for `AsIs` in time: fixing the time component to "whatever it currently is" while a date spec advances the date.

---

### 23. `HH:MM:_` — Every minute, preserving the original second

**Start:** 09:00:45

No `Every` component; `SS` is `AsIs` (never a driver); `MM` (ForEach) is the finest driver → advances by 1 minute. The second field **carries** its start value (45) on every tick.

Contrast with `HH:MM:00`: that spec pins seconds to `00` on every tick; this one keeps whatever second the start had.

```text
1.  09:00:45
2.  09:01:45
3.  09:02:45
4.  09:03:45
5.  09:04:45
6.  09:05:45
```

---

### 24. `HH:_:00` — Every hour, preserving the original minute

**Start:** 09:22:00

No `Every` component; `SS` is `At(0)` and `MM` is `AsIs` (neither is a driver); `HH` (ForEach) is the only driver → advances by 1 hour. The minute field **carries** its start value (22) on every tick.

Contrast with `HH:00:00`: that spec pins minutes to `00`; this one preserves the original minute.

```text
1.  09:22:00
2.  10:22:00
3.  11:22:00
4.  12:22:00
5.  13:22:00
6.  14:22:00
```

---

### 25. `_:30M:00` — Every 30 minutes, hours carry (AsIs)

**Start:** 09:00:00

`Every(30)` drives; hours `AsIs` carries (identical observable behaviour to `HH:30M:00` — both variants carry when an `Every` is present).

```text
1.  09:00:00
2.  09:30:00
3.  10:00:00
4.  10:30:00
5.  11:00:00
6.  11:30:00
```

---

### 26. `1H:_:00` — Every hour, minutes carry (AsIs)

**Start:** 09:15:00

`Every(1)` drives hours; minutes `AsIs` carries (identical observable behaviour to `1H:MM:00`).

```text
1.  09:15:00
2.  10:15:00
3.  11:15:00
4.  12:15:00
5.  13:15:00
6.  14:15:00
```

---

## end_spec Usage

An end spec is a time spec string evaluated **once from the start datetime**: its first
occurrence after the start becomes the end boundary. Any valid time spec may be used:

| end_spec | Meaning when applied from start S |
|----------|-----------------------------------|
| `13:00:00` | The next 13:00:00 after S — today, or tomorrow if S is later than 13:00 |
| `1H:00:00` | End 1 hour after start, on the hour |
| `3H:30M:SS` | End 3 hours and 30 minutes after start |
| `8H:MM:SS` | End exactly 8 hours after start |

The end is **inclusive**: results up to and including the end are yielded, and the iterator stops
at the first result after it. Nothing is invented at the boundary — `1H:00:00` from 09:00 with an
end of 11:30 yields 09:00, 10:00, 11:00.

---

## Behaviour Notes

### Start-time inclusivity

When constructed with `new_with_start`, the start datetime is always returned as the **first result** regardless of whether it aligns with the spec pattern. Use `new_after` to begin strictly after a given point in time.

### `new_after` with non-aligned starts

With `new_after`, the first result is the next grid point **strictly after** the given datetime,
so a non-aligned cursor snaps to the grid:

- `1H:00:00` from 09:30:00 → **10:00:00**, then 11:00:00, 12:00:00 …
- `HH:30:00` from 10:00:00 → **10:30:00** (the current hour's :30 is not skipped)
- `HH:MM:30` from 10:00:00 → **10:00:30**

### Day boundary crossing

The spec is applied to the wall-clock (local) time using `chrono::Duration` arithmetic. Day boundaries are crossed transparently. There is no built-in "end of day" concept; the spec produces results indefinitely until an end boundary is set or the caller stops consuming results. For what happens across DST changes, see [Daylight Saving Time](#daylight-saving-time).

### Resuming a stored schedule

To continue from an occurrence stored in a database, use
`SpecIteratorBuilder::resume_after(spec, tz, last_actual)`, where `last_actual` is the
stored `time` parsed back from RFC 3339. Store it with its UTC offset and rebuild with the
same options; add `.with_end(end)` if the schedule has a stored end. The crate docs' section *Persisting and resuming a schedule* has the full
pattern and the rules that keep resumed results identical to a continuous iterator.

### Sub-second precision

Specs have whole-second resolution. Results drop any sub-second part of the start or cursor, so
`HH:MM:10S` from `Utc::now()` runs on whole seconds. A `new_with_start` start is still yielded
exactly as given.

---

## Daylight Saving Time

The timezone-aware `SpecIterator` applies the spec to the wall clock and then maps each result
to a real instant. Two settings control what happens across a DST change:

- **`IntervalMode`** (`.with_interval_mode(…)`) — whether steps (`nH`, `nM`, `nS`, or a driving
  `HH`/`MM`/`SS`) move the **wall clock** (`WallClock`, the default, like cron) or measure
  **real elapsed time** (`Elapsed`).
- **`DstPolicy`** (`.with_dst_policy(…)`) — what `WallClock` does with a time that does not exist
  (spring-forward gap) or exists twice (fall-back overlap). `Adjust` (default) moves a gap time
  later by the length of the gap and takes the earlier of two overlapping times; `Strict` returns
  `Error::AmbiguousLocalTime` instead.

Results always **strictly increase**: when two wall-clock times map to the same instant (or a
later one maps to an earlier instant), the later one is dropped.

### `1H:00:00` in America/New_York from 00:00

**Spring-forward, 2025-03-09** (02:00 → 03:00):

```text
WallClock              Elapsed
00:00 EST              00:00 EST
01:00 EST              01:00 EST
03:00 EDT  ← 02:00     03:00 EDT
04:00 EDT              04:00 EDT
```

With `WallClock`, 02:00 does not exist and moves to 03:00; the real 03:00 then lands on the
same instant and is dropped, so there is no duplicate.

**Fall-back, 2025-11-02** (02:00 → 01:00):

```text
WallClock              Elapsed
00:00 EDT              00:00 EDT
01:00 EDT              01:00 EDT
02:00 EST              01:00 EST  ← repeated hour
03:00 EST              02:00 EST
```

`WallClock` visits the repeated 01:00 once; `Elapsed` runs every real hour.

### Longer steps drift in `Elapsed`

`6H:00:00` from 06:00 on 2025-03-08 (New York):

```text
WallClock                  Elapsed
2025-03-08 06:00 EST       2025-03-08 06:00 EST
2025-03-08 12:00 EST       2025-03-08 12:00 EST
2025-03-08 18:00 EST       2025-03-08 18:00 EST
2025-03-09 00:00 EST       2025-03-09 00:00 EST
2025-03-09 06:00 EDT       2025-03-09 07:00 EDT  ← stays an hour off the clock
```

`WallClock` keeps the schedule on the clock (the 00:00 → 06:00 step is 5 real hours); `Elapsed`
keeps every step at 6 real hours.

### Sub-hourly steps across a gap

`HH:30M:00` from 01:00 on 2025-03-09 (New York, `WallClock`):

```text
01:00 EST
01:30 EST
03:00 EDT  ← 02:00
03:30 EDT  ← 02:30
04:00 EDT     (the real 03:00 and 03:30 are dropped as duplicates)
```

### Notes

- In `Elapsed` mode stepped times never fall in a gap, so `DstPolicy` does not apply to them.
  Fixed fields (`09`, `30`) are read in the UTC offset of the previous occurrence.
- `IntervalMode` is available on the time iterator only. Combined datetime specs always use
  wall-clock steps.
- The time iterator yields plain `DateTime<Tz>` values, so it cannot mark a time as shifted. The
  date and datetime iterators report shifted times as `AdjustedLater`; see the datetime spec.

---

## Semantically Illogical Combinations

The following combinations either fail at parse time or terminate after a bounded number of
results. They are documented here for clarity.

---

### `hh:mm:ss` (all At) used as an iterator

**Examples:** `09:30:00`, `13:00:00`

**Behaviour:** A spec with no step names one time of day. As a time iterator it yields that time
**at most once, on the current day**, then terminates.

- `new_with_start` from `09:30:00` → yields `09:30:00` (passthrough) then terminates.
- `new_after` from `09:30:00` → yields nothing (09:30 today is not after the cursor).
- `new_after` from `09:00:00` → yields `09:30:00` once, then terminates.

**Preferred use:** All-`At` specs are most useful as **end specs** (passed to `with_end_spec`),
where they resolve to the next such time after the start — tomorrow, if today's has passed — or
as the time part of a combined datetime spec, where they apply to every date.

---

### `_:_:_` (all AsIs) used as a pure time iterator

**Behaviour:** `AsIs` keeps every field at the start's value, so the spec names the start's own
time of day — which is never after it. The iterator terminates immediately after any passthrough.

- `new_with_start` from any time → yields start once then terminates.
- `new_after` from any time → yields nothing.

**Preferred use:** `AsIs` components are meaningful only within **combined datetime specs**
(e.g. `YY-1M-31T_:_:_`) where the date spec advances and the time portion is preserved.

---

### `Every(0)` — zero-size interval

**Examples:** `0H:00:00`, `HH:0M:00`, `HH:MM:0S`

**Result:** **Parse error.** `Every(0)` is rejected at parse time by `verify(parse_u8, |&n| n > 0)`
in the nom parser.  These specs cannot be constructed.

**Use:** `Every(n)` with n ≥ 1 (enforced).

---

### Out-of-range fixed values

**Examples:** `24:00:00`, `HH:60:00`, `HH:MM:60`

**Result:** **Parse error** (`Error::InvalidTimeSpec`). Fixed hours must be 0–23, fixed minutes
and seconds 0–59. Steps are not limited this way: `25H:00:00` and `HH:90M:00` are valid.

---

### Summary table

| Combination | Behaviour | Notes |
|-------------|-----------|-------|
| `hh:mm:ss` (as iterator) | At most one result, on the current day (plus any `new_with_start` start) | Preferred as `end_spec` or datetime time part |
| `_:_:_` (as pure time iterator) | Only the `new_with_start` start; nothing with `new_after` | Use in combined datetime spec only |
| `0H:00:00` / `HH:0M:00` / `HH:MM:0S` | **Parse error** — rejected at construction | Use n ≥ 1 (enforced by parser) |
| `24:00:00` / `HH:60:00` / `HH:MM:60` | **Parse error** — fixed value out of range | Hours 0–23, minutes and seconds 0–59 |
