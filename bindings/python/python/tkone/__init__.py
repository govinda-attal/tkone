"""tkone for Python: one module per tkone crate.

- ``tkone.schedule``: rules written in one line of text (``YY-MM-BD#3``),
  their plain-English descriptions and occurrences; and schedule documents,
  which put rules in context: start, time zone, calendar, end, count and
  exclusions.
- ``tkone.calendar``: verified holiday calendars (``GBLO``, ``USNY``, ...).

    >>> from tkone.schedule import Document
    >>> doc = Document({"version": 1, "time_zone": "Europe/London", "calendar": "GBLO",
    ...                 "start": "2026-01-01", "count": 3, "rules": [{"rule": "YY-MM-BD#3T09:00:00"}]})
    >>> [s.at.date().isoformat() for s in doc.slots()]
    ['2026-01-06', '2026-02-04', '2026-03-04']
"""

from . import calendar, schedule

__all__ = ["calendar", "schedule"]
__version__ = "0.1.0"
