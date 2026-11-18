# Contract engine — Example: a fixed-term deposit

Status: **architecture only**. The first example to read: one simple product,
from nothing to a customer's matured deposit, in the order the work is done.
Each concept is used once, with a link to where it is explained. When this
reads easily, the [advanced example](example-advanced-offset-mortgage.md)
adds reading other accounts, late reference data and twenty-five years of
change. The sketches are illustrative, not APIs.

**The product:** a twelve-month fixed deposit.

- The customer opens it, then funds it by transfer within **14 days**, with
  between £1,000 and £1,000,000.
- It earns a **fixed rate**, accrued daily.
- Nothing can be withdrawn before maturity.
- At maturity, principal and interest are paid to an account the customer
  nominated, and the deposit closes.

## The steps

| Step | Who | Produces |
|---|---|---|
| [1. Model](#1-model) | modelling team | a small `term-deposits` model |
| [2. Reference data](#2-reference-data) | finance | where interest is accounted |
| [3. Definition](#3-definition) | product owner | the product: parameters, operations, hooks, schedules |
| [4. Implementation](#4-implementation) | implementer | a WASM component and its manifest |
| [5. Test, publish, promote](#5-test-publish-promote) | implementer, product owner | a signed, live implementation |
| [6. Open a deposit](#6-open-a-deposit) | a channel | an account |
| [7. Its life](#7-its-life) | the customer, tempo, the engine | bookings and events |

The engine, the ledger, and the `core` model (parties, accounts, the ledger)
already exist.

---

## 1. Model

A model says what the data is, not how the product behaves
([models](contract-models.md)). A fixed deposit needs two balance types, a
facet holding its terms, and one view.

```rust
#[model(name = "term-deposits", version = "1.0.0", requires = "core@^1.5")]
mod term_deposits {
    #[ledger::balances]
    enum DepositBalance {
        Principal,
        #[ledger::balance(scale = 12)] Accrued,     // interest, finer than pennies until paid
    }

    /// The deposit's terms: a facet of the core account.
    #[extends(core::Account, when = "product_line = 'term-deposits'")]
    struct DepositTerms {
        matures_on: NaiveDate,
        /// The nominated account; changing it is a declared action.
        #[effective] #[action(set = "SetPayTo")]
        pay_to: AccountId,
    }

    /// What the product's hooks read about the deposit.
    #[view(key = account_id, consistency = "with-instance", from = core::Entry,
           group_by = address, fold = sum(signed(amount)))]
    struct DepositBalances { account_id: AccountId, address: BalanceAddress, amount: Money }
}
```

```text
$ tkone model plan term-deposits@1.0.0
term-deposits (new) 1.0.0                  requires core@^1.5 (applied: 1.5.0) ✔
  + balances   Principal, Accrued (scale 12)
  + actions    SetPayTo
  + facet      DepositTerms on core::Account
  + views      DepositBalances (with-instance)
$ tkone model apply term-deposits@1.0.0    # a second person approves
```

## 2. Reference data

Contracts name accounts by logical names; finance maps them to the chart of
accounts ([values](contract-engine.md#values-constants-code-sets-parameters-and-reference-data)):

```yaml
# reference/accounting-deposits-uk.yaml
mapping: deposits-uk
legal_entity: bank-uk
accounts:
  deposit_interest_expense: "5100-110"
```

```text
$ tkone reference load reference/accounting-deposits-uk.yaml
```

## 3. Definition

The definition is the product: its parameters, what callers can do, which
hooks the code must provide, which actions it may take, and its schedules
([definitions](contract-engine.md#definitions)).

```rust
#[definition(name = "fixed-12m", version = "1.0.0", model = "term-deposits@^1")]
#[ledger(guard = "debits-and-credits",      // credits must be accepted: the funding window
         credits_to = "Principal")]         // and, once accepted, count as principal
mod fixed_12m {
    #[parameter(kind = "commercial", levels(instance(required)))]   // fixed when opened
    #[range(min = "0", max = "0.10")]
    const RATE: Rate;
    #[parameter(kind = "commercial", levels(definition(default = "14")))]
    #[range(min = 1, max = 30)]
    const FUNDING_DAYS: u16;
    #[parameter(kind = "commercial", levels(definition(default = "1000.00 GBP")))]
    const MINIMUM: Money;
    #[parameter(kind = "commercial", levels(definition(default = "1000000.00 GBP")))]
    #[invariant("value > MINIMUM", reject = "maximum-below-minimum")]
    const MAXIMUM: Money;
    #[parameter(kind = "operational", levels(implementation(default = "HalfEven")))]
    const ROUNDING: Rounding;

    /// The customer may change where the money goes at maturity.
    #[operation(returns = { pay_to: DepositTerms.pay_to })]
    struct ChangePayTo { pay_to: AccountLocator }
    #[rejections(ChangePayTo)]
    enum ChangeRefused { Matured, NotTheHolder }

    /// What the customer will receive at maturity, at today's balance.
    #[query(returns = MaturityQuote)]
    struct QuoteMaturity;
    struct MaturityQuote { principal: Money, interest: Money, total: Money, on: NaiveDate }

    #[rejections(credited)]                       // the delivered hook's refusals
    enum CreditRefused { FundingClosed, AboveMaximum { room: Money } }

    #[event] struct Funded { amount: Money }
    #[event] struct Matured { principal: Money, interest: Money, paid_to: AccountId }

    #[hook(activation)]               fn open;
    #[hook(delivered(ledger::Entry))] fn credited;    // a credit landed: accept or refuse (funding)
    #[hook(scheduled)]                fn scheduled;   // accrue, funding-close, maturity

    permits! {
        Book,
        Transition(Pending -> Active | Closed, Active -> Closed),
        SetPayTo,
        ScheduleChange,
    }

    schedules! {
        time_zone: "Europe/London",
        calendar: "GBLO",
        start: 2026-01-01,
        /// Interest, each business evening.
        accrue:    "YY-MM-DD~SBT18:00:00",
        /// One-off dates each deposit selects: its funding deadline, its maturity.
        milestone: "YY-MM-DD~SBT09:00:00",
    }
}
```

- **Parameters** are typed, with where each may be set (`levels`), its limits
  (`#[range]`, `#[invariant]`) and its kind: `commercial` values are the
  customer's terms, `operational` ones are not
  ([parameters](contract-engine.md#parameters)). The rate has no default:
  every deposit is given one when it opens.
- **Operations** (`ChangePayTo`) and the **query** (`QuoteMaturity`) are what
  channels call. Each is handled by a hook of the same name
  ([hooks](contract-hooks.md)).
- **Guarded credits.** Money arriving is pending until `credited` accepts it,
  so a transfer after the funding window, or above the maximum, is returned
  to the sender ([delivery with acceptance](hot-accounts.md#delivery-with-acceptance)).
- **`permits!`** lists the only actions the code may return; anything else
  does not compile ([actions](contract-hooks.md#actions)).

- **`schedules!`** declares the product's schedules, in Rust like everything
  else. Each named rule becomes a tempo definition shared by every fixed
  deposit. The rules are written in tkone-schedule's grammar and parsed when
  the crate compiles, so a mistyped rule is a compile error. Registering the
  definition hands tempo its documents; nobody writes them by hand
  ([schedules](contract-engine.md#schedules)).

From `schedules!` the SDK generates two things the code uses:

```rust
pub enum Rule { Accrue, Milestone }       // what a run is for: run.is_for(Rule::Accrue)
pub struct Schedules { /* … */ }          // an account's own selection: .accrue(…), .milestone(…)
```

## 4. Implementation

### The code

Each hook says what it needs with `#[requires]`: the engine fetches exactly
that before the call, so the code does no I/O. Every hook takes two
arguments: a context the engine prepared (`ctx`: the instance, the time, the
caller, the parameters, and exactly what `#[requires]` named, in fields
generated from it) and the message that started it (an operation, a run, a
delivery). `cargo component build` writes the manifest
the engine reads from these attributes
([implementations](contract-engine.md#implementations)).

```rust
use tkone_contract_sdk::prelude::*;
use fixed_12m::{Action, Event, *};                         // generated from the definition

#[implementation(of = "fixed-12m@1.0.0")]
impl fixed_12m::Contract for Fixed12m {
    /// Money arrived. Accept it inside the window and under the maximum.
    #[requires(view(DepositBalances))]
    fn credited(ctx: &credited::Ctx, c: Delivery<ledger::Entry>) -> Result<Outcome, CreditRefused> {
        let (p, b) = (&ctx.params, &ctx.views.deposit_balances);
        if c.value_date > ctx.instance.opened_on + Days(p.FUNDING_DAYS) {
            return Err(CreditRefused::FundingClosed);
        }
        let room = p.MAXIMUM - b.principal;
        if c.amount > room {
            return Err(CreditRefused::AboveMaximum { room });
        }
        // Accepting is enough: the credit now counts, in Principal (credits_to).
        let mut out = Outcome::new().emit(Event::Funded { amount: c.amount });
        if ctx.instance.status == Status::Pending {
            out = out.act(Action::Transition(Active));        // first money: the deposit starts
        }
        Ok(out)
    }

    #[requires(view(DepositBalances),
               instance(from = terms.pay_to, read = [status]))]     // to pay out at maturity
    fn scheduled(ctx: &scheduled::Ctx, run: Run) -> Outcome {
        let (p, b) = (&ctx.params, &ctx.views.deposit_balances);
        let mut out = Outcome::new();

        if run.is_for(Rule::Accrue) {
            for day in run.days_since_last() {                // a Friday run covers the weekend
                let daily = b.principal * p.RATE.daily(DayCount::Act365Fixed);
                out = out.book(booking! {
                    code:   BankTransactionCode::INTEREST_ACCRUAL,
                    value_date: day,
                    debit:  ("deposit_interest_expense", daily),
                    credit: (This, Accrued, daily),
                });
            }
        }

        if run.is_for(Rule::Milestone) && ctx.instance.status == Status::Pending {
            // the funding window closed with nothing in it
            out = out.act(Action::Transition(Closed));
        }

        if run.is_for(Rule::Milestone) && run.due().date() == ctx.instance.terms.matures_on {
            let interest = b.accrued.round(p.ROUNDING);
            let pay_to = ctx.instances.pay_to.id;             // the nominated account
            out = out
                .book(booking! {                              // pay principal and interest out
                    code:   BankTransactionCode::MATURITY,
                    debit:  (This, Principal, b.principal),
                    debit:  (This, Accrued, interest),
                    credit: (Input(pay_to), b.principal + interest),   // lands where that account takes credits
                })
                .act(Action::Transition(Closed))
                .emit(Event::Matured { principal: b.principal, interest, paid_to: pay_to });
        }
        out
    }

    #[requires(instance(from = op.pay_to, read = [status, holders]))]
    fn change_pay_to(ctx: &change_pay_to::Ctx, op: ChangePayTo) -> Result<Outcome, ChangeRefused> {
        let to = &ctx.instances.pay_to;
        if !to.holders.contains_all(&ctx.instance.holders) {
            return Err(ChangeRefused::NotTheHolder);
        }
        Ok(Outcome::new().act(Action::SetPayTo(to.id)))
    }

    #[requires(view(DepositBalances))]
    fn quote_maturity(ctx: &quote_maturity::Ctx, _: QuoteMaturity) -> Result<MaturityQuote, Never> {
        let (p, b) = (&ctx.params, &ctx.views.deposit_balances);
        let matures_on = ctx.instance.terms.matures_on;
        let days = matures_on - ctx.time.today;
        let interest = (b.accrued + b.principal * p.RATE.daily(DayCount::Act365Fixed) * days)
            .round(p.ROUNDING);
        Ok(MaturityQuote { principal: b.principal, interest, total: b.principal + interest, on: matures_on })
    }

    /// Nothing to book until funded; start the deposit's own schedules.
    fn open(ctx: &open::Ctx, o: Opening) -> Result<Outcome, OpenRefused> {
        let deadline = o.opened_on + Days(ctx.params.FUNDING_DAYS);
        let schedules = Schedules::new()
            .accrue(Select::between(o.opened_on, o.matures_on))
            .milestone(Select::once_on(deadline))             // or the next business day
            .milestone(Select::once_on(o.matures_on));
        Ok(Outcome::new().act(Action::ScheduleChange(schedules)))
    }
}
```

```text
$ cargo component build --release
```

## 5. Test, publish, promote

A simulation runs the real component over a timeline of its schedules
([simulation](contract-engine.md#simulation)):

```rust
#[test]
fn pays_principal_and_interest_at_maturity() {
    let mut sim = Simulation::new()
        .definition("fixed-12m@1.0.0")
        .component("target/wasm32-wasip2/release/fixed_12m.wasm")
        .reference("reference/*.yaml");

    let current = sim.open("easy-saver@2", opening! {});
    let fd = sim.open("fixed-12m@1", opening! {
        parameters: { RATE: "0.0450" }, terms: { pay_to: current.id(), matures_on: date(2027, 12, 1) },
    });

    sim.transfer(current, fd, gbp(10_000), date(2026, 12, 3));           // accepted
    let late = sim.transfer(current, fd, gbp(500), date(2026, 12, 20));  // after 14 days
    assert_eq!(late.returned(), Some(CreditRefused::FundingClosed));

    sim.run_until(date(2027, 12, 2));
    assert_eq!(sim.status(fd), Status::Closed);
    assert_eq!(sim.event::<Matured>(fd).interest, gbp_pence(44_753));    // 363 days at 4.50% on £10,000
}
```

```text
$ tkone contract publish oci://registry.bank/contracts/fixed-12m:1.0.0 --sign
  sha256:3b9d…  registered as draft; WIT matches fixed-12m@1.0.0 ✔, requires bounded ✔
$ tkone contract promote sha256:3b9d… live --default      # the product owner approves
```

## 6. Open a deposit

```text
POST /instances
{ "definition": "fixed-12m@^1",
  "parties": [ { "party": "pty_01JA…", "role": "holder" } ],
  "terms": { "matures_on": "2027-12-01", "pay_to": "iban:GB33BUKB20201555555555" },
  "parameters": { "RATE": "0.0450" } }

201 Created
{ "account": "acc_01JF…", "status": "Pending", "identifiers": ["sort-code-account:202015-71234567"] }
```

In one commit, the engine binds the account to the live implementation,
issues its identifiers, and runs `open`, which returns the account's own
schedules as a `ScheduleChange`: accrue from today to maturity, and two
milestones, the funding deadline and maturity. They become its tempo set.
Every type here is declared in the [core domain](core-domain.md).

| Selection in `open` | Tempo receives |
|---|---|
| `.accrue(Select::between(1 Dec 2026, 1 Dec 2027))` | the `accrue` rule, from opening until maturity |
| `.milestone(Select::once_on(15 Dec 2026))` | `YY-12-15~NBT09:00:00`, once |
| `.milestone(Select::once_on(1 Dec 2027))` | `YY-12-01~NBT09:00:00`, once |

Tempo checks that each selection falls on slots its definition produces; a
selection that does not is refused at commit, and shows up in simulation
long before production.

## 7. Its life

| When | What happens | Hook | Written |
|---|---|---|---|
| 1 Dec 2026 | opened | `open` | the account, `Pending` |
| 3 Dec, 12:00 | the customer transfers £10,000 from their current account. The transfer is an operation on the **current account** (its debit is checked there); the credit is delivered to the deposit, pending | `credited` accepts | the credit counts as `Principal`; `Active`; `Funded` event |
| 3 Dec, 18:00 onwards | each business evening, interest for each day since the last run | `scheduled` (`accrue`) | an accrual booking per day |
| 15 Dec, 09:00 | the funding deadline: the deposit is funded, so nothing happens | `scheduled` (`milestone`) | nothing |
| 20 Dec | the customer tries to add £500 | `credited` refuses `FundingClosed` | a return to the current account |
| 4 Mar 2027 | the customer changes `pay_to` to a new account | `change_pay_to` | the new `pay_to`, effective today |
| 1 Dec 2027, 09:00 | maturity: interest rounded once, principal and interest paid to `pay_to` | `scheduled` (`milestone`) | the payout booking (debit at home, credit delivered); `Closed`; `Matured` event |

Every commit also produces events for the systems that need them:
statements, the general ledger, notifications.

---

## What this example left out

Each is in the [advanced example](example-advanced-offset-mortgage.md):

- **interfaces** shared by several products, so channels need not know which
  product an account is;
- **reading other accounts** (an offset), and corrections when they change
  late;
- **late reference data**, such as a rate published after its effective date;
- **change over years:** upgrades to new versions, product switches, model
  growth.

## Concepts used

| Here | Explained in |
|---|---|
| model, facet, balance types, views | [models](contract-models.md), [views](contract-models.md#views-four-forms-no-code) |
| exact numbers, rounding once | [models 5](contract-models.md#5-numbers-are-exact) |
| logical accounts, accounting mapping | [values](contract-engine.md#values-constants-code-sets-parameters-and-reference-data) |
| definition, parameters, `permits!` | [definitions](contract-engine.md#definitions) |
| operations, queries, rejections, events | [hooks, outcomes and events](contract-hooks.md) |
| guarded credits, returns, deliveries | [hot instances](hot-accounts.md) |
| `#[implementation]`, `#[requires]`, the generated manifest | [implementations](contract-engine.md#implementations) |
| schedules | [schedule documents](schedule-document.md) |
| publish, promote, the default | [C7](contract-engine.md#c7-implementation-versions-four-states-and-a-default) |
