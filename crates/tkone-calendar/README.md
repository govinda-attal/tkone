# tkone-calendar

Holiday calendars for [tkone-schedule](../tkone-schedule/README.md). A calendar
knows which dates are business days in one place, and plugs into any schedule
as its business-day processor:

```rust
use tkone_calendar::Calendars;
use tkone_schedule::date::SpecIteratorBuilder;

let london = Calendars::builtin().get("GBLO")?;
let payments = SpecIteratorBuilder::new_with_start("YY-1M-31L~MF", london, start).build()?;

// Business days in both London and TARGET2.
let both = Calendars::builtin().get("GBLO+EUTA")?;

// A schedule that names its calendar: the text alone fixes its dates.
let spec = "YY-MM-BD#3 ~MF @2026-01-01 [cal=GBLO+USNY] x12";
let cal = Calendars::builtin().for_spec(spec)?;
let iter = SpecIteratorBuilder::from_spec(spec, cal)?.build()?;
```

A schedule that names a calendar (`[cal=GBLO]`) only builds with that
calendar; given any other, building fails rather than running on the wrong
days.

Runnable use cases, in [`example-app`](../example-app/src/calendar/):

| Program | Industry | Shows |
|---------|----------|-------|
| `cargo run -p example-app --bin loan-repayments` | Loan servicing | collections on a 25-year loan, Modified Following on London + New York; due vs paid dates |
| `cargo run -p example-app --bin exchange-jobs` | Capital markets | 16:30 jobs on NYSE trading days, month-end report; the wrong calendar refused |
| `cargo run -p example-app --bin payroll` | HR | one fortnightly template run with Sydney's and Auckland's calendars |
| `cargo run -p example-app --bin store-deliveries` | Retail | deliveries dropped on Johannesburg holidays, including a declared election day |

## Calendars

| Code | Calendar | Covers | Confirmed until | Checked against |
|------|----------|--------|-----------------|-----------------|
| `GBLO` | London: England and Wales bank holidays (and the LSE) | 1990–2099 | 2028 | gov.uk, London Gazette |
| `EUTA` | T2 (TARGET2), euro payments | 1999–2099 | 2026 | ECB |
| `USNY` | New York: Federal Reserve holidays | 1990–2099 | 2030 | Federal Reserve |
| `XNYS` | New York Stock Exchange | 1990–2099 | 2028 | NYSE, ICE |
| `CATO` | Toronto (bank holidays) | 1990–2099 | 2026 | Payments Canada, Bills of Exchange Act, Ontario |
| `AUSY` | Sydney (NSW bank holidays) | 1990–2099 | 2027 | NSW Government |
| `NZAU` / `NZWE` | Auckland / Wellington | 1990–2052 | 2027 | employment.govt.nz, Matariki Act |
| `ZAJO` | Johannesburg | 1995–2099 | 2026 | Government Gazette, Presidency |

Codes are FpML / ISDA business centres, or the exchange's MIC. Every calendar
here is made of **rules** (fixed dates, nth weekdays, Easter) and **listed
dates**, each with the official notice that declared it (coronations, state
funerals, election days). Every calendar is checked against its publisher's
own lists up to its "confirmed until" year, and matches
[QuantLib](https://www.quantlib.org)'s calendar day for day from 1990 to 2075
except where `tools/compare_quantlib.py` names the official holiday QuantLib
lacks (South African declared days, NSW's 2026–2027 Anzac Day additional
days).

## What a calendar promises

- **A date outside its range is an error**, never a guess.
- **After `confirmed_until`, dates are projected:** the rules still apply, but
  a one-off holiday not yet announced (a state funeral, a jubilee) cannot be
  known. `Calendar::status` says which.
- **Each holiday keeps its actual date and its day off:** Christmas 2022 fell
  on a Sunday; in London its day off was Tuesday 27 December.

Calendars whose holidays are set each year by decree, or follow a lunar
calendar (China, Japan, India, the Gulf), are not here yet.

## Data

The calendars are YAML files in [`data/`](data/), described in
[`data/README.md`](data/README.md), compiled into `compiled/calendars.json`,
which the crate embeds. With the `yaml` feature, `Calendars::from_dir` loads
your own files at runtime.
