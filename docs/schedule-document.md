# Schedule documents — Design

Status: **in progress**. `Schedule`, `Selector` and `ScheduleDoc` (the
`serde` feature of tkone-schedule) are built, users are moved over, and the
grammar is rule-only. Tempo is next. See [Migration](#migration).
Periods, bounded slots and [timelines](timeline.md) (several schedules read
as one) are built on `Schedule`. [A loan's life](loan-lifecycle.md) puts it
all together, from setup to closure.

## Motivation

The tkone spec string grew from a recurrence rule into a whole schedule:

```text
YY-MM-[TUE,THU]/2WT18:00:00 ! YY-12-25 | YY-12-26 @2025-01-07[Europe/London][cal=GBLO] ..2025-06-30 x10
└──────────── rule ───────┘ └──── exclusions ───┘ └─ start, zone, calendar ─────────┘ └── end ──┘ └count┘
```

Only the first part is the recurrence. The rest is context: where, on whose
business days, from when, until when, how many, and what to leave out. Packing
it into one string made the grammar long, the longest specs unreadable, and
left no place for what a scheduler such as tempo needs next: several named
rules, per-instance selection, and checks between a definition and its
instances.

Every mature standard splits the two:

| Standard | Rule | Document around it |
|---|---|---|
| iCalendar (RFC 5545) | `RRULE` | `VEVENT`: `DTSTART`, `TZID`, `RDATE`, `EXDATE`, `DURATION` |
| JSCalendar (RFC 8984) | `recurrenceRules[]` | `start`, `timeZone`, `duration`, `excludedRecurrenceRules[]` |
| Temporal | cron strings | `ScheduleSpec`: specs, excludes, `start_time`, `end_time`, `timezone` |
| Quartz | `CronExpression` | `Trigger`: `startAt`, `endAt`, calendar, misfire policy |
| Kubernetes | `schedule` | `CronJob`: `timeZone`, `startingDeadlineSeconds`, `concurrencyPolicy` |

tkone does the same: **the spec string is the rule; a schedule document
(YAML or JSON) is everything around it.** Every document field is also a
builder method, so code can do anything a document can.

---

## Layers

| Layer | Owns | Lives in |
|---|---|---|
| **Rule** (string) | The shape of the recurrence: date and time cycles, Easter, business-day adjustment (`~NB`, `~PB`, `~SB`, …) | `tkone-schedule` grammar |
| **Schedule document** (YAML/JSON) and **builder** | Time zone, calendar, start, until, count, several rules, exclusions, options | `tkone-schedule` (`document` module, `serde` feature) |
| **Execution** | Overlap, dependencies, catch-up and misfire, versions, payloads | `tkone-trigger` (builder options), `tkone-tempo` (definition and instance tables) |

The rule says *business day* in the abstract; the document says *whose*. A
schedule says *when*; execution says *what happens if a run is late, still
running, or depends on another*. Execution never goes in the document.

---

## The rule string

What stays: everything that describes one recurrence.

- Date part: years, months, days, weekdays, `L`, `#n`, cycles (`2W`, `8D[1-4]`), `EAS` / `OEAS`, business-day adjustment.
- Time part: hours, minutes, seconds, lists, ranges, steps.
- `T` joining the two.

What leaves the grammar: `@start`, `..end`, `[Zone]`, `[cal=…]`, `xN`, `!`
exclusions and `|` unions. Each becomes a document field and a builder method.

What joins it: moving a date to a day of the week, `~MON` (the nearest
Monday), `~NMON` (the next) and `~PMON` (the previous). Three holiday
calendars used date-level `!` and `|` for the shape of a holiday, not its
context: Victoria Day was `YY-05-MON ! YY-05-[01-17,25-31]`, the Monday
from 18 to 24 May. It is now `YY-05-24~PMON`, which says what the law
says, keeps the 24th as the actual date, and gives the same dates from 1900
to 2299.

The IETF draft keeps covering the rule string only. The document format stays
crate documentation until a real consumer (tempo) has used it.

---

## The schedule document

### Example: a definition

```yaml
version: 1
time_zone: America/New_York
calendar: XNYS
start: 2026-01-02T00:00:00
until: 2026-12-31
rules:
  - id: close
    rule: YY-MM-DD~SBT16:30:00
  - id: month-end
    rule: YY-MM-L~PBT18:00:00
exclude:
  dates: [2026-11-27]
  rules: [YY-12-24]
```

### Fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `version` | integer | yes | Document format version; `1`. |
| `time_zone` | IANA name | yes, in a definition | Zone every rule, `start` and `until` is read in. No default: an implicit UTC is a bug waiting to happen. |
| `calendar` | code or `A+B` | when a rule uses business days | Business days for every rule, resolved by a `CalendarResolver`. `WEEKENDS` is Saturday and Sunday off, with no holidays; it never reaches the resolver. See [D10](#d10-no-calendar-means-no-business-days). |
| `start` | local date or datetime | yes | Anchor for cadences and the time of day, and the inclusive lower bound. See [D1](#d1-start-is-an-anchor-not-an-occurrence). |
| `until` | local date or datetime | no | Inclusive upper bound. A date means the whole of that day. |
| `count` | integer | no | Stop after this many slots, counted after exclusions. With `until`, whichever comes first. |
| `rules` | list | yes, at least one | Each `{ id, rule, basis }`. `id` is optional, unique within the document, and carried on every slot the rule produces. `rule` is a datetime rule, or one [derived](#rules-derived-from-other-rules) from another rule (`{payment}~2PT09:00:00`). `basis: true` marks a rule that derived rules refer to, with no slots of its own; it needs an `id`. |
| `exclude.dates` | list of dates | no | Dates with no slots. |
| `exclude.rules` | list of date specs | no | Dates matching any of these have no slots. |
| `exclude.match` | `actual` / `observed` | no | Which date an exclusion is tested against. Default `actual`. See [D3](#d3-exclusions-match-the-actual-date-by-default). |
| `options.dst_policy` | `adjust` / `strict` | no | As `with_dst_policy`. |
| `options.interval_mode` | `wall-clock` / `elapsed` | no | As `with_interval_mode`. |
| `options.time_anchor` | `start` / `midnight` | no | As `with_time_anchor`. |

Local datetimes carry no offset: the zone is `time_zone`, and an offset could
contradict it. Unknown fields are rejected, so `untill:` fails instead of being
ignored.

Field names follow JSCalendar's concepts in this repository's snake_case
(as the calendar YAML does), not its camelCase spelling.

### Rule kinds

Version 1 takes datetime rules (`YY-MM-DD~SBT09:00:00`): tempo and
tkone-trigger need instants. Date documents (report dates, payment dates)
and time-only rules can follow; all rules in one document would then be the
same kind, since merging dates with instants has no meaning.

### Slots

A schedule yields **slots**. (The crate's `Occurrence` is one rule's result,
`Exact` or adjusted; a slot can come from several.)

```rust
pub struct Slot {
    /// When it happens: the adjusted (observed) instant.
    pub at: DateTime<Tz>,
    /// Every rule that produced this instant, in rule order, with its actual
    /// (unadjusted) time. Usually one.
    pub sources: Vec<Source>,
}

pub struct Source {
    pub rule_id: Option<String>,
    pub actual: DateTime<Tz>,
}
```

See [D4](#d4-slots-are-unique-by-observed-instant).

---

## Definitions and instances

The same document shape serves both, read in two ways.

- A **definition's** document is a **generator**: its rules produce slots.
- An **instance's** document is a **selector**: its rules pick from the
  definition's slots. It can never produce an instant the definition
  does not.

### Example: interest accrual

```yaml
# definition: accrual batches, three a business day
version: 1
time_zone: Europe/London
calendar: GBLO
start: 2026-01-01
rules:
  - id: batch
    rule: YY-MM-DD~SBT[09,13,18]:00:00
```

```yaml
# instance: account 1234 accrues at month end, in the evening batch
version: 1
start: 2026-03-15T00:00:00     # account opened
until: 2027-03-14              # account closed
rules:
  - rule: YY-MM-L~PBT18:00:00
exclude:
  dates: [2026-12-31]
```

An instance document may use `version`, `start`, `until`, `count`, `rules`
and `exclude`. It may not set `time_zone`, `calendar` or `options`: its rules
are evaluated with the definition's. (An instance calendar is
[deferred](#deferred).)

Around May 2026's month end (31 May is a Sunday):

| Definition slot | acct-1 month end 18:00 | acct-2 daily 09:00 | acct-3 opens 1 Jun | acct-4 month end **17:00** |
|---|---|---|---|---|
| Fri 29 May 09:00 | | ✓ | | |
| Fri 29 May 13:00 | | | | |
| Fri 29 May 18:00 | ✓ (actual 31 May) | | | |
| Mon 1 Jun 09:00 | | ✓ | | |

acct-4 wants 29 May 17:00, which the definition never produces: `uncovered`
reports it at registration (below).

### Selection is lazy

Instance runs are not computed ahead. Each instance holds only its **next
due** instant, computed when its previous run fires (D6). A definition run for slot `t`
contains exactly the instances whose next due is `t`: an index lookup, with
no walk over selectors.

```rust
impl<B: BizDayProcessor> Schedule<B> {
    /// A selection from this schedule, anchored at `start`, read with this
    /// schedule's zone, business days and options.
    pub fn selector(&self, start: DateTime<Tz>) -> SelectorBuilder<B>;
}

pub enum Next {
    /// The selector's next slot that the definition produces.
    Due(Slot),
    /// The selector has no more slots: its window has ended.
    Done,
    /// The selector has slots, but none the definition produces before the horizon.
    NotFound,
}

impl<B: BizDayProcessor> Selector<B> {
    /// The first slot strictly after `after`, no later than `horizon`,
    /// that this selector picks and `definition` produces. Slots the
    /// selector wants that the definition does not produce are passed to
    /// `missed`.
    pub fn next_after(
        &self,
        definition: &Schedule<B>,
        after: &DateTime<Tz>,
        horizon: &DateTime<Tz>,
        missed: impl FnMut(Slot),
    ) -> Result<Next>;
}
```

A document reads as either: `ScheduleDoc::schedule(&calendars)` for a
definition, `ScheduleDoc::selector(&definition)` for an instance.

`next_after` merges the selector's slots after `after` with the
definition's and keeps the first instant both have. `Due` carries the
selector's slot: its sources are the instance's rules, with their actual
times (acct-1's 31 May). A definition that has ended does not make its
instances `Done`: their slots are reported missed, and the result is
`NotFound` until the instance's own window ends, so an instance outliving
its definition is visible. The horizon is not optional: acct-4's
17:00 never exists, and an unbounded search would never return.

Pseudo code for tempo:

```rust
// When an instance is registered (`after` = just before its start, since
// `next_after` is strictly after), and when each of its runs fires
// (`after` = that run's due), in the same transaction (D6):
match selector.next_after(&definition, &after, &(after + horizon), |m| record_missed(instance, m))? {
    Next::Due(slot) => instance.next_due = Some(slot.at),
    Next::Done      => instance.complete(),
    Next::NotFound  => instance.flag("no selected run within horizon"),
}

// When the definition's slot `slot` fires (an occurrence, in tempo's tables):
let due = instances.where_next_due(defn, slot.at);  // index (defn_id, next_due)
if due.is_empty() {
    record_empty(&slot);                             // D8
} else {
    create_defn_run(&slot, &due);                    // one instance run each,
}                                                    // plus each one's next due
```

### Checking an instance against its definition

```rust
impl<B: BizDayProcessor> Selector<B> {
    /// The selector's slots in `window` that `definition` does not
    /// produce. Empty means the instance is a subset over that window.
    pub fn uncovered(&self, definition: &Schedule<B>, window: RangeInclusive<DateTime<Tz>>)
        -> Result<Vec<Slot>>;
}
```

A proof for all time is not offered. Pure Gregorian rules repeat every 400
years, but Easter's cycle is 5.7 million years and holiday calendars are lists
known only to `confirmed_until`. Over a bounded horizon the check is exact and
cheap (three batches a business day for a year is about 750 instants).

Tempo runs it when an instance is registered, and against every distinct
instance document when a new definition version is rolled out. Lazy
selection is safe without it — an instance cannot fire outside a definition
run — but it only notices a miss when the miss is due. The check fails at
registration instead: a batch time that does not exist, or a definition
exclusion that removes an account's month end.

---

## Decisions

### D1. `start` is an anchor, not an occurrence

RFC 5545 always makes `DTSTART` the first instance, and tkone's
`new_with_start` used to as well. In a document, `start: 2026-01-01` (a
holiday) would then fire on a holiday. A document's `start` anchors cadences
(`2W`, `3M` count from it) and bounds the stream; it is a slot only if a rule
produces it. A start written in the spec (`@…`) did this before the grammar
became rule-only; schedules now do it without the string.

The iterators' `new_with_start` follows the same rule: the start anchors the
rule and is a result only if the rule gives it, adjusted as any other. The
one difference is the bound: an iterator reports a rule's occurrences, so its
start bounds their *actual* dates, as `new_after` and `resume_after` do; a
schedule's slots are when things happen, so its start bounds the *observed*
instant. Continuing from an occurrence found with `new_after` therefore
passes its `actual()` date, which works whichever way it was adjusted.

A slot is never before the start, so a rule date on or after the start
that an adjustment moves before it is dropped. For a schedule that starts
now that is right: the slot is past. For a fixed term it is a payment that
never happens: a loan drawn on Saturday 31 January with month-end
repayments on `~PB` loses January's, moved to Friday the 30th.
`Schedule::moved_before_start()` lists such slots, so a test or a check
before saving can require none. Refusing them in `build()` was rejected: a
schedule started now, on a weekend, would then fail to build.

`start` is required. Lazy selection asks for the slots after any instant
and must get the same answer from each: a cadence, or a time carried from the
start (`1H:MM:SS`), would otherwise take its phase from wherever iteration
began.

### D2. `count` counts after exclusions

"Run 12 times" means 12 runs. This is already what `with_count` does; RFC
5545, which counts excluded dates, differs.

### D3. Exclusions match the actual date by default

That is what `!` exclusions did, and what the iterators' `with_exclusions`
still does; changing the default would silently move existing schedules. With `~NB`, matching the actual date excludes "the 25th,
moved to the 27th"; `match: observed` excludes whatever lands on the 27th.

### D4. Slots are unique by observed instant

Two rules can land on the same instant from different actual dates:
`YY-MM-29T18:00:00` and `YY-MM-L~PBT18:00:00` both give Fri 29 May 2026 at
18:00. The removed `|` union kept the first part's occurrence and keyed on
actual time, which would give a scheduler two runs at the same instant. A
schedule merges them into one slot carrying both sources.

A single rule does this too. The iterator for `YY-MM-DD~NBT09:00:00` around
30 May 2026 yields Saturday's and Sunday's occurrences moved to Monday 1 June
09:00, then Monday's own: three occurrences at one instant. The iterator is
right to report each actual date; a schedule merges them into one slot with
three sources.

### D5. Selection matches the observed instant

acct-1's actual date is Sunday 31 May; the definition (`~SB`) never produces
Sundays. Matching on actual dates would never fire. Selection and exclusion
therefore match differently (D3), and the API names say which.

### D6. The next due is computed when a run fires

When an instance's run is created, its next due is computed from that run's
due (`after` = the due that fired), without waiting for the run to complete.
The next due therefore depends only on the schedule, never on how long runs
take.

The instance's overlap policy, chosen explicitly when it is defined, only
decides what happens when a due arrives while the previous run is still
going:

| Policy | A due that arrives while the previous run is going |
|---|---|
| `ALLOW` | runs alongside it |
| `BUFFER` | waits (`BUFFERED`), then runs |
| `SKIP` | is recorded as skipped; its next due is computed as for a run |

Requirement on tempo: a definition's occurrences fire in order, and an
instance's next due is written in the same transaction that creates its run.
The definition run for the next occurrence then cannot be created before the
instance is due in it, even when the claimer is behind.

### D7. Instance `count` becomes `until` at registration

When a selector is built, its nth slot becomes `until`, counted over the
selector's own slots, not over those the definition produces: a month end
the definition lacks still counts. The count then stays a property of the schedule,
not of execution: 12 monthly accruals end at the 12th month even if one was
skipped by the overlap policy. A skip is an execution failure that must be
visible as one, not a reason to accrue a 13th month. Instances never carry
`count` at run time.

### D8. A definition occurrence with no due instance creates no run

Most of the accrual batches select nothing. Creating empty runs would flood
`SCHEDULE_DEFN_RUN`; tempo records the occurrence as fired with zero
instances, for observability, and creates no run.

### D9. A definition version change keeps the anchor

`start` anchors cadences; tempo's `effective_from` only cuts the stream of
occurrences between versions. If each version re-anchored, a fortnightly
schedule could change phase on every edit.

### D10. No calendar means no business days

A rule or exclusion using business days in a document without `calendar` is
an error, and so is one in an instance document whose definition has none.
Business days are used by `BD#n`, `nBD` and every adjustment except the
weekday ones (`~W`, `~NW`, `~PW`) and the day-of-the-week ones (`~MON`,
`~NMON`, `~PMON`, …); weekday rules (`WD#n`, `nWD`) need no calendar either.
Without the check, a missing calendar would fall back to `WeekendSkipper`,
which silently treats every weekday as a business day; for operations that
is a trap. Weekend-only is written `calendar: WEEKENDS`.

### D11. Applications own domain dates; tempo owns timing

Dates that belong to a domain object, such as a loan's repayments and its
borrower's reminders, are the application's. The application knows the
terms and the events that change them. Tempo knows when to run things, and
how to retry, overlap and catch up.

- **Set once.** When a loan is set up, the application registers its
  instances with their rules. Tempo computes each next due lazily (D6) and
  keeps going for the life of the loan.
- **Lifetime events are new versions.** A payment holiday, early repayment
  or restructure: the application recomputes the rules and sends a new
  version of the instance document (D12). Tempo recomputes from it.
- **Nothing crosses documents.** Tempo never derives one document's dates
  from another's. A rule may refer only to rules in its own document
  ([derived rules](#rules-derived-from-other-rules)). Anything that
  spans documents is application logic, computed with tkone-schedule
  (`Schedule`, periods, a [timeline](timeline.md)) and registered as rules.

### D12. A new version keeps the stored next due, unless it can't stand

Tempo versions every document. Each instance stores only its next due (D6),
so a new version changes what is computed *after* that due, from the same
anchor (D9). The stored due itself is kept, and fires as computed by the
version that computed it (recorded on the run), except when:

| Case | Example | The stored due |
|---|---|---|
| The new version has ended before it | early repayment: `until` before the due | dropped |
| The calendar closes its day | a holiday declared at short notice | recomputed |
| The new version asks for it (`recompute_pending: true`) | a restructure: repay on the 20th, not the 15th | recomputed |

A lifetime event usually needs the third case. A change that leaves the
dates alone, such as a new rate, needs none.

Calendar data is a version too: a holiday declared at short notice changes
every document on that calendar. Tempo records the calendar data each due
was computed with, which is how it detects the second case.

---

## Builder

The document is data for a builder that code can use directly:

```rust
// `start` is a DateTime<chrono_tz::Tz>: it carries the time zone.
let schedule = Schedule::builder(start, calendars.get("XNYS")?)
    .until(until)
    .named_rule("close", "YY-MM-DD~SBT16:30:00")
    .named_rule("month-end", "YY-MM-L~PBT18:00:00")
    .exclude_date(NaiveDate::from_ymd_opt(2026, 11, 27).unwrap())
    .exclude_rule("YY-12-24")
    .build()?;

let mut slots = schedule.slots_after(&last_fired)?.take(3);   // a FallibleIterator
while let Some(slot) = slots.next()? { /* … */ }

let doc: ScheduleDoc = serde_yaml_ng::from_str(yaml)?;
let schedule = doc.schedule(Calendars::builtin())?;   // resolves the calendar code
```

```rust
/// Turns a document's calendar code into business days. Implemented by
/// tkone-calendar's `Calendars`, and by tempo for its own calendars, so
/// tkone-schedule does not depend on tkone-calendar.
pub trait CalendarResolver {
    type Bdp: BizDayProcessor;
    fn resolve(&self, code: &str) -> Result<Self::Bdp>;
}
```

Gaps in the earlier builders this closed:

| Before | Now |
|---|---|
| `with_exclusions` appended `! spec` to the string | Exclusions held as data, not spliced into the spec |
| Unions only as `A \| B`; `Merge` was crate-private | Several rules with ids, merged by observed instant (D4) |
| `new_with_start` emitted the start as-is | An anchored start, in schedules and iterators alike (D1) |
| Calendar checked against `[cal=]` in the string | The document resolves the calendar, so there is nothing to mismatch. The calendar zone check stays. |

---

## Rules derived from other rules

Status: **built**, in `Schedule` (`rule`, `named_rule`, `basis_rule`) and in
documents (`basis: true`). [A loan's life](loan-lifecycle.md) uses it for
rate fixings and reminders.

### The need

A date is often defined by another date *after* its adjustment: a reminder
two business days before a loan's repayment, a report two business days
before the month-end close. A spec has one adjustment, and the nearest
single spec can give the wrong day:

| Month end | Payment `YY-MM-L~MF` | `YY-MM-L~2P` | 2 business days before the payment |
|---|---|---|---|
| Sun 31 May 2026 | Fri 29 May | Thu 28 May | **Wed 27 May** |

`~2P` counts back from Sunday, not from Friday. On `GBLO` the two differ in
five months of 2026.

In Rust this is one line, `calendar.sub(&slot.at.naive_local(), 2)?`. But
an application registers rules once and leaves tempo to compute every next
due for the life of the loan (D11), so there is no code at run time: the
rule itself has to say "two business days before the repayment".

### Rejected: chained adjustments

`YY-MM-L~MF~2P`, applied left to right.

- **One calendar.** A spec is read on one calendar; a dependent date on
  another calendar can't be written.
- **Duplicated rules.** The dependent rule repeats the payment's
  `YY-MM-L~MF`. Change one to `~MP` and not the other, and the dates drift
  apart silently.
- **Mostly meaningless.** `~NB~PB`, `~SB~2P` and `~MON~MF` would all parse,
  and each would need defined behaviour, tests and documentation. The grammar
  was just made smaller by moving context out of it (bounds, `!`, `|`).

### Rejected: explicit fields

```yaml
- { id: reminder, from: repayment, offset: -2BD, time: "09:00:00" }
```

This is a second, weaker language beside the spec. The offset is a
`~nP` written differently, and the time is a `T…` written differently.

### Rejected: references across documents

`{loan.repayment}~2PT11:00:00`, one document's rule referring to another
document's. Rules would no longer mean anything without the documents they
name; loading, validation and versioning (D12) would have to follow a graph
of documents, one edge per instance; and it would make tempo derive domain
dates, which belong to the application (D11).

### Design: a reference to a rule in the same document

A spec is `date [~adjustment] T time`. The date part can name another rule
of the same document instead of a date pattern:

```yaml
# loan 123's reminder instance, registered once when the loan is set up
rules:
  - id: repayment
    rule: YY-MM-15~MFT12:00:00
    basis: true                          # referred to; no slots of its own
  - id: reminder
    rule: "{repayment}~2PT09:00:00"      # two business days before each repayment
```

- `{repayment}` gives each slot of `repayment` at its adjusted date.
- `~2P` is the derived rule's one adjustment. "One adjustment per spec"
  still holds: the first adjustment is in the rule referred to.
- `T09:00:00` is an ordinary time. `T_:_:_` keeps the referred slot's time,
  as `_` ("as is") keeps the start's today.
- **`basis: true`** marks a rule that other rules refer to and that has no
  slots of its own. An instance selects among one definition's slots, and
  the repayments are another definition's (the collections). The reminder
  instance therefore carries the repayment rule as its basis.

The repayment rule is then written in two documents: the repayment instance
and the reminder instance. Both are written by the application from one loan
record, and replaced together on a lifetime event (D11, D12). The
application owns keeping them the same, as it owns the loan.

A reference within one document keeps everything else as it is:

- The document is still one context: one zone, one calendar, one start.
- It has one version, so a lifetime event changes the basis and the derived
  rule together.
- Validation stays local: references resolve within the document when it is
  built.

### Semantics: two sieves

```text
1. The basis rule, from the document's start:      15 Feb   15 Mar   15 Apr  ...
   adjusted (~MF):                                 16 Feb   16 Mar   15 Apr  ...
2. The derived rule's own adjustment (~2P):        12 Feb   12 Mar   13 Apr  ...
3. The document's exclusions, until and count apply to these slots, as to any.
```

- **The basis decides what exists.** It is read from the document's start
  without `until` or count. A derived slot exists only for a basis slot.
- **The document can only remove.** Its exclusions, `until` and count apply
  to the derived slots. With no `until`, they last as long as the basis.
- **Start.** A reference has no cadence, so for a derived rule the start
  only bounds. A derived slot moved before the start is dropped (D1), and
  `moved_before_start` reports it.
- **Actual and observed.** A derived slot's actual is the basis slot's
  adjusted date, at the derived rule's time. Its observed is that after the
  derived rule's adjustment. Exclusions match the actual by default (D3).
- **Provenance.** A derived slot's source names the basis slot (rule id and
  instant): "the reminder for the 16 March repayment" is an identity, not a
  coincidence of dates.
- **Selection.** A derived slot's observed instant must still be a slot of
  the definition (D5); `uncovered` checks this as for any rule.
- **Adjustments after a reference.** `~nP` and `~nN` (offsets) are the main
  use. `~NB`, `~PB`, `~MF`, `~MP` and `~SB` handle a basis date that isn't a
  business day on the document's calendar. The weekday forms (`~MON`) work as
  anywhere else.
- **A reference is the whole date part.** `{repayment}` can't be combined
  with a date pattern or a week cycle.
- **Not in the rule grammar.** `{id}` is a form of schedules: a rule string
  alone (an iterator, `describe`, the ABNF) never takes one, because it means
  nothing outside its schedule. The grammar is unchanged.

### What was built, and its limits

- **Within one schedule or document.** A reference names a rule of the same
  schedule. Anything that spans documents is the application's (D11).
- **One level.** The referred rule must not itself be derived, so references
  cannot loop.
- **Any time spec, read from the basis slot's time.** After the reference
  come an optional adjustment and `T` with a time spec, read with the basis
  slot's time as an ordinary rule's is read with its start: `09:00:00`,
  `[09,13]:00:00` (two slots a day), `_:_:_` (the basis slot's time),
  `_:30:00` (its hour), `4H:00:00` (in phase with it). The adjustment applies
  once per date; every time on that date shares it and its basis.
- **A basis must be referred to, and needs an `id`.** An unreferred basis is
  refused: it would produce nothing, which is almost always a typo.
- **Reach composes.** `slots_after(t)` reads the basis from far enough back:
  a derived slot can be later than its basis slot by its own forward reach
  (`~2N`), and the basis slot later than its actual date by the basis rule's.
  `until` bounds the search by the derived rule's backward reach (`~2P`).
- **Python** reads documents, so references come with them; `Source.basis`
  is there too.

### The start as a boundary, not a run

A loan drawn on the 15th with repayments on the 15th makes the drawdown a
slot (D1). Two places want opposite things:

| Place | Wants the start to be |
|---|---|
| repayments | not a slot: nothing is repaid at the drawdown |
| the first period's rate fixing, derived from the boundaries | a boundary to derive from |

- **"The start is not a run":** exclude its date, as the repayments do
  in [the loan example](loan-lifecycle.md#3-collections-the-repayments).
- **"The start is a boundary":** a basis rule keeps it. A basis has no
  slots of its own, so including the drawdown is harmless, and
  `{boundary}~2P` derives the first fixing from it.
- **Both in one document:** a derived reminder whose basis includes the
  drawdown excludes the drawdown's *date*, since a derived slot's actual
  is its basis date (D3).

`Schedule::periods()` already treats a slot at the start this way: it
opens the first period instead of ending one. A start-exclusive option
was considered and rejected: it fixes the first place and breaks the
second.

### Open question: the basis is read on its document's calendar

In the reminder instance, the repayment's `~MF` is evaluated on the reminder
definition's calendar. If the collections definition uses another one, the
basis can differ from the real repayments. The application must use the same
calendar, or tempo must check that the basis's calendar matches the
definition it was copied from.

## Deferred

Specified here so the format can grow into them; not built in version 1.

- **Active periods** (`active: PT2H`): each occurrence becomes `[at, at + 2h)`.
  Only useful once tempo's overlap policy exists.
- **Duration end** (`for: P1Y`): duplicates `until`.
- **Extra dates** (iCalendar `RDATE`): a one-off can be a rule with a literal
  date.
- **Moved occurrences** (JSCalendar `recurrenceOverrides`).
- **Instance calendars**: "last GBLO business day" on a `GBLO+USNY`
  definition. The instance calendar would shape selection only; `uncovered`
  still guards it.
- **Per-rule zone or calendar**: a second zone is a second definition.
- **Jitter**: execution, not schedule.

---

## Impact

| Area | Change |
|---|---|
| `tkone-schedule` | Rule-only grammar; `document` module; `Schedule` builder; `Selector` with `next_after`, `uncovered`; `CalendarResolver`. `calendar_of`, `time_zone_of` and the `[cal=]` code check are removed. |
| `tkone-calendar` | `Calendars` implements `CalendarResolver`; `for_spec` is removed. |
| `tkone-trigger` | `BusinessSchedule` takes a `Schedule` and stores the last slot's `at`; execution options (catch-up) stay builder arguments. `time_schedule` reads its zone from `tz` only, and aligns steps to the clock (`TimeAnchor::Midnight`): with no `@start` to anchor them, `HH:15M:00` would otherwise take its phase from when the process started. |
| Python bindings | `Document` from a dict or JSON string, iterated as `Slot`s; `Schedule` is one rule. |
| Grammar, ABNF, IETF draft, spec docs | Bounds, `!` and `\|` removed; conformance examples rewritten. |
| example-app | Use cases rewritten as documents. |
| `tkone-tempo` (docs, schema) | See below. |

Tempo schema changes:

- `SCHEDULE_DEFN.run_spec` holds the schedule document (or a new `schedule` column beside the payload).
- `SCHEDULE_INSTANCE.run_spec`'s "overrides" becomes the instance document (selector and window); `count` stored as `until` (D7).
- `SCHEDULE_INSTANCE` gains `next_due_dtm`, indexed with `defn_id`; null once the instance is done (D6).
- `SCHEDULE_OCCURRENCE` gains `sources` (rule ids and actual times, D4); `actual_dtm` stays as the first source's.
- `SCHEDULE_DEFN_RUN.instance_count` is the number of instances due; occurrences with none create no run (D8).
- A new definition version is checked with `uncovered` against every distinct selector before rollout.
- `CALENDAR` rows are resolved through `CalendarResolver`, after tkone-calendar's built-in codes.

---

## Migration

Each step leaves the workspace building and tested.

1. **Build the document** in tkone-schedule, beside the existing syntax:
   `Schedule` (anchored rules with ids, merged into slots, exclusions as
   data, until, count), `ScheduleDoc`, `CalendarResolver` (implemented by
   tkone-calendar's `Calendars`), `Selector` with `next_after` and
   `uncovered`, `ScheduleDoc::selector` — done.
2. **Move users over**: tkone-calendar, tkone-trigger, Python, example-app,
   docs — done.
3. **Remove** bounds, `!` and `|` from the grammar, ABNF, IETF draft and spec
   docs; add `~DAY` for the holiday calendars that needed `!` and `|` —
   done. The iterator builders keep `with_end`, `with_count` and
   `with_exclusions` (one spec per call) for a single rule.
4. **Tempo on paper**: ERD, schema and sequence diagrams.
5. **Thin tempo prototype**: PostgreSQL definitions, instances and
   occurrences, running the accrual example end to end. This is what tests
   whether steps 1–4 were right.

Nothing is published (all crates are 0.1.0), so step 3 needs no deprecation
period.
