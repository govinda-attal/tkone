# Contract engine — Advanced example: an offset mortgage

Status: **architecture only**. This walks one product from nothing to a
customer's account, end to end, in the order the work is done. The concepts
are explained in the [engine](contract-engine.md), [models](contract-models.md),
[hooks](contract-hooks.md) and [hot instances](hot-accounts.md) documents; here
they are only used, with a link where each first appears. The sketches are
illustrative, not APIs.

The product: a **tracker mortgage with offset**. Interest is charged daily at
the Bank of England base rate plus a margin, on the mortgage balance less the
balances of up to five linked savings accounts. Repayments fall due monthly;
up to 10% of the balance may be overpaid each year without a charge; up to six
months of payment holidays may be taken. At the end of the tracker period
the customer switches product, or reverts to a standard rate.

It is chosen because it is hard: it reads other accounts, it depends on
reference data that is published late, and its rules change over 25 years.

## Who does what

| Step | Who | Produces | Through |
|---|---|---|---|
| [1. Model](#1-model-lending) | modelling team | the `lending` model crate | `tkone model plan`, `apply` |
| [2. Reference data](#2-reference-data) | treasury, finance, operations | base rate series, accounting mapping, identifier scheme | `tkone reference load`, the API |
| [3. Interface](#3-interface-loan-account) | product architecture | `loan-account@1` | registration |
| [4. Definitions](#4-definitions) | product owners | `offset-tracker@1`, `offset-saver@1`, schedules | registration |
| [5. Implementation](#5-implementation) | implementers | a WASM component and its manifest | `cargo component build` |
| [6. Simulate, publish, promote](#6-simulate-publish-promote) | implementers, product, risk | a signed artifact, `live` and default | `tkone simulate`, `contract publish`, `promote` |
| [7. Open an account](#7-open-an-account) | a channel, an adviser | an instance | the engine API |
| [8. A month in its life](#8-a-month-in-its-life) | tempo, customers, the engine | bookings, events | — |
| [9. Late facts](#9-late-facts) | the engine | corrections | — |
| [10. Change over years](#10-change-over-years) | product, operations | new versions, upgrades | `promote`, `upgrade` |

The platform team has already built the engine and the ledger extension, and
the `core` and `deposits` models are applied.

---

## 1. Model: `lending`

The mortgage is a `core::Account` with a facet; the model adds its balance
types, its own actions, the offset link and its views.

```rust
#[model(name = "lending", version = "1.0.0", requires = "core@^1.5")]
mod lending {
    /// Balance types a loan uses, beside core's.
    #[ledger::balances]
    enum LoanBalance {
        Principal,
        #[ledger::balance(scale = 12)] Accrued,     // interest, kept finer than pennies (models 5)
        Due,                                // instalments fallen due, unpaid
        Fees,
        Incoming,                           // delivered credits land here; `credited` allocates them
    }

    /// The loan's terms: a facet of the core account (C15).
    #[extends(core::Account, when = "product_line = 'mortgages'")]
    struct LoanTerms {
        #[effective] term_end: NaiveDate,
        #[effective] repayment_day: u8,
        #[effective] repayment: RepaymentType,          // Repayment | InterestOnly
    }

    /// What secures it: a facet of its own, so other loans need not have one.
    #[extends(core::Account, when = "product_line = 'mortgages'")]
    struct Security { property: PropertyRef, #[effective] valuation: Money }

    /// Savings accounts whose balances offset this mortgage's interest.
    /// A savings account offsets at most one mortgage: a claim, like a unique
    /// identifier, made before the commit.
    #[relation(Account as mortgage <-> Account as offset,
               max_per_mortgage = 5, unique = "offset")]          // models 6
    #[action(create = "LinkOffset", end = "UnlinkOffset")]
    #[invariant("input.offset.status == 'Active'", reject = "offset-not-active")]
    #[invariant("input.offset.holders.all(h, h in state.borrowers)", reject = "not-the-borrowers")]
    struct OffsetLink { #[effective] since: NaiveDate }

    /// A payment holiday: starts and ends; six months in a mortgage's life.
    #[effective(key = account_id)]
    #[action(create = "StartPaymentHoliday", end = "EndPaymentHoliday")]
    #[invariant("months >= 1 && months <= 6")]
    #[invariant("state.holiday_months_taken + months <= 6", reject = "holiday-allowance-used")]
    struct PaymentHoliday { account_id: AccountId, from: NaiveDate, months: u8 }

    // Views: declared forms, no code (C26).
    #[view(key = account_id, consistency = "with-instance", from = core::Entry,
           group_by = address, fold = sum(signed(amount)))]
    struct LoanBalances { account_id: AccountId, address: BalanceAddress, amount: Money }

    #[view(key = account_id, consistency = "with-instance",
           from = core::Entry, filter = "code == 'LOAN_OVERPAYMENT' && value_date >= state.year_start",
           fold = sum(amount))]
    struct OverpaidThisYear { account_id: AccountId, amount: Money }

    #[view(key = account_id, consistency = "with-instance",
           from = PaymentHoliday, fold = sum(months))]
    struct HolidayMonthsTaken { account_id: AccountId, months: u8 }

    #[view(key = account_id, consistency = "with-instance",
           compose = [core::AccountStatus, LoanBalances, OverpaidThisYear, HolidayMonthsTaken],
           derive = { outstanding: "LoanBalances[Principal] + LoanBalances[Due] + LoanBalances[Fees]" })]
    struct LoanState;

    /// Re-key: from a savings account to the mortgage it offsets.
    #[view(key = offset_id, consistency = "eventual", from = OffsetLink)]
    struct OffsetOf { offset_id: AccountId, mortgage_id: AccountId }
}
```

```text
$ tkone model export lending        # writes model.lock, publishes the schema by digest
$ tkone model plan lending@1.0.0
lending (new) 1.0.0                        requires core@^1.5 (applied: 1.5.0) ✔
  + balances   Principal, Accrued (scale 12), Due, Fees, Incoming
  + facets     LoanTerms, Security on core::Account
  + relation   OffsetLink (max 5 per mortgage; offset unique; claims table)
  + entity     PaymentHoliday
  + actions    LinkOffset, UnlinkOffset, StartPaymentHoliday, EndPaymentHoliday
  + views      LoanBalances, OverpaidThisYear, HolidayMonthsTaken, LoanState (with-instance);
               OffsetOf (eventual)
  postgres     5 tables, 1 unique index     scylla  5 tables, 1 claims table
$ tkone model apply lending@1.0.0   # maker; a checker approves
```

**What the model does not contain:** interest calculation, instalment
amounts, the overpayment allowance. Those are product rules, in the
implementation. The model holds what every lending product means the same
way: balances, terms, links, holidays, and what keeps them valid.

## 2. Reference data

Values with owners outside the product team ([values](contract-engine.md#values-constants-code-sets-parameters-and-reference-data)):

```yaml
# reference/boe-base.yaml: a series, restated in place if ever corrected
series: boe-base
points:
  - { on: 2026-08-07, rate: "0.0400" }
  - { on: 2026-11-06, rate: "0.0375" }

# reference/accounting-mortgages-uk.yaml: logical names to ledger accounts
mapping: mortgages-uk
legal_entity: bank-uk
accounts:
  mortgage_interest_income: "4100-200"
  mortgage_fee_income:      "4300-200"
  mortgage_drawdown:        "1900-010"     # funds paid to the conveyancer
  erc_income:               "4300-250"

# reference/identifier-schemes.yaml (excerpt)
- scheme: mortgage-number
  applies_to: Account
  format: digits(10, mod11)
  unique: global
  per_object: one
  issued_by: bank
```

```text
$ tkone reference load reference/*.yaml     # same API as operations use; maker and checker
```

When the Bank changes base rate, treasury adds a point through the API. No
product version changes; every tracker's next accrual reads it.

## 3. Interface: `loan-account`

What channels can do with any loan, whatever its product
([operation schemas](contract-engine.md#operation-schemas)):

```rust
#[interface(name = "loan-account", version = "1.0.0", model = "lending@^1")]
mod loan_account {
    #[operation]
    struct LinkOffset { offset: AccountLocator }       // "iban:GB…", "acc_…": resolved by the engine
    #[operation]
    struct UnlinkOffset { offset: AccountLocator }
    #[operation(returns = { months_taken: LoanState.holiday_months_taken })]
    struct TakePaymentHoliday { from: NaiveDate, months: u8 }
    #[operation]
    struct Redeem { on: NaiveDate }

    #[query(returns = RedemptionQuote)]
    struct QuoteRedemption { on: NaiveDate }
    struct RedemptionQuote { principal: Money, interest: Money, erc: Money, total: Money }

    #[rejections(LinkOffset)]
    enum LinkRefused { OffsetNotActive, NotTheBorrowers, AlreadyOffsetting, TooManyOffsets }
    #[rejections(TakePaymentHoliday)]
    enum HolidayRefused { HolidayAllowanceUsed, InArrears }

    #[event] struct InstalmentDue { amount: Money, due_on: NaiveDate }
    #[event] struct PaymentAllocated { to_due: Money, overpaid: Money, charge: Money }
    #[event] struct TrackerPeriodEnded { reverts_to: Rate }
}
```

## 4. Definitions

### The mortgage

```rust
#[definition(name = "offset-tracker", version = "1.0.0", model = "lending@^1",
             implements = ["loan-account@^1"])]
#[identifiers(issue = ["mortgage-number"])]
#[ledger(guard = "debits",                // credits (repayments) arrive delivered; never refused
         credits_to = "Incoming")]         //   and land in Incoming, for `credited` to allocate
mod offset_tracker {
    approval_roles! {
        /// Approves a margin renegotiated by more than 0.25%.
        pricing_senior,
    }

    #[parameter(kind = "commercial",                       // set at offer; renegotiated
                levels(instance(required,                  //   with a senior's approval
                       approve(when = "abs(new - old) > 0.0025", by = pricing_senior))))]
    #[range(min = "0", max = "0.05")]
    const MARGIN: Rate;
    #[parameter(kind = "commercial", levels(instance(required)))]
    #[invariant("value > account.opened_on")]
    const TRACKER_END: NaiveDate;
    #[parameter(kind = "commercial", levels(implementation(default = "series:boe-base")))]
    const BASE: Ref<RateSeries>;                            // the rate it tracks
    #[parameter(kind = "commercial", levels(definition(default = "0.10")))]
    #[range(min = "0", max = "0.25")]
    const OVERPAYMENT_ALLOWANCE: Rate;                      // of the balance at the year's start
    #[parameter(kind = "commercial", levels(definition(default = r#"{ bands: [
        { until_year: 1, rate: "0.02" }, { until_year: 2, rate: "0.01" } ] }"#)))]
    const ERC: ErcSchedule;                                 // early repayment charge, by year
    #[parameter(kind = "operational", levels(implementation(default = "mortgages-uk")))]
    const ACCOUNTS: AccountingMapping;
    #[parameter(kind = "operational", levels(implementation(default = "HalfUp")))]
    const ROUNDING: Rounding;

    #[hook(activation)]               fn open;          // drawdown
    #[hook(scheduled)]                fn scheduled;     // accrue, instalment, anniversary, milestone
    #[hook(delivered(ledger::Entry))] fn credited;      // a credit landed: repayments, overpayments
    #[hook(correction, permits = [Book, Reverse])] fn correct;
    #[hook(upgrade)]                  fn upgrade;       // product switch
    #[hook(closure)]                  fn close;         // redemption

    permits! {
        Book, Reverse,
        LinkOffset, UnlinkOffset, StartPaymentHoliday, EndPaymentHoliday,
        Transition(Pending -> Active, Active -> Closed),
        ScheduleChange,
    }

    schedules! {
        time_zone: "Europe/London",
        calendar: "GBLO",
        start: 2026-01-01,
        /// Interest, each business evening.
        accrue:      "YY-MM-DD~SBT18:00:00",
        /// Instalments fall due in the morning; each mortgage selects its day.
        instalment:  "YY-MM-DD~SBT09:00:00",
        /// The overpayment allowance resets on the opening date each year.
        anniversary: "YY-MM-DD~SBT09:00:00",
        /// One-off dates: the tracker period's end.
        milestone:   "YY-MM-DD~SBT09:00:00",
    }
}
```

Its operations come from the interface (`link_offset`, `take_payment_holiday`,
`redeem`, the query `quote_redemption`), each handled by one hook named after
it.

### The offset savings account

```rust
#[definition(name = "offset-saver", version = "1.0.0", model = "deposits@^3.1",
             implements = ["deposit-account@^1"])]
#[ledger(guard = "debits")]
mod offset_saver {
    // Pays no interest: its balance reduces the linked mortgage's.
    #[hook(activation)] fn open;
    #[hook(closure)]    fn close;
    permits! { Book, Reverse, Hold, Release, Transition(Active -> Closed) }
}
```

The savings product knows nothing about mortgages. The mortgage reads it.

### Schedules

Each named rule in `schedules!` is a tempo definition shared by every offset
tracker, checked when the crate compiles; registering the definition hands
tempo its documents ([schedules](contract-engine.md#schedules)). The SDK
generates `Rule` (`Accrue`, `Instalment`, `Anniversary`, `Milestone`) for
`run.is_for`, and `Schedules`, from which each mortgage selects with its own
dates when it opens (step 7).

## 5. Implementation

### The code

Each hook says what it needs with `#[requires]`: the engine fetches exactly
that before the call, so the code does no I/O. Every hook takes the
context the engine prepared (`ctx`, with fields generated from
`#[requires]`) and the message that started it. `cargo component build` writes the manifest
the engine reads from these attributes
([implementations](contract-engine.md#implementations)).

The `linked` input on `scheduled` is the only place this product reads
another account, and it declares exactly how: which relation, which field,
as of when, how settled, how many.

```rust
use tkone_contract_sdk::prelude::*;
use offset_tracker::{Action, Event, *};                     // generated

#[implementation(of = "offset-tracker@1.0.0", values(ROUNDING = "HalfUp"))]
impl offset_tracker::Contract for OffsetTracker {
    #[requires(
        view(LoanState),
        ledger::balances(as_of = due),
        linked(via = OffsetLink, read = ledger::balances.available, as_of = due,
               settled, max = 5, as = offsets),   // other accounts: read-only, complete to the due (C5)
        series(BASE, as_of = due),
        effective(PaymentHoliday, active_at = due, as = holidays),
    )]
    fn scheduled(ctx: &scheduled::Ctx, run: Run) -> Outcome {
        let p = &ctx.params;
        let mut out = Outcome::new();

        if run.is_for(Rule::Accrue) {
            let offset: Money = ctx.linked.offsets.iter().map(|a| a.available).sum();
            let principal = ctx.ledger.balances.principal;
            let charged_on = (principal - offset).max(Money::zero(principal.currency()));
            let rate = ctx.series.base + p.MARGIN;
            // every calendar day since the last accrual: a Friday run covers the weekend
            for day in run.days_since_last() {
                let daily = charged_on * rate.daily(DayCount::Act365Fixed);
                out = out.book(booking! {
                    code: BankTransactionCode::LOAN_INTEREST_ACCRUAL,
                    value_date: day,
                    debit:  (This, Accrued, daily),
                    credit: ("mortgage_interest_income", daily),
                });
            }
        }

        if run.is_for(Rule::Instalment) && ctx.effective.holidays.is_empty() {
            let interest = ctx.ledger.balances.accrued.round(p.ROUNDING);
            let capital = instalment_capital(&ctx.views.loan_state, p, interest);   // annuity, product code
            out = out
                .book(booking! {                     // interest and capital fall due
                    code: BankTransactionCode::LOAN_INSTALMENT,
                    debit:  (This, Due, interest + capital),
                    credit: (This, Accrued, interest),
                    credit: (This, Principal, capital),
                })
                .emit(Event::InstalmentDue { amount: interest + capital, due_on: run.due().date() });
        }

        if run.is_for(Rule::Milestone) {             // the tracker period ends
            out = out.emit(Event::TrackerPeriodEnded { reverts_to: standard_variable(ctx) });
        }
        out
    }

    /// Money arrived: a direct debit, a transfer, a lump sum. Never refused:
    /// allocated to what is due first, then overpayment, with a charge beyond
    /// the allowance in the tracker period.
    #[requires(view(LoanState))]
    fn credited(ctx: &credited::Ctx, c: Delivery<ledger::Entry>) -> Outcome {
        let (p, s) = (&ctx.params, &ctx.views.loan_state);
        let to_due = c.amount.min(s.due);
        let over = c.amount - to_due;
        let allowance = (s.balance_at_year_start * p.OVERPAYMENT_ALLOWANCE).round(p.ROUNDING);
        let free = (allowance - s.overpaid_this_year).max(Money::zero(c.amount.currency()));
        let charged = (over - free).max(Money::zero(c.amount.currency()));
        let rate = p.ERC.rate_for(s.loan_year(c.value_date));   // zero after the last band
        let erc = (charged * rate).round(p.ROUNDING);

        // from Incoming: to Due first, the rest to Principal
        let mut out = Outcome::new().book(allocation(c, to_due, over));
        if erc.is_positive() {
            out = out.book(booking! {
                code: BankTransactionCode::LOAN_FEE,
                debit:  (This, Fees, erc),
                credit: ("erc_income", erc),
            });
        }
        out.emit(Event::PaymentAllocated { to_due, overpaid: over, charge: erc })
    }

    #[requires(view(LoanState),
               instance(from = op.offset, read = [status, holders]))]   // resolved locator; eventual
    fn link_offset(ctx: &link_offset::Ctx, op: LinkOffset) -> Result<Outcome, LinkRefused> {
        // The model's invariants check status and holders, and the claim makes
        // the offset unique; the hook decides only whether the product allows it.
        Ok(Outcome::new().act(Action::LinkOffset { offset: ctx.instances.offset.id,
                                                   since: ctx.time.due.date() }))
    }

    #[requires(view(LoanState), series(BASE, as_of = facts.earliest),
               ledger::entries(since = facts.earliest, until = due, max = 5000))]
    fn correct(ctx: &correct::Ctx, facts: MovedFacts) -> Outcome {
        // Recompute the accruals the moved facts touch; book the difference.
        let recomputed = reaccrue(&facts, ctx);
        Outcome::new().book(adjustment(recomputed - ctx.ledger.entries.accrued_on(&facts.days)))
    }

    // take_payment_holiday, redeem, open, upgrade, close …
    // quote_redemption: #[requires(view(LoanState), series(BASE, as_of = q.on))]
}
```

```text
$ cargo component build --release        # wasm32-wasip2, a component of the offset-tracker WIT world
```

## 6. Simulate, publish, promote

```rust
#[test]
fn offset_halves_the_interest() {
    let mut sim = Simulation::new()
        .model("lending@1.0.0").definition("offset-tracker@1.0.0")
        .component("target/wasm32-wasip2/release/offset_tracker.wasm")
        .reference("reference/*.yaml");

    let saver = sim.open("offset-saver@1", opening! { deposit: gbp(100_000) });
    let loan  = sim.open("offset-tracker@1", opening! {
        principal: gbp(200_000), margin: "0.0075", term_years: 25, repayment_day: 15,
        tracker_end: date(2028, 12, 1),
    });
    sim.call(loan, LinkOffset { offset: saver.locator() }).unwrap();

    sim.run_until(date(2026, 12, 31));                       // tempo's timeline, real dues
    assert_eq!(sim.view::<LoanBalances>(loan).accrued_between(date(2026, 12, 1), date(2026, 12, 31)),
               interest_on(gbp(100_000), "0.0450", 31));     // charged on 200k − 100k
}
```

```text
$ tkone simulate offset-tracker@1.0.0 --years 25 --scenarios scenarios/*.yaml
$ tkone contract publish oci://registry.bank/contracts/offset-tracker:1.0.0 --sign
  sha256:7c1e…  signed; registered as draft; checks: WIT matches offset-tracker@1.0.0 ✔,
  requires bounded ✔, permits within the definition ✔, lending@^1 applied ✔, ledger@^1 present ✔
$ tkone contract promote sha256:7c1e… live --default      # product and risk approve (C7)
```

## 7. Open an account

```text
POST /instances
{ "definition": "offset-tracker@^1",
  "parties": [ { "party": "pty_01JA…", "role": "borrower" },
               { "party": "pty_01JB…", "role": "borrower" } ],
  "terms": { "principal": { "minor": "20000000", "currency": "GBP" }, "term_end": "2051-12-15",
             "repayment_day": 15, "repayment": "Repayment", "property": "prop_01JC…" },
  "parameters": { "MARGIN": "0.0075", "TRACKER_END": "2028-12-01" } }
```

The engine, in one commit on the new instance:

1. **Binds** it to the default implementation, `sha256:7c1e…`.
2. **Issues** its identifiers: a mortgage number, allocated from the bank's
   range ([models 9](contract-models.md#9-identities-are-typed-and-survive-merges)).
3. **Runs `open`**, which books the drawdown: debit `Principal` £200,000,
   credit `mortgage_drawdown`.
4. **Registers the instance's tempo set** (D11, D13), which `open` returned
   as a `ScheduleChange`, selecting from the definitions with its own dates:

```rust
// in `open`
let t = &o.terms;
let schedules = Schedules::new()
    .accrue(Select::between(o.opened_on, t.term_end))
    .instalment(Select::between(o.opened_on.next_month_on(t.repayment_day), t.term_end)
        .rule(rule!("YY-MM-{day:02}~NBT09:00:00", day = t.repayment_day)))
    .anniversary(Select::yearly_from(o.opened_on.add_years(1)))     // YY-12-01~NBT09:00:00
    .milestone(Select::once_on(p.TRACKER_END));
```

   `rule!` checks its template against the grammar when the crate compiles;
   the day is filled in at run time, and tempo checks at commit that every
   selection falls on slots its definition produces.

Then `link_offset` with the customer's savings account, by its IBAN.

## 8. A month in its life

| When | What happens | Instance | Home, deliveries |
|---|---|---|---|
| Before December | the saver holds £100,000, linked as an offset | — | — |
| Thu 3 Dec, 12:10 | £5,000 transferred from the customer's current account to the offset saver | current account (operation `transfer`) | home: current account; credit delivered to the saver |
| Thu 3 Dec, 18:00 | **accrue** due. The engine fetches `LoanState`, base rate as of the due, and the saver's available balance **settled to 18:00**: it waits until the saver's journal and inbox have passed 18:00 | mortgage (`scheduled`) | home: mortgage. Accrual on 200,000 − 105,000 |
| Fri 4 Dec, 18:00 | accrue, for Friday, Saturday and Sunday | mortgage | three accrual bookings |
| Tue 15 Dec, 09:00 | **instalment** due: accrued interest rounded, capital computed; `InstalmentDue` emitted | mortgage | home: mortgage |
| Tue 15 Dec, 11:00 | the bank collects the direct debit from the current account | current account (operation `collect`) | home: current account (guarded debit, checked); credit delivered to the mortgage |
| Tue 15 Dec, 11:00+ | the credit lands in `Incoming`; **`credited`** allocates it to `Due` | mortgage (`credited`) | home: mortgage |
| Fri 18 Dec | the customer overpays £30,000; £20,000 is within the allowance, £10,000 is charged 2% | mortgage (`credited`) | allocation, ERC fee; `PaymentAllocated` |
| Each commit | action events (`ledger.booking-committed`, …) to the outbox; statements, GL and monitoring read them | — | — |

No commit in that month touches two instances' journals. The transfer is
homed on the current account, the accrual on the mortgage; the mortgage
*reads* the saver ([hot instances](hot-accounts.md)).

If a collection fails because the current account has insufficient funds,
`Due` stays unpaid; the mortgage's next `scheduled` run sees it in
`LoanState` and can raise an arrears fee or event, as the product decides.

## 9. Late facts

Three kinds arrive after the results they change, and each reaches a
`correction` hook (C14), without anything re-running:

| Late fact | Example | Why the mortgage hears of it |
|---|---|---|
| On the mortgage itself | a repayment returned by the payer's bank, value 15 Dec | its own journal |
| On reference data | the base rate change of 6 Nov is entered on 10 Nov | it read `boe-base` as of dues after 6 Nov |
| **On an account it read** | a branch books a deposit to the offset saver on 9 Dec with value 3 Dec | it read the saver's balance as of 3–8 Dec |

The third is the one that crosses instances. The engine knows the mortgage
depends on the saver, because the mortgage **declared the read**
(`linked` in `requires`) and the engine recorded what each call read, with
versions (C3, C17). When a fact lands on the saver before a point the
mortgage has read, the engine calls the mortgage's `correction` hook with the
moved facts. The saver's contract does nothing and knows nothing.

```text
correction  acc_01JD… (offset-tracker)   because acc_01JE… (offset-saver) moved:
  fact      deposit £2,000, value 2026-12-03, booked 2026-12-09
  reads     accrue 03-Dec … 08-Dec read acc_01JE… at versions 41 … 47
  outcome   book: interest adjustment −£1.48 (6 days on £2,000 at 4.50%)
```

## 10. Change over years

| Change | How | Who approves |
|---|---|---|
| Base rate moves | a new point in `boe-base` | treasury |
| A customer's margin is renegotiated | `SetParameter MARGIN`, effective from a date | operations, maker and checker |
| A payment holiday | `take_payment_holiday`: the model's invariant checks the six-month allowance; the hook returns `StartPaymentHoliday` and a `ScheduleChange` whose instalment selection excludes those months (D12) | the customer, through a channel |
| A bug fix | `offset-tracker` implementation 1.0.1, published and promoted; new accounts get it; existing ones move by `tkone contract upgrade --from sha256:7c1e… --to sha256:91ab…` (C9) | product, risk |
| A new rule (offset capped at 80% of the balance) | definition `offset-tracker@1.1.0` adds `OFFSET_CAP`; implementation 1.1.0; upgrade in bulk, each account's `upgrade` hook sets its value | product, risk |
| The tracker period ends | `TrackerPeriodEnded` reaches the channel; the customer picks a new product; `upgrade` to its definition's implementation, which rebinds and replaces the tempo set | customer, adviser |
| Lending model 1.1 adds `#[personal] broker_notes` | `model plan` shows growth only; implementations built for 1.0 keep their 1.0 inputs (models 3) | modelling team |
| Redemption | `quote_redemption` for the figure; `redeem` books the final balance, the ERC if any, and `Transition(Active -> Closed)`; the tempo set ends; offsets are unlinked | the customer |

---

## Across instances: what this product needs, and what it does not

An invariant (a CEL rule on a declared action, or the ledger's checks) reads
**one instance**. That limits where rules live, not what a product can
compute: interest, allowances, charges and allocation are hook code, in
Rust, with every input its `#[requires]` declares. The question is only how a
rule that involves several accounts is kept. This product has four such
rules, and each has a home:

| Rule | Kind | How it is kept |
|---|---|---|
| Interest is charged on the balance less the offsets | a **read** across instances | a declared, settled input (`linked`, `settled: true`) |
| A deposit to the saver changes interest already charged | a **dependency** | the engine records reads; a late fact on what was read calls the reader's `correction` hook |
| A savings account offsets at most one mortgage | **uniqueness** across instances | a claim before the commit (`unique = "offset"`), as for identifiers |
| The offset's holders are the borrowers | a **check against another instance**, as of now | an invariant over a resolved input (`input.offset.holders`), read eventually; re-checked by the engine on the next scheduled run, which unlinks if holders change |

None needs a commit across instances. The tools, in the order to reach for
them:

1. **Draw the instance around the invariant.** A mortgage with a fixed part
   and a tracker part is **one** instance with two parts (balance addresses
   per part), not two accounts that must agree: one repayment is split
   across both, and the loan-to-value is checked on the whole. The instance
   is the consistency boundary; choose it so the rules that must hold at
   every commit fall inside it.
2. **Read, don't write.** Another account's state is an input, declared,
   as of a time, settled if it must be complete (C5).
3. **Let reads create dependencies.** Corrections follow what was read,
   without the read account knowing (C14).
4. **Claim what must be unique.** Insert-if-absent, before the commit.
5. **Give a shared limit its own instance.** A credit line shared by an
   overdraft and a card, a family spending limit, a cash pool's cap: the
   limit is an instance (a *facility*), and drawing on it is an operation on
   the facility, which guards it and delivers the money to the account
   drawn. The invariant again sits inside one instance. Under heavy load,
   the facility's allowance can be escrowed across shards
   ([later](contract-models.md#later)).
6. **Only then, an extension,** with a commit across partitions (a saga on
   ScyllaDB). Banking's v1 has one case: a booking that debits two guarded
   accounts at once.

What remains genuinely harder is **eventual reads used as checks**: the
holders check in `LinkOffset` reads the saver as of moments ago. That is
acceptable where a later run corrects it (the next accrual re-reads, a
changed holder unlinks); it is not acceptable for money leaving the bank.
Money leaving is always a guarded debit, at home.
