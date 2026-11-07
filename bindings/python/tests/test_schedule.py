import datetime as dt
import json

import pytest

from tkone.calendar import Calendar
from tkone.schedule import Document, Schedule, ScheduleError, describe

UTC = dt.timezone.utc


def dates(occurrences):
    return [o.observed.date().isoformat() for o in occurrences]


def test_kinds_and_describe():
    assert Schedule("YY-MM-BD#3").kind == "date"
    assert Schedule("09-16:15M:00").kind == "time"
    assert Schedule("YY-MM-DD~SBT16:30:00").kind == "datetime"
    assert describe("YY-MM-FRI#L") == "on the last Friday of every month"
    assert describe("YY-01-29~MON") == "on the 29th of January, moved to the nearest Monday"


def test_actual_and_observed():
    s = Schedule("YY-1M-31L~MF")
    after = dt.datetime(2026, 1, 1, tzinfo=UTC)
    occ = s.take(8, after=after, calendar="GBLO+USNY")
    assert [(o.actual.date().isoformat(), o.observed.date().isoformat(), o.adjustment) for o in occ][::7] == [
        ("2026-01-31", "2026-01-30", "earlier"),
        ("2026-08-31", "2026-08-28", "earlier"),  # London's Summer bank holiday
    ]


def test_calendar_passed_in_and_after():
    s = Schedule("YY-MM-15~NB")
    after = dt.datetime(2026, 1, 1, tzinfo=UTC)
    assert dates(s.take(2, after=after, calendar="GBLO")) == ["2026-01-15", "2026-02-16"]
    assert dates(s.take(1, after=after, calendar=Calendar("GBLO"))) == ["2026-01-15"]
    # Weekends only when there is no calendar.
    assert dates(s.take(2, after=after)) == ["2026-01-15", "2026-02-16"]


def test_time_zones():
    # The clocks go forward on 29 March 2026 in London.
    after = dt.datetime(2026, 3, 28, 23, 30, tzinfo=UTC)
    t = Schedule("HH:30M:00").take(4, after=after, tz="Europe/London")
    assert [o.observed.strftime("%H:%M %Z") for o in t] == ["00:00 GMT", "00:30 GMT", "02:00 BST", "02:30 BST"]


def test_rules_are_bare():
    for spec in ["YY-MM-DD ! YY-12-25", "YY-MM-01 | YY-MM-15", "YY-MM-15 @2026-01-01", "YY-MM-15 [cal=GBLO]"]:
        with pytest.raises(ScheduleError):
            Schedule(spec)


EOD = {
    "version": 1,
    "time_zone": "America/New_York",
    "calendar": "XNYS",
    "start": "2026-11-25",
    "until": "2026-11-30",
    "rules": [
        {"id": "reconciliation", "rule": "YY-MM-DD~SBT16:30:00"},
        {"id": "month-end", "rule": "YY-MM-BD#LT16:30:00"},
    ],
}


def test_document():
    doc = Document(EOD)
    slots = list(doc.slots())
    # Thanksgiving (26 November) is skipped.
    assert [s.at.isoformat() for s in slots] == [
        "2026-11-25T16:30:00-05:00",
        "2026-11-27T16:30:00-05:00",
        "2026-11-30T16:30:00-05:00",
    ]
    assert str(slots[0].at.tzinfo) == "America/New_York"
    # Rules that coincide make one slot with both sources.
    assert [s.rule_id for s in slots[-1].sources] == ["reconciliation", "month-end"]
    # After an instant; the same as a JSON string.
    after = dt.datetime(2026, 11, 26, tzinfo=UTC)
    assert [s.at.date().isoformat() for s in Document(json.dumps(EOD)).take(5, after=after)] == ["2026-11-27", "2026-11-30"]
    assert json.loads(doc.to_json()) == EOD


def test_document_errors():
    # A run on another date in the calendar's zone is refused.
    late = {**EOD, "time_zone": "UTC", "rules": [{"rule": "YY-MM-DDT02:00:00"}]}
    with pytest.raises(ScheduleError, match="America/New_York"):
        Document(late).take(1)
    # Business days need a calendar.
    with pytest.raises(ScheduleError, match="no `calendar`"):
        Document({k: v for k, v in EOD.items() if k != "calendar"})
    with pytest.raises(ScheduleError, match="unknown field"):
        Document({**EOD, "untill": "2026-12-31"})


def test_errors():
    with pytest.raises(ScheduleError, match="month 13"):
        Schedule("YY-13-01")
    with pytest.raises(ScheduleError, match="no business days"):
        Schedule("09:00:00").take(1, calendar="GBLO")
    assert issubclass(ScheduleError, ValueError)


def test_package_layout():
    import tkone

    assert {"schedule", "calendar"} <= set(dir(tkone))
    assert Schedule.__module__ == "tkone.schedule"
    assert Document.__module__ == "tkone.schedule"
    assert Calendar.__module__ == "tkone.calendar"
    assert ScheduleError.__module__ == "tkone.schedule"
