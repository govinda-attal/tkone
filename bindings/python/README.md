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
| `doc.slots(after=None)` | an iterator of `Slot`, strictly after `after`, or from the start |
| `doc.take(n, after=None)` | the first `n`, as a list |
| `doc.between(start, end)` | the slots from `start` to `end`, both included; always ends |
| `doc.periods(start=None, end=None)` | an iterator of `Period`, from one slot to the next (see below) |
| `doc.moved_before_start()` | slots an adjustment moved before the start, and so dropped: for a loan, a payment that never happens |
| `doc.start`, `doc.until` | the start, and the end if any |
| `doc.to_json()` | the document as JSON |

Times passed in are aware `datetime`s with any `tzinfo` (`zoneinfo.ZoneInfo`
or `datetime.timezone`); a naive one raises `ScheduleError`.
`Document(doc, calendars=[...])` uses the given `Calendar`s for their codes
instead of the built-in ones; see [Testing](#testing-with-schedules).

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

### Testing with schedules

To check a loan's interest and repayments, step through its **periods**. Each
has `start` and `end` boundaries with both times: `at`, when it happens, and
`actual`, when the rule put it before adjustment. Which one interest counts is
the contract's choice, and the day count is yours. A **fixed calendar** never
changes, so expected dates don't break when holiday data is updated, and
`with_days_off` simulates a closure at short notice. A **timeline** reads
several documents as one, moment by moment.

```python
import datetime as dt
from zoneinfo import ZoneInfo
from tkone.calendar import Calendar
from tkone.schedule import Document, Timeline

london = ZoneInfo("Europe/London")
loan = {
    "version": 1, "time_zone": "Europe/London", "calendar": "GBLO",
    "start": "2026-01-15", "until": "2026-04-15",
    "rules": [{"rule": "YY-MM-15~MFT00:00:00"}],
    "exclude": {"dates": ["2026-01-15"]},     # the drawdown is not a repayment
}
# A calendar that never changes: weekends, and Monday 16 March off.
test = Calendar.fixed("GBLO", dt.date(2026, 1, 1), dt.date(2026, 12, 31), days_off=[dt.date(2026, 3, 16)])
doc = Document(loan, calendars=[test])
assert doc.moved_before_start() == []
for p in doc.periods():
    days = (p.end.actual.date() - p.start.actual.date()).days
    print(p.start.at.date(), "→", p.end.at.date(), f"{days} days")
# 2026-01-15 → 2026-02-16 31 days
# 2026-02-16 → 2026-03-17 28 days
# 2026-03-17 → 2026-04-15 31 days

# Rates fixed two business days before each period starts, derived from the
# boundaries as adjusted.
fixings = Document({**loan, "start": "2026-01-01", "until": "2026-03-31",
                    "rules": [{"id": "boundary", "rule": "YY-MM-15~MFT00:00:00", "basis": True},
                              {"rule": "{boundary}~2PT11:00:00"}], "exclude": {}}, calendars=[test])
timeline = Timeline([("fixing", fixings), ("repayment", doc)])
for m in timeline.between(dt.datetime(2026, 1, 1, tzinfo=london), dt.datetime(2026, 3, 31, tzinfo=london)):
    print(m.at.strftime("%a %d %b %H:%M"), [e.key for e in m.entries])
# Tue 13 Jan 11:00 ['fixing']
# Thu 12 Feb 11:00 ['fixing']
# Mon 16 Feb 00:00 ['repayment']
# Thu 12 Mar 11:00 ['fixing']
# Tue 17 Mar 00:00 ['repayment']
```

| | |
|---|---|
| `Period` | `start`, `end` (`Boundary`: `at`, `actual`) and `slot`, the slot that ends it |
| `Calendar.fixed(code, valid_from, valid_until, weekend=("Sat", "Sun"), days_off=())` | a calendar written out in full |
| `calendar.with_days_off(dates)` | the same calendar with more days off |
| `calendar.holidays_between(start, end)` | a calendar's days off, to copy into a fixed one |
| `Timeline([(key, doc), ...]).between(start, end)` | an iterator of `Moment`: `at`, and `entries` in the order given, each an `Entry` with `key`, `slot` and `since` (the document's previous boundary) |

[`tests/test_loan.py`](tests/test_loan.py) steps a ten-year loan through
this way, with a floating rate on a second calendar. The design is in
[timelines](https://github.com/govinda-attal/tkone/blob/main/docs/timeline.md)
and [a loan's life](https://github.com/govinda-attal/tkone/blob/main/docs/loan-lifecycle.md).

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

- **`after`**: an aware `datetime` (any `tzinfo`); occurrences strictly after
  it. Without it, from now.
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
