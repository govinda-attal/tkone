"""Schedules written in one line of text: the ``tkone-schedule`` crate.

See the format guide:
https://github.com/govinda-attal/tkone/blob/main/crates/tkone-schedule/INTRODUCTION.md
"""

from ._tkone import schedule as _native

Schedule = _native.Schedule
Occurrence = _native.Occurrence
Occurrences = _native.Occurrences
ScheduleError = _native.ScheduleError
describe = _native.describe

__all__ = ["Occurrence", "Occurrences", "Schedule", "ScheduleError", "describe"]
