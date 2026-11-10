# tkone for Python

Business-day schedules: rules in one line of text, schedule documents around
them, and verified holiday calendars. Python bindings for
[tkone-schedule](https://github.com/govinda-attal/tkone/tree/main/crates/tkone-schedule)
and [tkone-calendar](https://github.com/govinda-attal/tkone/tree/main/crates/tkone-calendar).

One module per tkone crate: `tkone.schedule` and `tkone.calendar`.

```python
from tkone.schedule import Document

doc = Document({
    "version": 1,
    "time_zone": "Europe/London",
    "calendar": "GBLO",
    "start": "2026-01-01",
    "count": 3,
    "rules": [{"id": "report", "rule": "YY-MM-BD#3T09:00:00"}],
})
for slot in doc.slots():
    print(slot.at)
# 2026-01-06 09:00:00+00:00
# 2026-02-04 09:00:00+00:00
# 2026-03-04 09:00:00+00:00
```

## `tkone.schedule`

### `Document`: a schedule

`Document(doc)` takes a schedule document as a `dict` or a JSON string (read
YAML with a YAML library first). It holds one or more rules, with the start
they are anchored to, the time zone, the calendar (a `tkone.calendar` code,
`GBLO+USNY` for both, or `WEEKENDS`), an end (`until`) or a `count`, and
exclusions. See the
[schedule document design](https://github.com/govinda-attal/tkone/blob/main/docs/schedule-document.md).

| | |
|---|---|
| `doc.slots(after=None)` | an iterator of `Slot`, strictly after `after` (an aware `datetime`), or from the start |
| `doc.take(n, after=None)` | the first `n`, as a list |
| `doc.to_json()` | the document as JSON |

A `Slot` has `at` (an aware `datetime`) and `sources`: the rules that
produced it, each a `Source` with `rule_id` and `actual`, the time the rule
produced before any business-day adjustment. A rule derived from another
(`{"rule": "{payment}~2PT09:00:00"}`, two business days before each payment as
adjusted, with `{"id": "payment", ..., "basis": True}`) also has `basis`: the
payment it comes from. Rules that land on the same instant make one slot:

```python
doc = Document({
    "version": 1, "time_zone": "Europe/London", "calendar": "GBLO+USNY",
    "start": "2026-01-01", "count": 3,
    "rules": [{"id": "collection", "rule": "YY-1M-31L~MFT07:00:00"}],
})
for slot in doc.slots():
    print(slot.sources[0].actual.date(), "→", slot.at.date())
# 2026-01-31 → 2026-01-30
# 2026-02-28 → 2026-02-27
# 2026-03-31 → 2026-03-31
```

### `Schedule`: one rule

`Schedule(spec)` takes one rule: a date spec (`YY-MM-BD#3`), a time spec
(`09-16:15M:00`) or a datetime spec (`YY-MM-DD~SBT16:30:00`). See the
[format guide](https://github.com/govinda-attal/tkone/blob/main/crates/tkone-schedule/INTRODUCTION.md).

| | |
|---|---|
| `s.kind` | `"date"`, `"time"` or `"datetime"` |
| `s.describe()` | the rule in plain English |
| `s.occurrences(after=None, calendar=None, tz=None)` | an iterator of `Occurrence` |
| `s.take(n, …)` | the first `n`, as a list |

- **`after`**: an aware `datetime`; occurrences strictly after it. Without it,
  from now.
- **`calendar`**: a `tkone.calendar.Calendar` or a code. Without it, weekends
  only.
- **`tz`**: read the rule in this IANA zone (UTC if none).

An `Occurrence` has `actual` and `observed` (aware `datetime`s with a
`zoneinfo.ZoneInfo`) and `adjustment`: `"exact"`, `"later"` or `"earlier"`.

`describe(spec)` describes a rule without building a `Schedule`.

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
