# Date Schedule Specification

## Format

```
YEAR-MONTH-DAY[~ADJUSTMENT]
```

All three components are required. The optional `~ADJUSTMENT` suffix shifts the result to a nearby business day or weekday.

---

## Component Reference

### Year

| Syntax | Meaning |
|--------|---------|
| `YY` | Every year (wildcard) |
| `nY` | Every *n* years from the start year (e.g. `1Y`, `2Y`, `3Y`) |
| `YYYY` | Specific year only (e.g. `2025`) |
| `[Y1,Y2,...]` | Enumerated years only (e.g. `[2025,2026]`) |

### Month

| Syntax | Meaning |
|--------|---------|
| `MM` | Every month (wildcard) |
| `nM` | Every *n* months from the start month (e.g. `1M`, `3M`, `6M`) |
| `MM` (2-digit) | Specific month only (e.g. `01`, `06`, `12`) |
| `[M1,M2,...]` | Enumerated months only (e.g. `[01,06]`, `[03,09]`) |

### Day

| Syntax | Meaning |
|--------|---------|
| `DD` | Every calendar day |
| `nD` | Every *n* calendar days (e.g. `4D`, `7D`, `14D`) |
| `nBD` | Every *n* business days (using configured processor) |
| `nWD` | Every *n* weekdays (Mon–Fri only, using WeekendSkipper) |
| `DD` (2-digit) | Fixed day-of-month (e.g. `01`, `15`) |
| `DDL` | Fixed day; clamp to last day of month on overflow (e.g. `31L`) |
| `DDN` | Fixed day; roll to 1st of next month on overflow (e.g. `31N`) |
| `DDO` | Fixed day; overflow into next month by remainder days (e.g. `31O`) |
| `L` | Last day of month |
| `WD` | Every occurrence of weekday (e.g. `MON`, `FRI`) |
| `[WD,WD,...]` | Every occurrence of any listed weekday (e.g. `[MON,WED,FRI]`) |
| `WD#N` | *N*th weekday of month (e.g. `MON#1`, `WED#2`) |
| `WD#L` | Last weekday of month (e.g. `FRI#L`) |
| `WD#NL` | *N*th-from-last weekday of month (e.g. `WED#2L`) |
| `[D1,D2,...]` | Enumerated days of month (e.g. `[01,15]`, `[01,10,20,25]`) |

### Business Day Adjustment (optional)

| Syntax | Meaning |
|--------|---------|
| `~NW` | If result is a weekend, move to **next** weekday (WeekendSkipper) |
| `~PW` | If result is a weekend, move to **previous** weekday (WeekendSkipper) |
| `~W` / `~NB` | If result is not a biz day, move to **next** biz day (configured processor) |
| `~B` / `~PB` | If result is not a biz day, move to **previous** biz day (configured processor) |
| `~nN` | **Unconditional**: add *n* biz days (e.g. `~2N`) |
| `~nP` | **Unconditional**: subtract *n* biz days (e.g. `~3P`) |

`~NW`/`~PW` always use the built-in WeekendSkipper (Sat/Sun only).  
`~W`/`~B` use the configured `BizDayProcessor` and can account for custom holidays.  
`~nN`/`~nP` are unconditional — they shift every result regardless of day-of-week.

---

## Result Types

- **`Single(date)`** — the date is exactly as computed, no adjustment needed.
- **`AdjustedLater(actual, observed)`** — `actual` is the computed date; `observed` is a later adjusted date.
- **`AdjustedEarlier(actual, observed)`** — `actual` is the computed date; `observed` is an earlier adjusted date.

---

## Examples

### 1. `YY-MM-DD` — Every calendar day

**Start:** 2025-01-01

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
 1. Single        2025-01-31
 2. AdjustedLater actual=2025-02-28  observed=2025-03-01
 3. Single        2025-03-31
 4. AdjustedLater actual=2025-04-30  observed=2025-05-01
 5. Single        2025-05-31
 6. AdjustedLater actual=2025-06-30  observed=2025-07-01
 7. Single        2025-07-31
 8. Single        2025-08-31
 9. AdjustedLater actual=2025-09-30  observed=2025-10-01
10. Single        2025-10-31
11. AdjustedLater actual=2025-11-30  observed=2025-12-01
12. Single        2025-12-31
13. Single        2026-01-31
14. AdjustedLater actual=2026-02-28  observed=2026-03-01
15. Single        2026-03-31
```

---

### 11. `YY-1M-31O` — 31st; overflow by remainder days into next month

**Start:** 2025-01-31

The `O` suffix means: when day 31 overflows by *k* days, `observed` = 1st of next month + (*k* − 1) days.
- February (28 days): overflow = 3 → observed = Mar 1 + 2 = **Mar 3**
- 30-day months: overflow = 1 → observed = 1st of next month + 0 = **1st of next month**

```
 1. Single        2025-01-31
 2. AdjustedLater actual=2025-02-28  observed=2025-03-03
 3. Single        2025-03-31
 4. AdjustedLater actual=2025-04-30  observed=2025-05-01
 5. Single        2025-05-31
 6. AdjustedLater actual=2025-06-30  observed=2025-07-01
 7. Single        2025-07-31
 8. Single        2025-08-31
 9. AdjustedLater actual=2025-09-30  observed=2025-10-01
10. Single        2025-10-31
11. AdjustedLater actual=2025-11-30  observed=2025-12-01
12. Single        2025-12-31
13. Single        2026-01-31
14. AdjustedLater actual=2026-02-28  observed=2026-03-03
15. Single        2026-03-31
```

---

### 12. `YY-MM-MON` — Every Monday

**Start:** 2025-01-06 (Monday)

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
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

```
 1. Single        2025-01-15  Wed
 2. AdjustedLater actual=2025-02-15 Sat  observed=2025-02-17 Mon
 3. AdjustedLater actual=2025-03-15 Sat  observed=2025-03-17 Mon
 4. Single        2025-04-15  Tue
 5. Single        2025-05-15  Thu
 6. AdjustedLater actual=2025-06-15 Sun  observed=2025-06-16 Mon
 7. Single        2025-07-15  Tue
 8. Single        2025-08-15  Fri
 9. Single        2025-09-15  Mon
10. Single        2025-10-15  Wed
11. AdjustedLater actual=2025-11-15 Sat  observed=2025-11-17 Mon
12. Single        2025-12-15  Mon
13. Single        2026-01-15  Thu
14. AdjustedLater actual=2026-02-15 Sun  observed=2026-02-16 Mon
15. AdjustedLater actual=2026-03-15 Sun  observed=2026-03-16 Mon
```

---

### 30. `YY-1M-15~PW` — 15th monthly; if weekend, move back to previous weekday

**Start:** 2025-01-15

```
 1. Single          2025-01-15  Wed
 2. AdjustedEarlier actual=2025-02-15 Sat  observed=2025-02-14 Fri
 3. AdjustedEarlier actual=2025-03-15 Sat  observed=2025-03-14 Fri
 4. Single          2025-04-15  Tue
 5. Single          2025-05-15  Thu
 6. AdjustedEarlier actual=2025-06-15 Sun  observed=2025-06-13 Fri
 7. Single          2025-07-15  Tue
 8. Single          2025-08-15  Fri
 9. Single          2025-09-15  Mon
10. Single          2025-10-15  Wed
11. AdjustedEarlier actual=2025-11-15 Sat  observed=2025-11-14 Fri
12. Single          2025-12-15  Mon
13. Single          2026-01-15  Thu
14. AdjustedEarlier actual=2026-02-15 Sun  observed=2026-02-13 Fri
15. AdjustedEarlier actual=2026-03-15 Sun  observed=2026-03-13 Fri
```

---

### 31. `YY-1M-15~3P` — 15th monthly; unconditionally subtract 3 business days

**Start:** 2025-01-15

`~3P` is **unconditional** — every result is shifted back 3 business days (WeekendSkipper), regardless of what day the 15th falls on.

```
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

**Start:** 2025-01-31

`~2N` is **unconditional** — every result is pushed forward 2 business days.

```
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

## Behaviour Notes

### Month-constrained relative day specs (`YY-01-4D`, `1Y-[01,06]-7D`)

When the month is a fixed set (`Values`) and the day is relative (`NextNth`), the N-day sequence **restarts from day 1** of each new month period. This contrasts with the unconstrained case (`YY-MM-7D`) where the step rolls transparently across month boundaries.

### Combined year+month clocks (`1Y-3M-15`)

When both year and month use `NextNth`, each tick advances **both** counters simultaneously. Month overflow carries into the year counter, which is why `1Y-3M-15` produces Dec→Mar with a 2-year gap rather than 1-year.

### `DDL` vs `DDN` vs `DDO`

All three handle the case where the target day does not exist in a given month:

| Suffix | Behaviour on overflow |
|--------|-----------------------|
| `L` | Clamp to last day; `Single(last_day)` |
| `N` | `AdjustedLater(last_day, first_of_next_month)` |
| `O` | `AdjustedLater(last_day, first_of_next_month + (overflow_days − 1))` |

### Iterator termination

Specs with finite year sets (`2025-MM-01`, `[2025,2026]-MM-15`) terminate once all valid dates are exhausted. Callers should handle `None` from the iterator.

### Start-date inclusivity

When constructed with `new_with_start`, the start date itself is always returned as the **first result** if it satisfies the spec (or is equal to the provided start). Use `new_after` to begin strictly after a given point in time.

---

## Semantically Illogical or Impossible Combinations

The following combinations are either silently broken, produce no results, or loop indefinitely. **Avoid them.**

---

### `nY-MM-DD` — ForEach month with a multi-year cadence

**Examples:** `2Y-MM-15`, `3Y-MM-01`

**Problem:** The `MM` (every-month) component relies on the month iterator to self-advance. However, `find_next_in_month_cycle` for `ForEach` always returns the current date unchanged — it never independently steps the month forward. As a result, **only the start month is ever visited** after the first year-tick. The spec silently degenerates to `nY-{start_month}-DD`.

```
Intended:  2Y-MM-15  starting 2025-03-15  →  2025-03-15, 2025-04-15, 2025-05-15 … 2027-03-15 …
Actual:    2Y-MM-15  starting 2025-03-15  →  2025-03-15  (only March, every 2 years)
```

**Safe alternative:** Use `YY-nM-DD` to tick monthly every year. For example, `YY-1M-15` visits the 15th of every month in every year.

---

### `1Y-MM-DD` — Every-year + ForEach month (partial issue)

**Examples:** `1Y-MM-01`, `1Y-MM-15`

**Problem:** This is the same root issue as above but with `n=1`. The month wildcard does not self-advance — the iteration stays pinned to the start month. For monthly-in-every-year behaviour, the correct spec is `YY-1M-DD` (which sets the year to `AsIs` and the month to `NextNth(1)`).

```
Intended:  1Y-MM-15  →  every 15th of every month
Actual:    1Y-MM-15  →  every 15th of the start month only, annually
```

**Safe alternative:** `YY-1M-15` (year wildcard, 1-month cadence).

---

### `YY-02-30`, `YY-02-31` — Fixed day that never exists in February (without overflow suffix)

**Examples:** `YY-02-30`, `YY-02-31`

**Problem:** Without an overflow suffix (`L`, `N`, or `O`), day 30 or 31 in February (`02`) simply does not exist. The iterator will find no valid date in any February and will either skip February silently or, in pathological cases, loop without progress.

```
YY-02-30  →  no February result (day 30 never exists)
YY-MM-31  →  skips all months with fewer than 31 days (Apr, Jun, Sep, Nov, Feb)
```

**Safe alternatives:**
- `YY-02-L` — last day of February (28 or 29 on leap years)
- `YY-02-28L` — 28th, clamped to last day on overflow (same effect as `L` here)
- `YY-02-28N` — 28th, with `AdjustedLater → 1 Mar` on non-leap years
- `YY-MM-31L` — 31st every month, clamped to last day when the month is shorter

---

### `WD#0` — Zero-th occurrence of a weekday

**Example:** `YY-MM-MON#0`

**Problem:** There is no "zeroth" weekday occurrence in any month. The `Starting(occurrence)` arm uses `occurrence.unwrap_or(1)` for `None`, but an explicit `0` is passed through as-is. `to_months_weekday` will never find a match, causing the iterator to loop until it hits the internal 10 000-iteration safety limit and then return an error.

**Safe alternative:** Use `WD#1` (first occurrence) or `WD#L` (last occurrence).

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
| `nY-MM-DD` (n > 1) | Month wildcard never self-advances; only start month visited | `YY-nM-DD` |
| `1Y-MM-DD` | Same as above for n=1 | `YY-1M-DD` |
| `YY-02-30` / `YY-02-31` | Day never exists in February | `YY-02-L` or `YY-02-28L` |
| `YY-MM-31` (no suffix) | Silently skips short months | `YY-MM-31L` (clamp) |
| `WD#0` | No 0th occurrence; loops to limit | `WD#1` or `WD#L` |
| `YYYY-02-29` (non-leap year) | Zero results | `YYYY-02-28L` |
| `[YYYY,...]-02-29` (mix) | Non-leap years silently skipped | Filter list to leap years only |
