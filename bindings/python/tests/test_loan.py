"""Testing a loan's computations against its schedule, in Python.

The same pattern as tkone-calendar's ``tests/loan.rs``: a ten-year loan on
fixed calendars, stepped through period by period. tkone gives the dates;
the interest, the day count and the principal are the test's own.
"""

import datetime as dt
from zoneinfo import ZoneInfo

import pytest

from tkone.calendar import Calendar
from tkone.schedule import Document, ScheduleError, Timeline

LONDON = ZoneInfo("Europe/London")
PRINCIPAL = 12_000_000  # 120,000.00, in cents
RATE_BP = 500  # 5% a year


def d(y, m, day):
    return dt.date(y, m, day)


def midnight(y, m, day):
    return dt.datetime(y, m, day, tzinfo=LONDON)


def loan_calendar(code="GBLO"):
    """Weekends, Christmas, Boxing Day and New Year's Day, 2026 to 2036."""
    days_off = [day for y in range(2026, 2037) for day in (d(y, 1, 1), d(y, 12, 25), d(y, 12, 26))]
    return Calendar.fixed(code, d(2026, 1, 1), d(2036, 12, 31), days_off=days_off)


def repayments(calendar=None):
    """Drawn on 15 January 2026; repaid on the 15th, Modified Following, for ten years."""
    return Document(
        {
            "version": 1,
            "time_zone": "Europe/London",
            "calendar": "GBLO",
            "start": "2026-01-15",
            "until": "2036-01-15",
            "rules": [{"id": "repayment", "rule": "YY-MM-15~MFT00:00:00"}],
            # The drawdown starts the schedule; it is not a repayment.
            "exclude": {"dates": ["2026-01-15"]},
        },
        calendars=[calendar or loan_calendar()],
    )


def days(a, b):
    # Dates, not instants: a period across a clock change is not whole days.
    return (b.date() - a.date()).days


def amortise(doc, unadjusted=True):
    periods = list(doc.periods())
    repayment = PRINCIPAL // len(periods)
    balance, interest = PRINCIPAL, 0
    for p in periods:
        n = days(p.start.actual, p.end.actual) if unadjusted else days(p.start.at, p.end.at)
        interest += balance * RATE_BP * n // (10_000 * 365)
        balance -= repayment
    return interest, balance


def test_every_period_is_accounted_for():
    calendar = loan_calendar()
    doc = repayments(calendar)
    periods = list(doc.periods())

    assert doc.moved_before_start() == []
    # The drawdown opens the first period; 120 repayments close them.
    assert len(periods) == 120
    assert periods[0].start.at == midnight(2026, 1, 15)
    for a, b in zip(periods, periods[1:]):
        assert (b.start.at, b.start.actual) == (a.end.at, a.end.actual)
    term = days(midnight(2026, 1, 15), midnight(2036, 1, 15))
    assert sum(days(p.start.actual, p.end.actual) for p in periods) == term
    assert sum(days(p.start.at, p.end.at) for p in periods) == term
    for p in periods:
        paid, due = p.end.at.date(), p.end.actual.date()
        assert calendar.is_business_day(paid)
        assert (paid.year, paid.month) == (due.year, due.month)
    # 15 March 2026 is a Sunday: repaid on Monday the 16th.
    assert (periods[1].end.actual, periods[1].end.at) == (midnight(2026, 3, 15), midnight(2026, 3, 16))
    assert amortise(doc)[1] == 0


def test_one_year_of_interest():
    periods = list(repayments().periods(midnight(2030, 1, 1), midnight(2030, 12, 31)))
    assert len(periods) == 12
    assert periods[0].start.actual == midnight(2029, 12, 15)
    assert sum(days(p.start.actual, p.end.actual) for p in periods) == 365


def test_a_closure_at_short_notice():
    closed = loan_calendar().with_days_off([d(2027, 9, 15)])
    before, after = repayments(), repayments(closed)
    paid = lambda doc: [s.at.date() for s in doc.slots()]
    moved = [(a, b) for a, b in zip(paid(before), paid(after)) if a != b]
    assert moved == [(d(2027, 9, 15), d(2027, 9, 16))]
    assert amortise(before) == amortise(after)
    assert amortise(after, unadjusted=False)[0] > amortise(before, unadjusted=False)[0]


def test_a_drawdown_on_a_month_end_weekend():
    doc = Document(
        {
            "version": 1,
            "time_zone": "Europe/London",
            "calendar": "GBLO",
            "start": "2026-01-31",
            "rules": [{"rule": "YY-MM-L~PBT00:00:00"}],
        },
        calendars=[loan_calendar()],
    )
    lost = doc.moved_before_start()
    assert [(s.at, s.sources[0].actual) for s in lost] == [(midnight(2026, 1, 30), midnight(2026, 1, 31))]


def test_a_floating_rate_over_ten_years():
    rate_calendar = Calendar.fixed(
        "EUTA", d(2026, 1, 1), d(2036, 12, 31), days_off=[d(y, 5, 1) for y in range(2026, 2037)]
    )
    fixings = Document(
        {
            "version": 1,
            "time_zone": "Europe/London",
            "calendar": "EUTA",
            "start": "2026-01-01",
            "until": "2035-12-31",
            "rules": [{"rule": "YY-MM-15~2PT00:00:00"}],
        },
        calendars=[rate_calendar],
    )
    timeline = Timeline([("fixing", fixings), ("repayment", repayments())])
    assert timeline.keys == ["fixing", "repayment"]
    moments = list(timeline.between(midnight(2026, 1, 1), midnight(2036, 1, 31)))
    keys = [e.key for m in moments for e in m.entries]
    assert keys == ["fixing"] + ["fixing", "repayment"] * 119 + ["repayment"]

    rate, next_rate, balance = None, None, PRINCIPAL
    for entry in (e for m in moments for e in m.entries):
        if entry.key == "fixing":
            fixed = 400 + (entry.slot.at.month % 3) * 50
            if rate is None:
                rate = fixed
            else:
                next_rate = fixed
            continue
        assert rate is not None
        balance -= PRINCIPAL // 120
        rate, next_rate = (next_rate or rate), None
    assert balance == 0


def test_errors():
    with pytest.raises(ScheduleError, match="need an `end`"):
        repayments().periods(start=midnight(2026, 1, 1))
    from tkone.calendar import CalendarError

    with pytest.raises(CalendarError, match="day of the week"):
        Calendar.fixed("X", d(2026, 1, 1), d(2026, 12, 31), weekend=["Caturday"])
    with pytest.raises(CalendarError, match="one by one"):
        repayments(Calendar("GBLO+USNY"))
    # Built-in data for codes not given.
    doc = Document(
        {"version": 1, "time_zone": "America/New_York", "calendar": "XNYS", "start": "2026-07-01",
         "rules": [{"rule": "YY-MM-DD~SBT12:00:00"}]},
        calendars=[loan_calendar()],
    )
    assert [s.at.day for s in doc.take(3)] == [1, 2, 6]  # Fri 3 July: the exchange is closed for Independence Day
    assert [s.at.day for s in doc.between(dt.datetime(2026, 7, 2, tzinfo=dt.timezone.utc), dt.datetime(2026, 7, 6, 23, tzinfo=dt.timezone.utc))] == [2, 6]
    assert doc.until is None and doc.start.isoformat() == "2026-07-01T00:00:00-04:00"
