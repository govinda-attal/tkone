# Time Schedule Specification

## Format

```
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

Components are applied in order: **seconds first, then minutes, then hours**.

- **`Every(n)`** adds a `chrono::Duration` of *n* units to the running datetime.
- **`At(v)`** sets that field to value *v* (via `with_second` / `with_minute` / `with_hour`).
- **`AsIs`** (`_`) is a true no-op — the field is left at its current value. Useful primarily in combined datetime specs (e.g. `YY-1M-31T_:_:_`) where only the date advances.
- **`ForEach`** (`HH`/`MM`/`SS`) advances each occurrence; behaves differently depending on whether any `Every` component is present:

### ForEach when an `Every` component is present

`ForEach` means **carry** — the field is not independently advanced. The `Every` component drives the tick; `ForEach` fields accumulate any natural overflow from the duration arithmetic.

Examples: `HH:30M:00` (minutes drive, hours carry), `1H:MM:SS` (hours drive, minutes and seconds carry).

### ForEach when no `Every` component is present

`ForEach` acts as **`Every(1)`** for its own unit, using the finest (rightmost) `ForEach` component as the driver. Coarser `ForEach` components still carry.

- **`HH:MM:SS`** — all ForEach → seconds drive (finest): **every second**.
- **`HH:MM:00`** — `At(0)` for seconds, ForEach for minutes and hours → minutes drive (next finest): **every minute at second :00**.
- **`HH:00:00`** — `At(0)` for both seconds and minutes, ForEach for hours → hours drive: **every hour at :00:00**.

The rule: find the rightmost (finest) `ForEach` component; it advances by 1 of its unit. All coarser `ForEach` components carry. `At` components always pin.

---

## AsIs vs ForEach

| Syntax | Variant | When an `Every` exists | When no `Every` exists |
|--------|---------|------------------------|------------------------|
| `HH`/`MM`/`SS` | `ForEach` | Carry | Drive (finest advances by 1) |
| `_` | `AsIs` | No-op (keep current) | No-op (keep current) |

**Key distinction:** `ForEach` participates in the "finest driver" election; `AsIs` never does. A spec using only `AsIs` and `At` components with no `ForEach` or `Every` will not advance time — this is intentional for combined datetime specs but is an infinite loop in a pure time spec.

---

## Examples

All examples use `new_with_start` semantics: the start datetime is always returned as the first result.

---

### 1. `1H:00:00` — Every hour, on the hour

**Start:** 10:00:00

Hours advance by 1; minutes and seconds are pinned to 00.

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

The end is computed by applying the end spec once from the start via `new_after`:
`At(0)` seconds, `At(0)` minutes, `At(13)` hours → **13:00:00**.

```
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

End computed: from 09:00:00, apply `Every(3)H` + `Every(30)M` + `ForEach S` (carry) → **12:30:00**.

```
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

```
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

```
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

```
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

```
1.  09:15:00
2.  10:15:00
3.  11:15:00
4.  12:15:00
5.  13:15:00
6.  14:15:00
```

---

## end_spec Usage

An end spec is a time spec string evaluated **once from the start datetime** to produce the absolute end boundary. Any valid time spec may be used:

| end_spec | Meaning when applied from start S |
|----------|-----------------------------------|
| `13:00:00` | End at 13:00:00 same day (all At: pins all three fields) |
| `1H:00:00` | End 1 hour after start, on the hour |
| `3H:30M:SS` | End 3 hours and 30 minutes after start |
| `8H:MM:SS` | End exactly 8 hours after start |

The iterator terminates when the computed next value reaches or passes the end boundary. If the computed end is ≤ start (e.g. using `13:00:00` from a 14:00 start), the iterator produces zero results.

---

## Behaviour Notes

### Start-time inclusivity

When constructed with `new_with_start`, the start datetime is always returned as the **first result** regardless of whether it aligns with the spec pattern. Use `new_after` to begin strictly after a given point in time.

### `new_after` with non-aligned starts

With `new_after`, the first result is the next occurrence **strictly after** the given datetime. Because `At` transforms apply before `Every` on higher units, a non-aligned start may produce a shorter-than-expected first interval.

**Example:** `1H:00:00` via `new_after` from 09:30:00:
1. seconds → At(0): 09:30:00
2. minutes → At(0): **09:00:00** (moves minute backwards)
3. hours → Every(1): **10:00:00**

First result is 10:00:00, not 09:30:00. Subsequent results are 11:00:00, 12:00:00 … (exactly 1-hour gaps).

### Day boundary crossing

The iterator works on naive datetimes using `chrono::Duration` arithmetic. Day boundaries are crossed transparently. There is no built-in "end of day" concept; the spec produces results indefinitely until an end boundary is set or the caller stops consuming results.

### Component application order

On each tick: **seconds → minutes → hours**, in that fixed order. The seconds transform runs before the minutes transform, so a spec like `HH:30M:05` sets seconds to :05 first and then adds 30 minutes.

---

## Semantically Illogical Combinations

The following specs produce infinite loops or backwards movement. **Avoid them.**

---

### `hh:mm:ss` (all At) used as an iterator

**Examples:** `09:30:00`, `13:00:00`

**Problem:** Each call pins all three fields to the same fixed values. If the current time already equals or exceeds the target, the `At` transform for hours moves time backwards. The iterator then loops forever between the current time and the pinned time.

```
09:30:00 as iterator from 09:30:00 → 09:30, 09:30, 09:30 ... (infinite)
09:30:00 as iterator from 10:00:00 → 09:30 (backwards!), then loops at 09:30
```

**Safe use:** All-`At` specs are valid exclusively as **end specs** (passed to `with_end_spec`), where they are evaluated exactly once to produce an absolute boundary.

---

### `_:_:_` (all AsIs) used as a pure time iterator

**Problem:** `AsIs` never advances time. Every call returns the same datetime. Infinite loop.

**Safe use:** `AsIs` components are meaningful only within **combined datetime specs** (e.g. `YY-1M-31T_:_:_`) where the date spec advances and the time portion is preserved.

---

### Every(0) — zero-size interval

**Examples:** `0H:00:00`, `HH:0M:00`, `HH:MM:0S`

**Problem:** Adding a duration of 0 produces no forward movement. The iterator returns the same value on every call.

**Safe alternative:** Use `Every(n)` with n ≥ 1.

---

### Summary table

| Combination | Problem | Safe alternative |
|-------------|---------|-----------------|
| `hh:mm:ss` (as iterator) | Backwards on every tick | Use as `end_spec` only |
| `_:_:_` (as pure time iterator) | No advance, infinite loop | Use in combined datetime spec only |
| `0H:00:00` / `HH:0M:00` / `HH:MM:0S` | Zero advance, infinite loop | Use n ≥ 1 |
