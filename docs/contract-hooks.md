# Contract engine — Hooks, outcomes and events

Status: **architecture only**. Part of the [contract engine](contract-engine.md).
The sketches show what an implementer writes and what callers receive; they
are illustrative, not APIs.

A hook is a pure function. It gets a payload, a snapshot and the resolved
parameters, and returns an **outcome**: the actions it wants taken and the
events it wants told. The engine validates the outcome against the
definition and the model, and commits it. Everything a hook can say is a
declared type; nothing is a string or a blob the engine has to trust.

## Hooks and what each may return

| Hook | Runs when | Gets | Returns |
|---|---|---|---|
| **operation** (one per `#[operation]`) | a caller invokes it | the payload, its snapshot | `Result<Outcome, Rejection>` |
| **query** (one per `#[query]`) | a caller asks | the payload, its snapshot | `Result<T, Rejection>`: a value, never actions |
| `scheduled` | tempo's run is due | the run (due, sources, lateness), its snapshot | `Outcome`: it cannot refuse a due |
| `credited` | a delivered credit lands ([hot accounts](hot-accounts.md)) | the entries, its snapshot | `Outcome`; or, where credits are guarded, `Result<Outcome, CreditRefused>`: a refusal is returned to the sender |
| `activation`, `closure` | the account opens, closes | the payload, its snapshot | `Result<Outcome, Rejection>` |
| `upgrade` | the instance is rebound (C9) | the old state | `Result<Outcome, Rejection>` |
| `correction` | facts land before results already computed (C14) | the facts that moved the past | `Outcome` |

An operation is handled by **one hook**, named after it: `Withdraw` is
handled by `withdraw`. It checks and acts, or refuses. There is no separate
pre-hook to validate and post-hook to act; the check and the action belong
together, against the same snapshot.

Money reaches an account two ways, and each has its hook:

- **An operation on the account** (a cash deposit at a branch) is handled by
  that operation's hook, which may refuse it.
- **A credit delivered** from another account's booking (a transfer in) is
  handled by `credited`. On most accounts it reacts to money already counted.
  On an account whose definition **guards credits** (an ISA's annual
  allowance, a fixed-term deposit after its funding window, an account under
  a court order), the credit is pending until `credited` accepts it; if it
  refuses, the engine books a return to the sender
  ([guarded credits](hot-accounts.md#guarded-credits)).

```rust
fn credited(c: Credit, input: input::Credited, p: Params) -> Result<Outcome, CreditRefused> {
    let subscribed = input.state.subscribed_this_tax_year;
    if subscribed + c.amount > p.ANNUAL_ALLOWANCE {
        return Err(CreditRefused::AllowanceExceeded { remaining: p.ANNUAL_ALLOWANCE - subscribed });
    }
    Ok(Outcome::new())          // accepted: the credit counts from now
}
```

A `scheduled` or `correction` hook that cannot do its work does not reject:
it returns an empty outcome, or fails, and a failure is an incident for an
operator (D15's `stale_after` covers a due that keeps failing).

## The outcome

```rust
// tkone-contract: the platform's, versioned with the engine
pub struct Outcome<A = Action, E = Event> {
    pub actions: Vec<A>,     // what to change: committed together, or not at all
    pub events:  Vec<E>,     // what to tell: committed with the actions, in the outbox
}
```

In WIT, generated per definition:

```wit
record outcome { actions: list<action>, events: list<event> }

withdraw:    func(op: withdraw, input: withdraw-input, params: params) -> result<outcome, withdraw-refused>;
end-of-day:  func(run: run, input: end-of-day-input, params: params) -> outcome;
quote-withdrawal: func(q: quote-withdrawal, input: quote-input, params: params) -> result<withdrawal-quote, quote-refused>;
```

An implementation, with the SDK:

```rust
use tkone_contract_sdk::prelude::*;
use easy_saver::{Action, Event, Params, Rule, Withdraw, WithdrawRefused, input}; // generated

#[implementation(of = "easy-saver@2.1.0")]
impl easy_saver::Contract for Saver {
    #[requires(view(AccountState))]
    fn withdraw(op: Withdraw, input: input::Withdraw, p: Params)
        -> Result<Outcome, WithdrawRefused>
    {
        let available = input.state.available;
        if op.amount > available {
            return Err(WithdrawRefused::InsufficientFunds { available });
        }
        Ok(Outcome::new().book(booking! {
            code:   BankTransactionCode::CASH_WITHDRAWAL,
            debit:  (This, Available, op.amount),
            credit: ("customer_withdrawals", op.amount),   // logical, mapped (C17)
        }))
    }

    #[requires(view(AccountState), balances(as_of = due))]
    fn end_of_day(run: Run, input: input::EndOfDay, p: Params) -> Outcome {
        let mut out = Outcome::new();
        if run.is_for(Rule::Accrue) {
            out = out.book(accrual(&input.balances, p.RATE, run.due()));
        }
        if run.is_for(Rule::Capitalise) {
            let interest = input.balances.accrued.round(p.ROUNDING);
            out = out
                .book(capitalisation(interest))
                .emit(Event::InterestCapitalised { amount: interest, period: run.period() });
        }
        out
    }
}
```

The generated `Action` and `Event` hold only what the definition permits and
declares, so an implementation that tries anything else does not compile.

## Actions

An action is the only way anything is written. The engine must understand
each one: how to check it, how to write it, which views it changes, which
event it produces. That does not make the set of actions the engine's, or
banking's. It comes in three tiers:

| Tier | What | Declared in | Versioned with | Open? |
|---|---|---|---|---|
| **1. Kernel primitives** | the changes storage can make to any model's kinds | the engine | the engine | closed, and small |
| **2. Declared actions** | a domain's named actions: a primitive, its payload, its invariants | model crates | the model | open: any domain |
| **3. Extensions** | actions whose meaning needs code: the ledger | Rust crates compiled into the engine build | the extension | open, but rare and trusted |

The engine knows nothing about money. Banking is the core model's declared
actions plus the ledger extension; a metering or insurance product is its own
models, with or without an extension of its own.

### 1. Kernel primitives

| Primitive | Applies to | Writes |
|---|---|---|
| `Append` | a `#[fact]` type | a fact in the home journal |
| `Set` | an `#[effective]` attribute | a new version, effective from a date |
| `Create`, `End` | an `#[effective]` entity: something that starts and stops | the entity, and its end |
| `Transition` | a state machine declared in a model | the state, effective from the due |
| `Deliver` | a fact for another instance | an insert into its inbox, unguarded ([hot accounts](hot-accounts.md)) |
| `ScheduleChange` | the instance's tempo set | its next version (D12) |

These mirror what the store can do (C12, C13), which is why they rarely
change. `Emit`, for events, sits beside them in the outcome.

### 2. Declared actions

A model names its actions: which primitive each is, on which of its types,
and the **invariants** it must keep. Invariants are
[CEL](https://cel.dev) expressions over the action's payload, its parameters
and the instance's snapshot, evaluated by the engine at commit; a failed one
is a typed rejection.

```rust
#[model(name = "core", version = "1.5.0", extensions = ["ledger@^1"])]
mod core {
    #[effective(key = account_id)]
    #[action(create = "Hold", end = "Release")]
    #[invariant("amount.currency == state.currency")]
    #[invariant("amount <= state.available", reject = "hold-exceeds-available")]
    struct Hold { account_id: AccountId, balance: BalanceType, amount: Money,
                  until: Option<NaiveDate>, reason: HoldReason }

    #[state_machine(Account.status)]
    transitions! { Pending -> Active, Active -> Dormant | Closed, Dormant -> Active | Closed }

    #[action(set = "SetParameter", on = "instance parameters")]
    #[invariant("value >= parameter.min && value <= parameter.max")]
    struct ParameterOverride;

    #[fact(key = booking_id)]
    #[action(extension = "ledger.book", reverse = "ledger.reverse")]   // tier 3
    struct Booking { /* below */ }
}
```

The same mechanism, with no banking in it:

```rust
#[model(name = "metering", version = "1.0.0")]
mod metering {
    #[fact(key = subscription_id)]
    #[action(append = "RecordUsage")]
    #[invariant("quantity > 0")]
    #[invariant("at <= due")]
    struct Usage { subscription_id: SubscriptionId, meter: MeterCode,
                   quantity: Quantity<KilowattHour>, at: DateTime }

    #[effective(key = subscription_id)]
    #[action(set = "ChangeTariff")]
    struct Tariff { subscription_id: SubscriptionId, tariff: Ref<TariffCard> }

    #[state_machine(Subscription.status)]
    transitions! { Pending -> Active, Active -> Suspended | Ended, Suspended -> Active | Ended }
}
```

- **Actions belong to models, not definitions.** An action changes data that
  other products, views and downstream systems read. If one product could
  say what `Hold` means, a view summing a customer's holds across products
  would sum different things. Models say what an action is; definitions say
  which a product may use; implementations say when, and with what values.
- **Invariants read one instance.** The payload, the parameters and the
  instance's own snapshot. A rule across instances cannot be checked at
  commit without the coordination C6 avoids: it is an extension's, or an
  asynchronous check's.
- **CEL stays small:** comparisons, arithmetic, membership, over declared
  fields; no loops, no calls. What needs more is an extension.
- **Each declared action gets an event,** emitted by the engine on commit
  (`core.hold-created`, `metering.usage-recorded`).

### 3. Extensions

Some meanings cannot be a primitive plus invariants. A booking is spread
across accounts; its credits are delivered; a guarded debit is re-checked
against sharded balances; the day's close folds inboxes and reconciles. That
is an **extension**, compiled into the engine build:

```rust
pub trait Extension {
    const NAME: &'static str;                    // "ledger"
    const VERSION: Version;

    /// The actions it gives models, with their payload types.
    fn actions(&self) -> &[ActionSpec];
    /// Checks against the snapshot; a violation is a typed rejection.
    fn validate(&self, action: &ExtAction, snapshot: &Snapshot) -> Result<(), Violation>;
    /// The writes, in kernel primitives: appends at home, deliveries elsewhere.
    fn plan(&self, action: &ExtAction, snapshot: &Snapshot) -> Vec<Primitive>;
    /// Work at a business day's close: checkpoints, reconciliation.
    fn close(&self, day: BusinessDate, store: &dyn ReadStore) -> CloseReport;
}
```

- **Extensions plan; the engine writes.** An extension turns its action into
  kernel primitives, so storage, journals, views and events stay the
  engine's, on every store.
- **Required by models, checked at registration.** A model declares the
  extensions it needs (`extensions = ["ledger@^1"]`); applying it to an
  engine without them is refused, as an unapplied model version is (C10).
- **Trusted, not sandboxed.** Extensions enforce the invariants other code
  relies on, so they are reviewed platform code in-process, not WASM. The
  sandbox is for products.
- **The bar is high.** If most models need an extension, a primitive is
  missing. Banking's v1 needs one: the ledger.

The ledger extension's actions, which `core` declares on `Booking`:

```rust
pub struct Booking {
    pub code: BankTransactionCode,       // ISO 20022: domain, family, sub-family
    pub value_date: NaiveDate,           // the booking date is the commit's
    pub entries: Vec<Leg>,               // at most 1000 (C21)
    pub narrative: Option<Text>,         // personal (C19)
    pub details: Option<BookingDetails>, // declared facets: card, payment, tax
}

pub struct Leg { pub account: AccountRef, pub balance: BalanceType, pub amount: Money, pub side: Side }

pub enum AccountRef {
    This,                    // the instance the hook runs for
    Logical(LogicalAccount), // "interest_expense": an internal account, by the accounting mapping
    Input(AccountId),        // an account the hook was given (a transfer's payee), never invented
}
```

| Action | The ledger checks | It plans |
|---|---|---|
| `ledger.book` | entries net to zero per currency; amounts in the currency's minor units (or a balance type's declared scale); logical accounts resolve as of the due; `Input` accounts were inputs; a guarded debit leaves the balance within its limit, whatever the hook thought | `Append` the booking at home; `Deliver` each other account's entries (C22) |
| `ledger.reverse` | the booking is this instance's and not already reversed | the same, with opposite entries, naming the first |

The ledger re-checks the limit the hook already checked: the hook's check is
product logic; the ledger's is an invariant, in reviewed code.

### What a hook sees

Ids of what an outcome creates (bookings, holds) are made by the engine from
the instance, its version and the action's position, so a replay makes the
same ids. The SDK generates one `Action` enum per definition, from the
actions of the models it uses, narrowed:

```rust
// generated for easy-saver, from core and deposits, narrowed by permits!
pub enum Action {
    Book(Booking), Reverse(Reversal),
    Hold(Hold), Release(HoldId),
    Transition(AccountStatusTo),          // Dormant | Closed | Active
    ScheduleChange(ScheduleChange),
}
```

### Narrowing

A definition narrows the actions its models declare, and may narrow them
further per hook:

```rust
permits! {
    Book, Reverse, Hold, Release,
    Transition(Active -> Dormant | Closed, Dormant -> Active),
    ScheduleChange,
}

#[hook(correction, permits = [Book, Reverse])]
fn correct;
```

Each narrowing generates its own types: `Transition`'s target is an enum of
`Dormant | Closed | Active`, and the correction hook's `Action` has two
variants. The engine checks the same narrowing again at commit, so a
component built from altered WIT gains nothing.

## Events

Events are what other systems hear: statements, notifications, the general
ledger, monitoring for financial crime, the data warehouse. Two kinds:

| | Declared by | Emitted | Examples |
|---|---|---|---|
| **Action events** | the model or extension that declares the action | by the engine, for every committed action, whatever the contract does | `ledger.booking-committed`, `core.hold-created`, `core.hold-ended`, `core.account-status-changed`, `metering.usage-recorded`; and the engine's own: `ScheduleChanged`, `InstanceUpgraded`, `IdentifierIssued`, `PartyMerged`, `SubjectErased` |
| **Domain events** | an interface or definition, `#[event]` | by a hook, `.emit(…)` | `InterestCapitalised`, `NoticeGiven`, `BonusRateLost` |

Downstream systems that must not miss anything (the general ledger,
reconciliation, monitoring) read **action events**, so they never depend on
a contract remembering to emit. Domain events carry what only the product
knows: why something happened, in the product's terms.

Every event is delivered in an envelope:

```json
{
  "id":        "acc_01JD3K…/1843/0",
  "type":      "deposit-account.interest-capitalised",
  "schema":    "sha256:41aa…",
  "instance":  "acc_01JD3K…",
  "version":   1843,
  "definition": "easy-saver@2.1.0",
  "implementation": "sha256:c07e…",
  "due":       "2026-11-30T18:00:00Z",
  "committed": "2026-11-30T18:00:02.114Z",
  "cause":     { "operation": "end_of_day", "idempotency_key": null },
  "payload":   { "amount": { "minor": "1234", "currency": "GBP" },
                 "period": { "from": "2026-11-01", "to": "2026-11-30" } }
}
```

- **Committed with the actions.** Events go into the outbox in the same
  commit: no event for a change that did not happen, no change without its
  events.
- **At least once, ordered per instance.** The id (instance, version,
  position) lets consumers drop duplicates. Across instances there is no
  order, as there is none across schedule documents (D14).
- **Typed and versioned** like everything else: stable field ids, growth,
  the schema's digest in the envelope (C18, C23).
- **Personal fields stay encrypted** in payloads, as in the store. Consumers
  are registered, and that registration is an entitlement, with the
  retention they promise; `SubjectErased` tells them to forget (C19).

## Attributes and opaque data: not here

Some contract platforms let a call return free-form key/value attributes,
for indexers to build their own views from the log, and an opaque byte
payload for the caller. Neither has a place here.

**Attributes.** They exist where the log is the only store and every reader
indexes it themselves. Here the engine owns storage and the views. Banking
does need to find bookings by more than account: by transaction type for
statements and tax, by merchant and scheme reference for disputes, by purpose
code for regulatory reporting, by counterparty for monitoring. Each is a
**typed field**:

- every booking has a `BankTransactionCode` (ISO 20022's domain, family and
  sub-family), a closed code set;
- what a kind of booking carries beyond that is a **facet of `Booking`**,
  declared in a model like any other ([models](contract-models.md)):

  ```rust
  #[extends(core::Booking, when = "code.domain = 'PMNT' and code.family = 'CCRD'")]
  struct CardDetails { merchant: MerchantName, mcc: Mcc, scheme_reference: SchemeRef }
  ```

- a search that is run often is a **view keyed by it**
  (`BookingsBySchemeReference`);
- systems outside the engine index the **event stream**.

Free-form attributes would be untyped, unversioned and unclassified: what
the [model rules](contract-models.md) 1–4 exist to prevent.

**Opaque result bytes.** A caller's answer is typed in three ways instead:

| The caller wants | Declared as |
|---|---|
| state after the change | `returns`: a selection from `with-instance` views |
| why it was refused | the operation's rejection type |
| an answer that is computed, not stored | a query |

## Operation results

An operation declares what it returns as a selection from `with-instance`
views:

```rust
#[operation(returns = { available: AccountState.available, holds: AccountState.holds })]
struct Withdraw { amount: Money, channel: Channel }
```

```json
200 OK
{
  "instance_version": 1843,
  "bookings": ["bkg_01JD…"],
  "result": { "available": { "minor": "120000", "currency": "GBP" }, "holds": [] }
}
```

- **Exact as of the commit.** Only `with-instance` views may be selected; the
  engine holds their new values already, having written them in the commit,
  so it reads nothing more. `model plan` refuses a selection of an `eventual`
  view.
- **Typed per operation, with no code.** It comes from views, so it cannot
  disagree with what a read returns a moment later.
- **The receipt is always there:** the instance's new version and the
  bookings made, whether or not `returns` is declared.

## Queries

What needs the product's logic but is not stored is a query: a quote, an
eligibility check, "what if I repay now".

```rust
#[query(returns = WithdrawalQuote)]
struct QuoteWithdrawal { amount: Money, on: NaiveDate }

struct WithdrawalQuote { penalty: Money, available_on: NaiveDate }

#[rejections(QuoteWithdrawal)]
enum QuoteRefused { Closed }
```

```text
POST /accounts/acc_01JD3K…/queries/quote-withdrawal
{ "amount": { "minor": "500000", "currency": "GBP" }, "on": "2026-12-01" }

200 OK
{ "penalty": { "minor": "2740", "currency": "GBP" }, "available_on": "2027-02-28" }
```

A query runs the same pipeline (entitle, resolve, parameters, fetch, invoke)
and stops there: no validation of actions, no commit. The engine refuses an
outcome from a query hook; the generated WIT gives it no way to return one.
Queries are part of the interface's schema, with the same rules as
operations.
