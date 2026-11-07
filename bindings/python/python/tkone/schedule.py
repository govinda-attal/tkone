"""Rules and schedule documents: the ``tkone-schedule`` crate.

See the format guide:
https://github.com/govinda-attal/tkone/blob/main/crates/tkone-schedule/INTRODUCTION.md
"""

from ._tkone import schedule as _native

Schedule = _native.Schedule
Occurrence = _native.Occurrence
Occurrences = _native.Occurrences
Document = _native.Document
Slot = _native.Slot
Source = _native.Source
Slots = _native.Slots
ScheduleError = _native.ScheduleError
describe = _native.describe

__all__ = [
    "Document",
    "Occurrence",
    "Occurrences",
    "Schedule",
    "ScheduleError",
    "Slot",
    "Slots",
    "Source",
    "describe",
]
