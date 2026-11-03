"""Holiday calendars: the ``tkone-calendar`` crate.

Each calendar is checked against its publisher's official lists; see
https://github.com/govinda-attal/tkone/blob/main/crates/tkone-calendar/README.md
"""

from ._tkone import calendar as _native

Calendar = _native.Calendar
Holiday = _native.Holiday
CalendarError = _native.CalendarError
calendar_codes = _native.calendar_codes

__all__ = ["Calendar", "CalendarError", "Holiday", "calendar_codes"]
