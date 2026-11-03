import datetime as dt

import pytest

from tkone.calendar import Calendar
from tkone.schedule import Schedule, ScheduleError, describe

UTC = dt.timezone.utc


def dates(occurrences):
    return [o.observed.date().isoformat() for o in occurrences]


def test_kinds_and_describe():
    assert Schedule("YY-MM-BD#3").kind == "date"
    assert Schedule("09-16:15M:00").kind == "time"
    assert Schedule("YY-MM-DD~SBT16:30:00").kind == "datetime"
    assert describe("YY-MM-FRI#L") == "on the last Friday of every month"
    s = Schedule("YY-MM-BD#3 ~MF @2026-01-01[Europe/London] [cal=GBLO] x3")
    assert (s.calendar, s.zone) == ("GBLO", "Europe/London")


def test_calendar_named_in_the_spec():
    s = Schedule("YY-MM-BD#3 ~MF @2026-01-01 [cal=GBLO] x3")
    assert dates(s.occurrences()) == ["2026-01-06", "2026-02-04", "2026-03-04"]
    # Another calendar than the one named is refused.
    with pytest.raises(ScheduleError, match="names calendar GBLO"):
        s.take(1, calendar="USNY")


def test_actual_and_observed():
    s = Schedule("YY-1M-31L~MF @2026-01-31 [cal=GBLO+USNY] x8")
    occ = s.take(8)
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
    d = Schedule("YY-MM-DD~SBT16:30:00 @2026-11-25[America/New_York] ..2026-11-30 [cal=XNYS]")
    occ = list(d.occurrences())
    # Thanksgiving (26 November) is skipped.
    assert [o.observed.isoformat() for o in occ] == [
        "2026-11-25T16:30:00-05:00",
        "2026-11-27T16:30:00-05:00",
        "2026-11-30T16:30:00-05:00",
    ]
    assert str(occ[0].observed.tzinfo) == "America/New_York"
    # A run on another date in the calendar's zone is refused.
    with pytest.raises(ScheduleError, match="America/New_York"):
        Schedule("YY-MM-DDT02:00:00 @2026-11-25 [cal=XNYS]").take(1)
    # The clocks go forward on 29 March 2026 in London.
    t = Schedule("HH:30M:00 @2026-03-29T00:00[Europe/London] x4")
    assert [o.observed.strftime("%H:%M %Z") for o in t.occurrences()] == ["00:00 GMT", "00:30 GMT", "02:00 BST", "02:30 BST"]


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
    assert Calendar.__module__ == "tkone.calendar"
    assert ScheduleError.__module__ == "tkone.schedule"
