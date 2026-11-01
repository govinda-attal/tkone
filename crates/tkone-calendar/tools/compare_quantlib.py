#!/usr/bin/env python3
"""Compare tkone calendars with QuantLib's, day by day.

    pip install QuantLib==1.44
    python3 tools/compare_quantlib.py            # every calendar below
    python3 tools/compare_quantlib.py GBLO       # one

For each calendar, lists the weekdays that are a holiday in one and not the
other, from FROM to UNTIL. A difference is either a fix to make, or listed in
KNOWN with the reason (QuantLib is a reference, not an authority).
"""

import subprocess
import sys
from pathlib import Path

import QuantLib as ql

CRATE = Path(__file__).resolve().parent.parent
FROM, UNTIL = 1990, 2075

# code -> (QuantLib calendar, first year to compare)
CALENDARS = {
    "GBLO": (ql.UnitedKingdom(ql.UnitedKingdom.Settlement), FROM),
    "EUTA": (ql.TARGET(), 1999),
    "USNY": (ql.UnitedStates(ql.UnitedStates.FederalReserve), FROM),
    "XNYS": (ql.UnitedStates(ql.UnitedStates.NYSE), FROM),
    "CATO": (ql.Canada(ql.Canada.Settlement), FROM),
    "AUSY": (ql.Australia(ql.Australia.Settlement), FROM),
    "NZAU": (ql.NewZealand(ql.NewZealand.Auckland), FROM),
    "NZWE": (ql.NewZealand(ql.NewZealand.Wellington), FROM),
    "ZAJO": (ql.SouthAfrica(), 1995),
}

# Calendars that end before UNTIL.
LAST_YEAR = {"NZAU": 2052, "NZWE": 2052}

# code -> {date: reason}: differences checked and accepted.
MISSING_FROM_QUANTLIB = "a declared public holiday QuantLib does not list; see the source in data/ZAJO.yaml"
KNOWN: dict[str, dict[str, str]] = {
    "AUSY": {
        "2026-04-27": "NSW Anzac Day additional day, by order; see data/AUSY.yaml",
        "2027-04-26": "NSW Anzac Day additional day, by order; see data/AUSY.yaml",
    },
    "ZAJO": {d: MISSING_FROM_QUANTLIB for d in [
        "1999-06-02", "1999-12-31", "2000-01-03", "2000-12-05",
        "2006-03-01", "2008-05-02", "2011-05-18", "2011-12-27",
        "2014-05-07", "2016-12-27", "2019-05-08",
    ]},
}


def tkone_days(code: str, first: int, last: int) -> set[str]:
    out = subprocess.run(
        ["cargo", "run", "-q", "-p", "tkone-calendar", "--features", "yaml", "--example", "dump", "--", code, str(first), str(last)],
        cwd=CRATE, check=True, capture_output=True, text=True,
    ).stdout
    return {line.split()[0] for line in out.splitlines() if line.strip()}


def quantlib_days(cal: ql.Calendar, first: int, last: int) -> set[str]:
    days = ql.Calendar.holidayList(cal, ql.Date(1, 1, first), ql.Date(31, 12, last), False)
    return {d.ISO() for d in days}


def main() -> int:
    codes = sys.argv[1:] or list(CALENDARS)
    failed = False
    for code in codes:
        cal, first = CALENDARS[code]
        last = LAST_YEAR.get(code, UNTIL)
        ours, theirs = tkone_days(code, first, last), quantlib_days(cal, first, last)
        known = KNOWN.get(code, {})
        diffs = sorted((ours ^ theirs) - set(known))
        print(f"{code}: {len(ours)} holidays {first}-{last}, QuantLib {len(theirs)}; "
              f"{len(diffs)} unexplained differences, {len(known)} known")
        for d in diffs:
            print(f"  {d}  {'tkone only' if d in ours else 'QuantLib only'}")
        failed |= bool(diffs)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
