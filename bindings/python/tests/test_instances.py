"""Instances (selectors), and sets of documents that refer to each other."""

import datetime as dt
from zoneinfo import ZoneInfo

import pytest

from tkone.schedule import Bundle, Document, ScheduleError, Selector, Timeline

LONDON = ZoneInfo("Europe/London")


def at(y, m, d, h=0, mi=0):
    return dt.datetime(y, m, d, h, mi, tzinfo=LONDON)


def definition(time, start="2026-01-01"):
    """A batch at `time` every London business day."""
    return Document(
        {
            "version": 1,
            "time_zone": "Europe/London",
            "calendar": "GBLO",
            "start": start,
            "rules": [{"id": "batch", "rule": f"YY-MM-DD~SBT{time}"}],
        }
    )


SERVICING = definition("12:00:00")
NOTICES = definition("09:00:00")
BATCHES = Document(
    {
        "version": 1,
        "time_zone": "Europe/London",
        "calendar": "GBLO",
        "start": "2026-01-01",
        "rules": [{"rule": "YY-MM-DD~SBT[09,13,18]:00:00"}],
    }
)


def month_end(rule="YY-MM-L~PBT18:00:00", **extra):
    return {"version": 1, "start": "2026-03-15", "rules": [{"id": "accrue", "rule": rule}], **extra}


def test_a_selector_picks_its_definitions_slots():
    account = Selector(month_end(), BATCHES)
    assert account.start == at(2026, 3, 15)
    # 31 May 2026 is a Sunday: the month end is Friday 29 May's 18:00 batch.
    nxt = account.next_after(at(2026, 5, 1), at(2026, 7, 1))
    assert (nxt.kind, nxt.slot.at, nxt.slot.sources[0].actual) == ("due", at(2026, 5, 29, 18), at(2026, 5, 31, 18))
    assert nxt.missed == []
    # The same through the definition.
    assert BATCHES.selector(month_end()).next_after(at(2026, 5, 1), at(2026, 7, 1)).slot.at == nxt.slot.at
    # It wants a slot from its own rules, wherever the definition is.
    assert [s.at for s in account.take(2)] == [at(2026, 3, 31, 18), at(2026, 4, 30, 18)]


def test_missed_not_found_and_done():
    # 17:00 is not a batch: wanted, missed, never due.
    late = Selector(month_end("YY-MM-L~PBT17:00:00"), BATCHES)
    nxt = late.next_after(at(2026, 5, 1), at(2026, 7, 1))
    assert nxt.kind == "not_found" and nxt.slot is None
    assert [s.at for s in nxt.missed] == [at(2026, 5, 29, 17), at(2026, 6, 30, 17)]
    assert [s.at for s in late.uncovered(at(2026, 3, 15), at(2026, 4, 30, 23))] == [at(2026, 3, 31, 17), at(2026, 4, 30, 17)]
    assert Selector(month_end(), BATCHES).uncovered(at(2026, 3, 15), at(2026, 12, 31)) == []
    # A count becomes `until`, counted over its own slots; then it is done.
    three = Selector(month_end(count=3), BATCHES)
    assert three.until == at(2026, 5, 29, 18)
    assert three.next_after(at(2026, 5, 29, 18), at(2026, 12, 31)).kind == "done"


def test_an_instance_uses_its_definitions_context():
    with pytest.raises(ScheduleError, match="may not set `calendar`"):
        Selector({**month_end(), "calendar": "USNY"}, BATCHES)


def loan(reminder="{collections.repayment}~2PT09:00:00"):
    return Bundle(
        instances={
            "collections": ({"version": 1, "start": "2026-01-01", "rules": [{"id": "repayment", "rule": "YY-MM-L~MFT12:00:00"}]}, SERVICING),
            "reminders": ({"version": 1, "start": "2026-01-01", "rules": [{"id": "reminder", "rule": reminder}]}, NOTICES),
        }
    )


def test_a_set_refers_across_documents():
    bundle = loan()
    assert bundle.names == ["collections", "reminders"]
    reminders = bundle.selector("reminders")
    # 31 May 2026 is a Sunday: repaid Friday 29 May, reminded Wednesday 27 May.
    nxt = reminders.next_after(at(2026, 5, 1), at(2026, 7, 1))
    assert (nxt.kind, nxt.slot.at, nxt.slot.sources[0].basis) == ("due", at(2026, 5, 27, 9), at(2026, 5, 29, 12))
    assert reminders.uncovered(at(2026, 1, 1), at(2026, 12, 31)) == []
    assert bundle.selector("collections").take(1)[0].at == at(2026, 1, 30, 12)
    # A timeline of the instances.
    timeline = Timeline([("reminder", reminders), ("repayment", bundle.selector("collections"))])
    moments = [(m.at, [e.key for e in m.entries]) for m in timeline.between(at(2026, 5, 20), at(2026, 5, 31))]
    assert moments == [(at(2026, 5, 27, 9), ["reminder"]), (at(2026, 5, 29, 12), ["repayment"])]


def test_definitions_in_a_set():
    bundle = Bundle(
        definitions={
            "close": {"version": 1, "time_zone": "America/New_York", "calendar": "XNYS", "start": "2026-06-01",
                      "rules": [{"id": "close", "rule": "YY-MM-DD~SBT16:00:00"}]},
            "report": {"version": 1, "time_zone": "Europe/London", "calendar": "GBLO", "start": "2026-06-01",
                       "rules": [{"id": "report", "rule": "{close.close}~1NT09:00:00"}]},
        }
    )
    # Friday 5 June's New York close is reported in London on Monday 8 June.
    reports = [s.at for s in bundle.document("report").between(at(2026, 6, 8), at(2026, 6, 8, 23))]
    assert reports == [at(2026, 6, 8, 9)]
    with pytest.raises(ScheduleError, match="is a definition"):
        bundle.selector("report")
    with pytest.raises(ScheduleError, match="no document"):
        bundle.document("nope")


def test_a_set_is_accepted_as_a_whole():
    with pytest.raises(ScheduleError, match="no schedule `payments`"):
        loan("{payments.repayment}~2PT09:00:00")
    with pytest.raises(ScheduleError, match="not one of its rules"):
        loan("{collections.payment}~2PT09:00:00")
    looped = {
        "a": {"version": 1, "time_zone": "UTC", "calendar": "WEEKENDS", "start": "2026-01-01",
              "rules": [{"id": "x", "rule": "YY-MM-15T09:00:00"}, {"rule": "{b.y}~1PT09:00:00"}]},
        "b": {"version": 1, "time_zone": "UTC", "calendar": "WEEKENDS", "start": "2026-01-01",
              "rules": [{"id": "y", "rule": "YY-MM-20T09:00:00"}, {"rule": "{a.x}~1PT09:00:00"}]},
    }
    with pytest.raises(ScheduleError, match="in a loop: a → b → a"):
        Bundle(definitions=looped)
    with pytest.raises(ScheduleError, match="definitions refer only to definitions"):
        Bundle(
            definitions={"d": {"version": 1, "time_zone": "Europe/London", "calendar": "GBLO", "start": "2026-01-01",
                               "rules": [{"rule": "{i.repayment}~1PT09:00:00"}]}},
            instances={"i": ({"version": 1, "start": "2026-01-01", "rules": [{"id": "repayment", "rule": "YY-MM-L~MFT12:00:00"}]}, SERVICING)},
        )
    # Outside a set, a reference to another document is refused.
    with pytest.raises(ScheduleError, match="not in its bundle"):
        Document({"version": 1, "time_zone": "UTC", "calendar": "WEEKENDS", "start": "2026-01-01", "rules": [{"rule": "{a.x}~1NT09:00:00"}]})
