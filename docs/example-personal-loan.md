# Contract engine — Example: a personal loan

Status: **architecture only**. The second example, after the
[fixed-term deposit](example-fixed-deposit.md): money goes out first and
comes back in instalments, payments are allocated, arrears are handled, and
a model shared with another product grows by a minor version. The
[offset mortgage](example-advanced-offset-mortgage.md) then adds reading
other accounts. Every type used here is declared in the
[core domain](core-domain.md). The sketches are illustrative, not APIs.

**The product:** an unsecured personal loan.

- £1,000 to £25,000 over 12 to 60 months, at a **fixed rate** set when it is
  offered, accrued daily.
- Paid out to the customer's nominated account when it opens.
- A fixed monthly **instalment** by direct debit on a day the customer
  chooses; the last one settles what remains.
- A missed instalment puts the loan **in arrears** with a £12 fee; three
  missed, and it is passed to collections as **in default**.
- **Overpayments** are free. **Early settlement** costs 28 days' interest; a
  quote says exactly how much.

## The steps

| Step | Who | Produces |
|---|---|---|
| [1. Model](#1-model-grow-lending) | modelling team | `lending` 1.1.0: one facet widened, one state machine added |
| [2. Reference data](#2-reference-data) | finance | where interest and fees are accounted |
| [3. Definition](#3-definition) | product owner | `personal-loan@1` |
| [4. Implementation](#4-implementation) | implementer | a WASM component and its manifest |
| [5. Test, publish, promote](#5-test-publish-promote) | implementer, product owner, risk | a signed, live implementation |
| [6. Open a loan](#6-open-a-loan) | a channel | an account, paid out |
| [7. Its life](#7-its-life) | the customer, tempo, the engine | bookings and events |

---

## 1. Model: grow `lending`

The mortgage team already applied `lending` 1.0.0: loan balance types, loan
terms, payment holidays. A personal loan is the same kind of thing, so it
uses the same model rather than a new one. It needs two additions, both
growth ([models 2](contract-models.md#2-models-grow-they-do-not-break)):

```rust
#[model(name = "lending", version = "1.1.0", requires = "core@^1.5")]
mod lending {
    /// Now also personal loans.
    #[extends(core::Account, when = "product_line in ['mortgages', 'personal-loans']")]
    struct LoanTerms {
        #[effective] term_end: NaiveDate,
        #[effective] repayment_day: u8,
        #[effective] repayment: RepaymentType,
        /// New in 1.1: where collection stands. Optional for loans opened before.
        #[effective] collection: Option<CollectionStatus>,
    }

    /// New in 1.1: arrears and default, for every loan product.
    /// Starts at Current when the facet is created.
    #[state_machine(LoanTerms.collection, initial = "Current")]
    transitions! { Current -> Arrears, Arrears -> Current | Default }

    // balances, offsets, payment holidays, views: unchanged (core domain)
}
```

```text
$ tkone model plan lending@1.1.0
lending 1.0.0 → 1.1.0                         growth only: minor ✔
  ~ facet          LoanTerms covers personal-loans
  + field          LoanTerms.collection   field 12   Option<CollectionStatus>
  + state machine  LoanTerms.collection   Current, Arrears, Default
  postgres  ALTER TABLE lending_loan_terms ADD COLUMN f12 text;
$ tkone model apply lending@1.1.0      # a second person approves
```

The mortgage's implementation, built against 1.0.0, keeps getting its inputs
in the 1.0.0 shape ([models 3](contract-models.md#3-compatibility-is-checked-per-kind)).
Nothing about it changes.

## 2. Reference data

```yaml
# reference/accounting-loans-uk.yaml
mapping: loans-uk
legal_entity: bank-uk
accounts:
  loan_interest_income: "4100-300"
  loan_fee_income:      "4300-300"
```

```text
$ tkone reference load reference/accounting-loans-uk.yaml
```

## 3. Definition

```rust
#[definition(name = "personal-loan", version = "1.0.0", model = "lending@^1.1")]
#[identifiers(issue = ["loan-number"])]
#[ledger(guard = "debits", credits_to = "Incoming")]   // payments land in Incoming; `credited` allocates
mod personal_loan {
    approval_roles! {
        /// Approves a restructured instalment.
        collections,
    }

    #[parameter(kind = "commercial", levels(instance(required)))]    // fixed at offer
    #[range(min = "0.029", max = "0.299")]
    const RATE: Rate;
    #[parameter(kind = "commercial",                                  // set by `open`; changed
                levels(instance(approve(by = collections))))]       //   only in a restructure
    #[invariant("value.currency == account.currency")]
    const INSTALMENT: Money;
    #[parameter(kind = "commercial", levels(definition(default = "12.00 GBP")))]
    const LATE_FEE: Money;
    #[parameter(kind = "commercial", levels(definition(default = "3")))]
    #[range(min = 2, max = 6)]
    const DEFAULT_AFTER: u8;                                          // missed instalments
    #[parameter(kind = "commercial", levels(definition(default = "28")))]
    #[range(min = 0, max = 58)]
    const SETTLEMENT_DAYS: u16;
    #[parameter(kind = "operational", levels(implementation(default = "loans-uk")))]
    const ACCOUNTS: AccountingMapping;
    #[parameter(kind = "operational", levels(implementation(default = "HalfUp")))]
    const ROUNDING: Rounding;

    /// What it costs to settle the loan on a date.
    #[query(returns = SettlementQuote)]
    struct QuoteSettlement { on: NaiveDate }
    struct SettlementQuote { principal: Money, interest: Money, fees: Money, total: Money, valid_until: NaiveDate }

    #[event] struct InstalmentDue { amount: Money, due_on: NaiveDate }
    #[event] struct PaymentAllocated { fees: Money, due: Money, principal: Money }
    #[event] struct ArrearsStarted { missed: Money }
    #[event] struct DefaultRaised { missed_instalments: u8, outstanding: Money }
    #[event] struct Settled { on: NaiveDate }

    #[hook(activation)]               fn open;        // pay out; compute the instalment; schedules
    #[hook(scheduled)]                fn scheduled;   // accrue, instalment
    #[hook(delivered(ledger::Entry))] fn credited;    // a credit landed: allocate; clear arrears; settle

    permits! {
        Book,
        SetParameter(INSTALMENT),
        Transition(Pending -> Active, Active -> Closed),
        Transition(collection: Current -> Arrears, Arrears -> Current | Default),
        ScheduleChange,
    }

    schedules! {
        time_zone: "Europe/London",
        calendar: "GBLO",
        start: 2026-01-01,
        /// Interest, each business evening.
        accrue:     "YY-MM-DD~SBT18:00:00",
        /// Instalments fall due in the morning; each loan selects its day.
        instalment: "YY-MM-DD~SBT09:00:00",
    }
}
```

- **The instalment is an instance parameter** set by the loan's own `open`
  hook, so it is visible, versioned, and can be changed by an operator if a
  loan is restructured.
- **Arrears and default** are a state machine the model declares; this
  product decides when to move along it.
- **No operation to settle.** The customer gets a quote, then pays it; the
  `credited` hook recognises a payment that settles the loan.

- **Schedules** are declared in Rust, as in the
  [fixed deposit](example-fixed-deposit.md#3-definition): each named rule is
  a tempo definition shared by every personal loan, checked when the crate
  compiles. Each loan selects from them in `open`, with its own dates.

## 4. Implementation

### The code

Each hook says what it needs with `#[requires]`: the engine fetches exactly
that before the call, so the code does no I/O. Every hook takes the
context the engine prepared (`ctx`, with fields generated from
`#[requires]`) and the message that started it. `cargo component build` writes the manifest
the engine reads from these attributes
([implementations](contract-engine.md#implementations)).

```rust
use tkone_contract_sdk::prelude::*;
use personal_loan::{Action, Event, *};                      // generated

#[implementation(of = "personal-loan@1.0.0")]
impl personal_loan::Contract for PersonalLoan {
    /// Pay out, fix the instalment, start the schedules.
    #[requires(instance(from = opening.disburse_to, read = [status, holders]))]
    fn open(ctx: &open::Ctx, o: Opening) -> Result<Outcome, OpenRefused> {
        let (p, payout) = (&ctx.params, &ctx.instances.disburse_to);
        if !payout.holders.contains_all(&o.borrowers) {
            return Err(OpenRefused::PayoutNotTheBorrowers);
        }
        let instalment = annuity(o.amount, p.RATE, o.months).round(p.ROUNDING);
        Ok(Outcome::new()
            .book(booking! {
                code:   BankTransactionCode::LOAN_DRAWDOWN,
                debit:  (This, Principal, o.amount),
                credit: (Input(payout.id), o.amount),            // delivered to the customer
            })
            .act(Action::SetParameter(INSTALMENT, instalment, o.opened_on))
            .act(Action::Transition(Active))
            .act(Action::ScheduleChange(schedules(&o))))
    }

    #[requires(view(LoanState))]
    fn scheduled(ctx: &scheduled::Ctx, run: Run) -> Outcome {
        let (p, s) = (&ctx.params, &ctx.views.loan_state);
        let mut out = Outcome::new();

        if run.is_for(Rule::Accrue) {
            for day in run.days_since_last() {
                let daily = s.principal * p.RATE.daily(DayCount::Act365Fixed);
                out = out.book(booking! {
                    code: BankTransactionCode::LOAN_INTEREST_ACCRUAL,
                    value_date: day,
                    debit:  (This, Accrued, daily),
                    credit: ("loan_interest_income", daily),
                });
            }
        }

        if run.is_for(Rule::Instalment) {
            // Anything still due from last month is a missed instalment.
            if s.due.is_positive() {
                out = out.book(booking! {
                    code: BankTransactionCode::LOAN_FEE,
                    debit:  (This, Fees, p.LATE_FEE),
                    credit: ("loan_fee_income", p.LATE_FEE),
                });
                let missed = s.due.ceil_div(p.INSTALMENT);
                if s.collection == Current {
                    out = out.act(Action::Transition(collection: Arrears))
                             .emit(Event::ArrearsStarted { missed: s.due });
                } else if missed >= p.DEFAULT_AFTER {
                    out = out.act(Action::Transition(collection: Default))
                             .emit(Event::DefaultRaised { missed_instalments: missed, outstanding: s.outstanding });
                }
            }
            // This month's instalment falls due: interest first, the rest is capital.
            let interest = s.accrued.round(p.ROUNDING);
            let amount = if run.is_last() { s.principal + interest } else { p.INSTALMENT };
            out = out
                .book(booking! {
                    code: BankTransactionCode::LOAN_INSTALMENT,
                    debit:  (This, Due, amount),
                    credit: (This, Accrued, interest),
                    credit: (This, Principal, amount - interest),
                })
                .emit(Event::InstalmentDue { amount, due_on: run.due().date() });
        }
        out
    }

    /// A payment arrived in Incoming: fees, then what is due, then principal.
    #[requires(view(LoanState))]
    fn credited(ctx: &credited::Ctx, c: Delivery<ledger::Entry>) -> Outcome {
        let (p, s) = (&ctx.params, &ctx.views.loan_state);
        let q = settlement(s, p, c.value_date);
        let (fees, due, principal) = allocate(c.amount, s.fees, s.due);

        let mut out = Outcome::new()
            .book(allocation(fees, due, principal))              // out of Incoming
            .emit(Event::PaymentAllocated { fees, due, principal });

        if s.collection == Arrears && due == s.due && fees == s.fees {
            out = out.act(Action::Transition(collection: Current));
        }
        if c.amount >= q.total {                                 // it settles the loan
            out = out
                .book(final_interest(q.interest - s.accrued.round(p.ROUNDING)))
                .act(Action::Transition(Closed))
                .act(Action::ScheduleChange(Schedules::end_all(c.value_date)))
                .emit(Event::Settled { on: c.value_date });
        }
        out
    }

    #[requires(view(LoanState))]
    fn quote_settlement(ctx: &quote_settlement::Ctx, q: QuoteSettlement) -> Result<SettlementQuote, Never> {
        Ok(settlement(&ctx.views.loan_state, &ctx.params, q.on))   // the same function `credited` uses
    }
}

/// The loan's own schedules: accrue for its term; the instalment on its day.
fn schedules(o: &Opening) -> Schedules {
    let first_due = o.opened_on.first_full_month_on(o.repayment_day);
    let last_due = first_due + Months(o.months - 1);
    Schedules::new()
        .accrue(Select::between(o.opened_on, last_due))
        .instalment(Select::between(first_due, last_due)
            .rule(rule!("YY-MM-{day:02}~NBT09:00:00", day = o.repayment_day)))
}

/// Principal, interest to the date plus SETTLEMENT_DAYS, and fees.
fn settlement(s: &LoanState, p: &Params, on: NaiveDate) -> SettlementQuote { /* … */ }
```

```text
$ cargo component build --release
```

## 5. Test, publish, promote

```rust
#[test]
fn a_missed_instalment_then_early_settlement() {
    let mut sim = Simulation::new()
        .definition("personal-loan@1.0.0")
        .component("target/wasm32-wasip2/release/personal_loan.wasm")
        .reference("reference/*.yaml");

    let current = sim.open("easy-saver@2", opening! { deposit: gbp(2_000) });
    let loan = sim.open("personal-loan@1", opening! {
        amount: gbp(15_000), months: 48, repayment_day: 1, disburse_to: current.id(),
        parameters: { RATE: "0.079" },
    });
    assert_eq!(sim.parameter(loan, "INSTALMENT"), gbp_pence(365_49));
    assert_eq!(sim.balance(current, Available), gbp(17_000));          // paid out

    sim.direct_debit_each(loan, current, sim.parameter(loan, "INSTALMENT"))
       .except([date(2027, 3, 1), date(2027, 4, 1)]);                  // March bounces
    sim.run_until(date(2027, 4, 1));
    assert_eq!(sim.events::<ArrearsStarted>(loan).len(), 1);
    sim.transfer(current, loan, gbp_pence(742_98), date(2027, 4, 1));  // two instalments and the fee
    assert_eq!(sim.collection(loan), Current);

    let quote = sim.query(loan, QuoteSettlement { on: date(2027, 6, 15) });
    sim.transfer(current, loan, quote.total, date(2027, 6, 15));
    assert_eq!(sim.status(loan), Status::Closed);
}
```

```text
$ tkone contract publish oci://registry.bank/contracts/personal-loan:1.0.0 --sign
  sha256:a41f…  registered as draft; WIT matches personal-loan@1.0.0 ✔; lending@^1.1 applied ✔
$ tkone contract promote sha256:a41f… live --default      # product owner and risk approve
```

## 6. Open a loan

```text
POST /instances
{ "definition": "personal-loan@^1",
  "parties": [ { "party": "pty_01JA…", "role": "borrower" } ],
  "opening": { "amount": { "minor": "1500000", "currency": "GBP" }, "months": 48,
               "repayment_day": 1, "disburse_to": "iban:GB33BUKB20201555555555" },
  "parameters": { "RATE": "0.079" } }

201 Created
{ "account": "acc_01JG…", "status": "Active", "identifiers": ["loan-number:7300418826"],
  "result": { "instalment": { "minor": "36549", "currency": "GBP" } } }
```

In one commit on the new account: the engine binds it to the live
implementation and issues its loan number; `open` books the payout (debit at
home, credit delivered to the current account), sets `INSTALMENT`, moves it
to `Active` (its collection status starts at `Current`), and returns its schedules, which become the
account's tempo set: accrue each business evening for 48 months, and the
instalment on the 1st, or the next business day.

## 7. Its life

| When | What happens | Hook | Written |
|---|---|---|---|
| 14 Dec 2026 | opened and paid out | `open` | payout; `INSTALMENT` £365.49; `Active` (collection starts `Current`) |
| Each business evening | interest for each day since the last run | `scheduled` (`accrue`) | an accrual per day |
| 1 Feb 2027, 09:00 | the first instalment due (the first full month) | `scheduled` (`instalment`) | `Due` £365.49; `InstalmentDue` |
| 1 Feb, 11:00 | direct debit collected: an operation on the current account; the credit is delivered | `credited` | `Incoming` → `Due`; `PaymentAllocated` |
| 1 Mar | instalment due; the direct debit bounces (insufficient funds on the current account, refused there) | `scheduled` | `Due` £365.49 |
| 1 Apr, 09:00 | March is still due: £12 fee, `Arrears`; April falls due | `scheduled` | fee; `Arrears`; `ArrearsStarted`; `Due` £730.98 |
| 1 Apr, 11:00 | the customer pays £742.98 | `credited` | fee, then due; back to `Current` |
| 15 Jun 2027 | the customer asks for a settlement figure | `quote_settlement` | nothing: a query |
| 15 Jun 2027 | the customer pays it | `credited` | allocation, final interest; `Closed`; schedules ended; `Settled` |

---

## What this example added to the fixed deposit

- **A shared model, grown:** `lending` 1.1.0 serves the mortgage and the loan,
  and the mortgage's implementation did not notice.
- **Allocation:** credits land in `Incoming` and the product decides where
  they go.
- **A domain state machine** (`collection`), beside the core account status.
- **A parameter set by the contract itself** (`INSTALMENT`), and schedules
  returned by `open`.
- **A query and a payment that share one function,** so the quote and the
  settlement cannot disagree.

Still left out, and in the [offset mortgage](example-advanced-offset-mortgage.md):
interfaces, reading other accounts, late reference data, upgrades over years.
