import datetime as dt

import pytest

from tkone.calendar import Calendar, CalendarError, calendar_codes


def test_calendars():
    assert "GBLO" in calendar_codes()
    london = Calendar("GBLO")
    assert london.zone == "Europe/London"
    assert not london.is_business_day(dt.date(2026, 8, 31))
    assert london.is_holiday(dt.date(2026, 8, 31))
    assert london.status(dt.date(2026, 1, 2)) == "confirmed"
    assert london.status(dt.date(2030, 1, 2)) == "projected"
    xmas = [h for h in london.holidays(2022) if h.name == "Christmas Day"][0]
    assert (xmas.actual, xmas.observed) == (dt.date(2022, 12, 25), dt.date(2022, 12, 27))
    assert Calendar("GBLO+USNY").zone is None



def test_calendar_errors():
    with pytest.raises(CalendarError, match="unknown calendar"):
        Calendar("XXXX")
    with pytest.raises(CalendarError, match="2101-01-01"):
        Calendar("GBLO").is_business_day(dt.date(2101, 1, 1))
    assert issubclass(CalendarError, ValueError)
