# tkone for Python

Business-day schedules in one line of text, with verified holiday calendars.
Python bindings for [tkone-schedule](https://github.com/govinda-attal/tkone/tree/main/crates/tkone-schedule)
and [tkone-calendar](https://github.com/govinda-attal/tkone/tree/main/crates/tkone-calendar).

One module per tkone crate: `tkone.schedule` and `tkone.calendar`.

```python
from tkone.schedule import Schedule

s = Schedule("YY-MM-BD#3 ~MF @2026-01-01 [cal=GBLO] x3")
print(s.describe())
# on the 3rd business day of every month, moved to the next business day ...

for o in s.occurrences():
    print(o.observed.date())
# 2026-01-06
# 2026-02-04
# 2026-03-04
```

Each occurrence keeps the time the rule produced and the time it moved to:

```python
s = Schedule("YY-1M-31L~MF @2026-01-31 [cal=GBLO+USNY] x3")
for o in s.occurrences():
    print(o.actual.date(), "→", o.observed.date(), o.adjustment)
# 2026-01-31 → 2026-01-30 earlier
# 2026-02-28 → 2026-02-27 earlier
# 2026-03-31 → 2026-03-31 exact
```

## `tkone.schedule`

`Schedule(spec)` takes any spec: a date spec (`YY-MM-BD#3`), a time
spec (`09-16:15M:00`) or a datetime spec (`YY-MM-DD~SBT16:30:00`), with a
start (`@…`), end (`..…`, `..+30Y`), count (`x12`), time zone (`[Europe/London]`)
and calendar (`[cal=GBLO]`) written after it. See the
[format guide](https://github.com/govinda-attal/tkone/blob/main/crates/tkone-schedule/INTRODUCTION.md).

| | |
|---|---|
| `s.kind` | `"date"`, `"time"` or `"datetime"` |
| `s.describe()` | the spec in plain English |
| `s.calendar`, `s.zone` | the calendar code and time zone written in the spec |
| `s.occurrences(after=None, calendar=None, tz=None)` | an iterator of `Occurrence` |
| `s.take(n, …)` | the first `n`, as a list |

- **`after`**: an aware `datetime`; occurrences strictly after it. Without it,
  from the spec's written start, else from now.
- **`calendar`**: a `tkone.calendar.Calendar` or a code. Without it, the calendar the
  spec names, else weekends only. A spec that names a calendar only runs on
  that calendar.
- **`tz`**: read the schedule in this IANA zone instead of the written one
  (UTC if none).

An `Occurrence` has `actual` and `observed` (aware `datetime`s with a
`zoneinfo.ZoneInfo`) and `adjustment`: `"exact"`, `"later"` or `"earlier"`.

`describe(spec)` describes a spec without building a `Schedule`.

## `tkone.calendar`

```python
import datetime
from tkone.calendar import Calendar, calendar_codes

london = Calendar("GBLO")
london.is_business_day(datetime.date(2026, 8, 31))   # False: Summer bank holiday
london.holidays(2026)                                 # [Holiday("New Year's Day", actual=2026-01-01, ...), ...]
london.status(datetime.date(2030, 1, 2))              # 'projected'
calendar_codes()                                      # ['AUSY', 'CATO', 'EUTA', 'GBLO', ...]
Calendar("GBLO+EUTA")                                 # open in both
```

A date outside a calendar's range raises `tkone.calendar.CalendarError`; an
invalid spec, or a schedule that cannot go on, raises
`tkone.schedule.ScheduleError`. Both are `ValueError`s.

How the bindings are organised, and how to add a crate:
[`bindings/README.md`](../README.md).

## Building

```sh
pip install maturin
maturin develop          # into the current virtualenv
pytest tests
```

One wheel works on Python 3.9 and later (the stable ABI).
