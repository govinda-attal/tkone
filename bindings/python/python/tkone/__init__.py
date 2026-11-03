"""tkone for Python: one module per tkone crate.

- ``tkone.schedule``: schedules written in one line of text
  (``YY-MM-BD#3 ~MF [cal=GBLO]``), their plain-English descriptions and
  occurrences.
- ``tkone.calendar``: verified holiday calendars (``GBLO``, ``USNY``, ...).

    >>> from tkone.schedule import Schedule
    >>> [o.observed.date().isoformat() for o in Schedule("YY-MM-BD#3 ~MF @2026-01-01 [cal=GBLO] x3").occurrences()]
    ['2026-01-06', '2026-02-04', '2026-03-04']
"""

from . import calendar, schedule

__all__ = ["calendar", "schedule"]
__version__ = "0.1.0"
