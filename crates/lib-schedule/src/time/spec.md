# Time Schedule Specification

## Format

```
HH:MM:SS
```

All three components are required. Each component is one of: a fixed value (`At`), an interval (`Every`), or a wildcard (`AsIs`).

---

## Component Reference

### Hours

| Syntax | Type | Meaning |
|--------|------|---------|
| `HH` | AsIs | Keep the current hour unchanged |
| `nH` | Every | Advance by *n* hours each tick (e.g. `1H`, `2H`, `4H`, `12H`) |
| `hh` (2-digit) | At | Pin to an exact hour (e.g. `09`, `13`, `17`) |

Valid ranges: `00`–`23` for At; `1H`–`23H` for Every.

### Minutes

| Syntax | Type | Meaning |
|--------|------|---------|
| `MM` | AsIs | Keep the current minute unchanged |
| `nM` | Every | Advance by *n* minutes each tick (e.g. `5M`, `15M`, `30M`) |
| `mm` (2-digit) | At | Pin to an exact minute (e.g. `00`, `15`, `30`, `45`) |

Valid ranges: `00`–`59` for At; `1M`–`59M` for Every.

### Seconds

| Syntax | Type | Meaning |
|--------|------|---------|
| `SS` | AsIs | Keep the current second unchanged |
| `nS` | Every | Advance by *n* seconds each tick (e.g. `15S`, `30S`) |
| `ss` (2-digit) | At | Pin to an exact second (e.g. `00`, `15`, `30`) |

Valid ranges: `00`–`59` for At; `1S`–`59S` for Every.

---

## Semantics

Components are applied in order: **seconds first, then minutes, then hours**.

- **`Every(n)`** adds a `chrono::Duration` of *n* units to the running datetime.
- **`At(v)`** sets that field to value *v* (via `with_second` / `with_minute` / `with_hour`).
- **`AsIs`** leaves that field unchanged.

The critical design rule: **`At` on a component only produces forward-progressing results when paired with `Every` on a coarser component.** For example, `At(0)` for minutes makes sense alongside `Every(n)` for hours — the hourly advance carries time forward; the `At(0)` then pins the minute within each period. Without a coarser `Every` to drive forward movement, `At` will reset the field backwards and cause an infinite loop.

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

Hours are preserved (`AsIs`); minutes advance by 30; seconds pinned to 00.

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

Hours and minutes are preserved (`AsIs`); seconds advance by 30.

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

### 11. `1H:30:00` — Every hour at :30 past

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

### 12. `2H:15:00` — Every 2 hours at :15 past

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

### 13. `3H:45:00` — Every 3 hours at :45 past

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

### 14. `1H:MM:00` — Every hour, same minute, seconds=0

**Start:** 09:15:00

Hours advance by 1; minutes are preserved from the start time (`AsIs`); seconds pinned to 00.

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

### 15. `1H:MM:SS` — Every hour, preserving minute and second

**Start:** 09:22:45

Hours advance by 1; minutes and seconds are both preserved (`AsIs`). This is a pure 1-hour rolling advance from any starting time.

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

### 16. `HH:30M:15` — Every 30 minutes at second :15

**Start:** 09:00:15

Minutes advance by 30; seconds are pinned to 15; hours preserved.

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

### 17. `1H:00:00` bounded by end spec `13:00:00`

**Start:** 09:00:00  **End spec:** `13:00:00`

The end is computed by applying the end spec once from the start via `new_after`:
seconds=At(0), minutes=At(0), hours=At(13) → **13:00:00**.

```
1.  09:00:00
2.  10:00:00
3.  11:00:00
4.  12:00:00
5.  13:00:00
   (iterator ends)
```

---

### 18. `HH:30M:00` bounded by end spec `3H:30M:SS`

**Start:** 09:00:00  **End spec:** `3H:30M:SS`

End computed: from 09:00:00, apply seconds=AsIs, minutes+30, hours+3 → **12:30:00**.

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

## end_spec Usage

An end spec is a time spec string evaluated **once from the start datetime** to produce the absolute end boundary. Any valid time spec may be used:

| end_spec | Meaning when applied from start S |
|----------|-----------------------------------|
| `13:00:00` | End at 13:00:00 same day (At-At-At pins all three fields) |
| `1H:00:00` | End 1 hour after start, on the hour |
| `3H:30M:SS` | End 3 hours and 30 minutes after start |
| `8H:MM:SS` | End exactly 8 hours after start |

The iterator stops when the next computed value reaches or passes the end boundary. If the computed end is ≤ start (e.g. using `13:00:00` from a 14:00 start), the iterator produces zero results.

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

The iterator works on naive datetimes using `chrono::Duration` arithmetic. Day boundaries are crossed transparently. There is no built-in "end of day" concept; the spec produces results indefinitely until an end boundary is set or the caller stops consuming.

### Component application order

On each tick: **seconds → minutes → hours**, in that fixed order. The seconds transform (whether `At` or `Every`) runs before the minutes transform, so `HH:30M:05` sets seconds to :05 first and then adds 30 minutes. Starting at a second other than :05 therefore produces a slightly uneven first interval.

---

## Semantically Illogical Combinations

The following specs produce infinite loops, backwards movement, or zero progress. **Avoid them.**

---

### `HH:MM:SS` — All AsIs

**Problem:** No component ever advances time. Every call returns the same datetime, looping indefinitely.

**Safe alternative:** Ensure at least one component uses `Every(n)`.

---

### `hh:mm:ss` (all At) used as an iterator

**Examples:** `09:30:00`, `13:00:00`

**Problem:** Each call pins all three fields to the same fixed values. If the current time already equals or exceeds the target, the hours `At` transform moves time backwards. The iterator then oscillates between the current time and the pinned time forever.

```
09:30:00 as iterator from 09:30:00 → 09:30, 09:30, 09:30 ... (infinite)
09:30:00 as iterator from 10:00:00 → 09:30 (backwards!), then loops at 09:30
```

**Safe use:** All-`At` specs are valid exclusively as **end specs** (passed to `with_end_spec`), where they are evaluated exactly once to produce an absolute boundary.

---

### `HH:mm:ss` — AsIs hour, At minute (no Every on hours or minutes)

**Examples:** `HH:00:00`, `HH:30:00`, `HH:00:30`

**Problem:** Without `Every` on hours or minutes, the `At` minute transform resets the minute backwards from any non-matching position. The `AsIs` hour leaves that backward value unchanged. On the next tick the same backward-pin recurs → infinite loop.

```
HH:00:00 from 09:30:00 → 09:00:00 (backwards) → 09:00:00 → ...
HH:30:00 from 09:31:00 → 09:30:00 (backwards) → 09:30:00 → ...
```

**Safe alternatives:** `1H:00:00` (add `Every` to hours) or `HH:nM:00` (add `Every` to minutes).

---

### `HH:MM:ss` — AsIs hour and minute, At second (no Every)

**Examples:** `HH:MM:00`, `HH:MM:30`

**Problem:** Same root cause — pinning seconds without any `Every` on minutes or hours creates backwards movement and an infinite loop at non-aligned starts.

```
HH:MM:00 from 09:30:45 → 09:30:00 (backwards) → 09:30:00 → ...
```

**Safe alternatives:** `HH:nM:00` or `nH:MM:00`.

---

### Every(0) — zero-size interval

**Examples:** `0H:00:00`, `HH:0M:00`, `HH:MM:0S`

**Problem:** Adding a zero-duration produces no forward movement. The iterator returns the same value on every call.

**Note:** The spec regex accepts `0H` / `0M` / `0S` as syntactically valid, but `Every(0)` is a semantic no-op.

**Safe alternative:** Use `Every(n)` with n ≥ 1.

---

### Summary table

| Combination | Problem | Safe alternative |
|-------------|---------|-----------------|
| `HH:MM:SS` | No movement, infinite loop | Add at least one `Every(n)` |
| `hh:mm:ss` (as iterator) | Backwards on every tick | Use as `end_spec` only |
| `HH:mm:ss` (At min, no Every H/M) | Pins minute backwards, loops | `nH:mm:ss` or `HH:nM:ss` |
| `HH:MM:ss` (At sec, no Every H/M) | Pins second backwards, loops | `HH:nM:ss` or `nH:MM:ss` |
| `0H:00:00` / `HH:0M:00` / `HH:MM:0S` | Zero advance, infinite loop | Use n ≥ 1 |
