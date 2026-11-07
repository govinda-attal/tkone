# Schedule documents — Design

Status: **in progress**. `Schedule`, `Selector` and `ScheduleDoc` (the
`serde` feature of tkone-schedule) are built; moving users over and the
grammar change are not. See [Migration](#migration).

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
| `time_zone` | IANA name | yes | Zone every rule, `start` and `until` is read in. No default: an implicit UTC is a bug waiting to happen. |
| `calendar` | code or `A+B` | when a rule uses business days | Business days for every rule, resolved by a `CalendarResolver`. `WEEKENDS` is Saturday and Sunday only. See [D10](#d10-no-calendar-means-no-business-days). |
| `start` | local date or datetime | yes | Anchor for cadences and the time of day, and the inclusive lower bound. See [D1](#d1-start-is-an-anchor-not-an-occurrence). |
| `until` | local date or datetime | no | Inclusive upper bound. A date means the whole of that day. |
| `count` | integer | no | Stop after this many occurrences, counted after exclusions. With `until`, whichever comes first. |
| `rules` | list | yes, at least one | Each `{ id, rule }`. `id` is optional, unique within the document, and carried on every occurrence the rule produces. |
| `exclude.dates` | list of dates | no | Dates with no occurrences. |
| `exclude.rules` | list of date specs | no | Dates matching any of these have no occurrences. |
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

See [D4](#d4-occurrences-are-unique-by-observed-instant).

---

## Definitions and instances

The same document shape serves both, read in two ways.

- A **definition's** document is a **generator**: its rules produce occurrences.
- An **instance's** document is a **selector**: its rules pick from the
  definition's occurrences. It can never produce an instant the definition
  does not.

### Example: interest accrual

```yaml
# definition: accrual batches, three a business day
version: 1
time_zone: Europe/London
calendar: GBLO
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

| Definition occurrence | acct-1 month end 18:00 | acct-2 daily 09:00 | acct-3 opens 1 Jun | acct-4 month end **17:00** |
|---|---|---|---|---|
| Fri 29 May 09:00 | | ✓ | | |
| Fri 29 May 13:00 | | | | |
| Fri 29 May 18:00 | ✓ (actual 31 May) | | | |
| Mon 1 Jun 09:00 | | ✓ | | |

acct-4 wants 29 May 17:00, which the definition never produces: `uncovered`
reports it at registration (below).

### Selection is lazy

Instance runs are not computed ahead. Each instance holds only its **next
due** instant, computed when its previous run fires (D6). A definition run for occurrence `t`
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
instances `Done`: their slots are reported missed and the result is
`NotFound`, so an instance outliving its definition is visible. The horizon is not optional: acct-4's
17:00 never exists, and an unbounded search would never return.

Pseudo code for tempo:

```rust
// When an instance is registered (`after` = its start), and when each of
// its runs fires (`after` = that run's due), in the same transaction (D6):
match selector.next_after(&definition, after, after + horizon, |m| record_missed(instance, m))? {
    Next::Due(occ) => instance.next_due = Some(occ.at),
    Next::Done     => instance.complete(),
    Next::NotFound => instance.flag("no selected run within horizon"),
}

// When definition occurrence `occ` fires:
let due = instances.where_next_due(defn, occ.at);   // index (defn_id, next_due)
if due.is_empty() {
    record_empty(&occ);                              // D8
} else {
    create_defn_run(&occ, &due);                     // one instance run each,
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

RFC 5545 always makes `DTSTART` the first instance, and so does tkone's
`new_with_start` today. In a document, `start: 2026-01-01` (a holiday) would
then fire on a holiday. A document's `start` anchors cadences (`2W`, `3M`
count from it) and bounds the stream; it is an occurrence only if a rule
produces it. This is what a start written in the spec (`@…`) already does;
schedules reach it without the string.

`start` is required. Lazy selection asks for the slots after any instant
and must get the same answer from each: a cadence, or a time carried from the
start (`1H:MM:SS`), would otherwise take its phase from wherever iteration
began.

### D2. `count` counts after exclusions

"Run 12 times" means 12 runs. This is already what `with_count` does; RFC
5545, which counts excluded dates, differs.

### D3. Exclusions match the actual date by default

That is the current behaviour, and changing the default would silently move
existing schedules. With `~NB`, matching the actual date excludes "the 25th,
moved to the 27th"; `match: observed` excludes whatever lands on the 27th.

### D4. Occurrences are unique by observed instant

Two rules can land on the same instant from different actual dates:
`YY-MM-29T18:00:00` and `YY-MM-L~PBT18:00:00` both give Fri 29 May 2026 at
18:00. Today's union keeps the first part's occurrence and keys on actual
time, which would give a scheduler two runs at the same instant. A document
merges them into one occurrence carrying both sources.

A single rule does this too, today. `YY-MM-DD~NBT09:00:00` around 30 May
2026 yields Saturday's and Sunday's occurrences moved to Monday 1 June 09:00,
then Monday's own: three occurrences at one instant. The iterator is right to
report each actual date; a document must merge them into one occurrence with
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
an error, and so is one in an instance document whose definition has none. Business days are any adjustment but `~NA` and the weekday ones
(`~W`, …), `BD#n` and `nBD`; weekday rules (`WD#n`, `nWD`) need no calendar.
The builder's default today is `WeekendSkipper`, which silently treats every
weekday as a business day; for operations that is a trap. Weekend-only is
written `calendar: WEEKENDS`.

---

## Builder

The document is data for a builder that code can use directly:

```rust
// `start` is a DateTime<chrono_tz::Tz>: it carries the time zone.
let schedule = Schedule::builder(start, calendars.get("XNYS")?)
    .until(until)
    .named_rule("close", "YY-MM-DD~SBT16:30:00")
    .named_rule("month-end", "YY-MM-L~PBT18:00:00")
    .exclude_date(date(2026, 11, 27))
    .exclude_rule("YY-12-24")
    .build()?;

for slot in schedule.slots_after(&last_fired)?.take(3) { /* … */ }

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

Gaps in today's builders this closes:

| Today | Needed |
|---|---|
| `with_exclusions` appends `! spec` to the string | Exclusions held as data, not spliced into the spec |
| Unions only as `A \| B`; `Merge` is crate-private | Several rules with ids, merged by observed instant (D4) |
| `new_with_start` emits the start | An anchored start, as a written `@…` gives (D1) |
| Calendar checked against `[cal=]` in the string | The document resolves the calendar, so there is nothing to mismatch. The calendar zone check stays. |

---

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
| `tkone-trigger` | `BusinessSchedule` takes a `Schedule`; execution options (catch-up) stay builder arguments. |
| Python bindings | Documents passed as dicts or YAML/JSON strings. |
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
2. **Move users over**: tkone-calendar, tkone-trigger, Python, example-app, docs.
3. **Remove** bounds, `!` and `|` from the grammar, ABNF, IETF draft and spec docs.
4. **Tempo on paper**: ERD, schema and sequence diagrams.
5. **Thin tempo prototype**: PostgreSQL definitions, instances and
   occurrences, running the accrual example end to end. This is what tests
   whether steps 1–4 were right.

Nothing is published (all crates are 0.1.0), so step 3 needs no deprecation
period.
