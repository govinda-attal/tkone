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
Boundary = _native.Boundary
Period = _native.Period
Periods = _native.Periods
Timeline = _native.Timeline
Moment = _native.Moment
Entry = _native.Entry
Moments = _native.Moments
Selector = _native.Selector
Next = _native.Next
Bundle = _native.Bundle
ScheduleError = _native.ScheduleError
describe = _native.describe

__all__ = [
    "Boundary",
    "Bundle",
    "Document",
    "Entry",
    "Moment",
    "Moments",
    "Next",
    "Occurrence",
    "Occurrences",
    "Period",
    "Periods",
    "Schedule",
    "ScheduleError",
    "Selector",
    "Slot",
    "Slots",
    "Source",
    "Timeline",
    "describe",
]
