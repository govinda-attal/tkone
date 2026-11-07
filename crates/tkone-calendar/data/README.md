# Calendar data

One YAML file per calendar, named after its code. The crate does not read
these files directly: they are checked and compiled into
`compiled/calendars.json`, which it embeds.

```sh
# After editing: check every calendar and update compiled/calendars.json
cargo run -p tkone-calendar --features yaml --bin tkone-calendar-compile

# Compare with QuantLib, day by day (pip install QuantLib)
python3 tools/compare_quantlib.py GBLO
```

## A calendar file

```yaml
code: GBLO                      # FpML / ISDA business centre, or an exchange MIC
name: London (England and Wales bank holidays)
kind: settlement                # public | settlement | exchange
zone: Europe/London             # IANA time zone
weekend: [SAT, SUN]
valid_from: "1990-01-01"        # outside these dates the calendar returns an error
valid_until: "2099-12-31"
confirmed_until: "2026-12-31"   # last date checked against published sources
substitute: next-free-weekday   # how weekend holidays move; see below
sources:                        # laws and pages the rules come from
  - Banking and Financial Dealings Act 1971, Schedule 1

holidays:
  - name: Spring bank holiday
    rule: "YY-05-MON#L"                     # a tkone date spec
    except: ["2022-05-30"]                  # dates the rule does not give
    source: Royal Proclamation              # required with `except`
  - name: Spring bank holiday
    dates: ["2022-06-02"]                   # listed dates, the days off as they are
    source: Royal Proclamation              # required with `dates`
  - name: Juneteenth National Independence Day
    rule: "YY-06-19"
    from: 2022                              # first and last years of a rule
  - name: Matariki
    source: Te Kāhui o Matariki Public Holiday Act 2022, Schedule 1
    dates:                                  # or one entry per year
      2025: "2025-06-20"
      2026: "2026-07-10"
```

**Rules** are tkone date specs giving one date a year (`YY-12-25`,
`YY-11-THU#4`, `YY-MM-EAS-2`, `YY-05-24~PMON` for "the Monday on or before
24 May", `YY-01-29~MON` for "the Monday nearest 29 January"). They may move
off weekends (`~W`, `~NW`, `~PW`) or to a day of the week (`~MON`, `~NMON`,
`~PMON`), but not use business days (`~NB`, `BD#n`): business days are what
the calendar defines. The years a rule applies to are `from` and `until`.

**Substitute days.** A rule's holiday that falls on a weekend moves by the
calendar's `substitute`, or the holiday's own:

| Policy | A weekend holiday's day off | Used by |
|--------|-----------------------------|---------|
| `none` | none: it is lost | TARGET2, Anzac Day in Australia |
| `next-free-weekday` | the next weekday that is not already a day off | UK, Canada, Australia, New Zealand |
| `next-weekday` | the next weekday, even if already a day off | Waitangi and Anzac Day in New Zealand |
| `nearest-weekday` | Saturday to Friday, Sunday to Monday | NYSE |
| `sunday-to-monday` | Sunday to Monday; Saturday is lost | Federal Reserve, South Africa |

Holidays move in date order, so with `next-free-weekday` Christmas on a
Saturday and Boxing Day on a Sunday give Monday and Tuesday. Listed dates are
days off as written, and must not fall on a weekend.

## Rules for the data

The compile tool and the crate's tests reject a file that breaks any of these,
and list every problem at once:

- Unknown fields, YAML anchors and aliases are not allowed. Write dates as
  quoted strings, `"YYYY-MM-DD"`.
- `valid_from` ≤ `confirmed_until` ≤ `valid_until`; the zone is an IANA zone.
- A holiday has a `rule` or `dates`, not both.
- A rule parses, gives at least one date in range and at most four a year,
  and every `except` date is one it gives.
- `dates` and `except` have a `source`. A date is not both excepted and
  listed for the same holiday, nor listed twice.
- Year-keyed `dates` list every year without gaps, each under its own year.

## Updating a calendar

1. When a new year's holidays are published, check them against the file and
   add any one-off dates, with their source.
2. Move `confirmed_until` to the end of the checked year.
3. Run the compile tool and `tools/compare_quantlib.py`. Explain each
   difference from QuantLib in `KNOWN` there, or fix it.

A one-off holiday in the future (a coronation) goes in as soon as it is
proclaimed, even beyond `confirmed_until`.
