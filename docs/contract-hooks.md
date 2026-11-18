# Contract engine — Hooks, outcomes and events

Status: **architecture only**. Part of the [contract engine](contract-engine.md).
The sketches show what an implementer writes and what callers receive; they
are illustrative, not APIs.

A hook is a pure function of two arguments: a **context** the engine
prepared (the instance, the time, the caller, the resolved parameters, and
exactly the reads its `#[requires]` declares) and the **message** that
started it (an operation, a run, a delivery). It returns an **outcome**: the actions it wants taken and the
events it wants told. The engine validates the outcome against the
definition and the model, and commits it. Everything a hook can say is a
declared type; nothing is a string or a blob the engine has to trust.

## Hooks and what each may return

| Hook | Runs when | Message | Returns |
|---|---|---|---|
| **operation** (one per `#[operation]`) | a caller invokes it | the operation (`Withdraw`) | `Result<Outcome, Rejection>` |
| **query** (one per `#[query]`) | a caller asks | the query (`QuoteWithdrawal`) | `Result<T, Rejection>`: a value, never actions |
| `scheduled` | tempo's run is due | `Run`: due, sources, lateness | `Outcome`: it cannot refuse a due |
| `delivered(T)` | a fact of type `T` delivered from another instance's commit lands ([hot instances](hot-accounts.md)) | `Delivery<T>` | `Outcome`; or, where that direction is guarded with acceptance, `Result<Outcome, Refusal>`: a refusal is compensated to the sender |
| `activation`, `closure` | the instance opens, closes | `Opening`, `Closing` | `Result<Outcome, Rejection>` |
| `upgrade` | the instance is rebound (C9) | `Upgrade`: from and to versions, the old state | `Result<Outcome, Rejection>` |
| `correction` | facts land before results already computed (C14) | `MovedFacts` | `Outcome` |

Every hook has the same shape, `fn hook(ctx: &hook::Ctx, message) -> …`:
[the hook signature](#the-hook-signature).

An operation is handled by **one hook**, named after it: `Withdraw` is
handled by `withdraw`. It checks and acts, or refuses. There is no separate
pre-hook to validate and post-hook to act; the check and the action belong
together, against the same snapshot.

A write reaches an instance two ways, and each has its hook:

- **An operation on the instance** (a cash deposit at a branch) is handled by
  that operation's hook, which may refuse it.
- **A fact delivered** from another instance's commit (a transfer in) is
  handled by the hook the definition declares for that fact type,
  `#[hook(delivered(T))]`. On most instances it reacts to what already
  counts. Where the definition **guards that direction with acceptance**,
  the fact is pending until the hook accepts it; if it refuses, the kernel
  commits the fact's compensation back to the sender
  ([delivery with acceptance](hot-accounts.md#delivery-with-acceptance)).

The kernel names the hook kind; the definition names the hook. With the
ledger, delivered entries are credits, and the hook is conventionally
`credited`: an ISA that refuses subscriptions over its allowance
(`#[ledger(guard = DebitsAndCredits)]`, `#[hook(delivered(ledger::Entry))] fn credited;`):

```rust
#[requires(view(IsaState))]
fn credited(ctx: &credited::Ctx, c: Delivery<ledger::Entry>) -> Result<Outcome, CreditRefused> {
    let (allowance, subscribed) = (ctx.params.ANNUAL_ALLOWANCE, ctx.views.isa_state.subscribed_this_tax_year);
    if subscribed + c.amount > allowance {
        return Err(CreditRefused::AllowanceExceeded { remaining: allowance - subscribed });
    }
    Ok(Outcome::new())          // accepted: the credit counts from now
}
```

A `scheduled` or `correction` hook that cannot do its work does not reject:
it returns an empty outcome, or fails, and a failure is an incident for an
operator (D15's `stale_after` covers a due that keeps failing).

## The hook signature

A hook takes two arguments and returns one value:

```rust
fn withdraw(ctx: &withdraw::Ctx, op: Withdraw) -> Result<Outcome, WithdrawRefused>
fn end_of_day(ctx: &end_of_day::Ctx, run: Run) -> Outcome
fn credited(ctx: &credited::Ctx, c: Delivery<ledger::Entry>) -> Result<Outcome, CreditRefused>
fn quote_withdrawal(ctx: &quote_withdrawal::Ctx, q: QuoteWithdrawal) -> Result<WithdrawalQuote, QuoteRefused>
```

- **The message is only what started the hook:** what the caller sent, the
  run tempo fired, the fact that was delivered. Nothing the engine fetched
  is in it.
- **The context is everything the engine prepared,** and nothing the caller
  sent. Its type is generated per hook from `#[requires]`, so a hook cannot
  read what it did not declare: that does not compile.

| `ctx.` | Holds | From |
|---|---|---|
| `instance` | the instance's id, its definition and model versions, and its entity as the model declares it, with facets (`ctx.instance.status`, `ctx.instance.terms`) | always |
| `time` | the due, the recorded time, the business date in the definition's calendar | always |
| `caller` | the principal, its roles and channel; tempo for a run; the sender for a delivery | always |
| `params` | the parameters, resolved as of the due and typed (C8, C31) | always |
| `trace` | the operation's id, idempotency key and correlation | always |
| `views.<view>` | `view(V)`: `ctx.views.account_state` | `#[requires]` |
| `instances.<field>` | `instance(from = terms.pay_to, ..)`: `ctx.instances.pay_to`, named after the field it comes from | `#[requires]` |
| `linked.<as>` | `linked(via, .., as = offsets)`: `ctx.linked.offsets` | `#[requires]` |
| `series.<param>`, `effective.<as>` | reference data and effective records | `#[requires]` |
| `facets.<facet>` | operation facets the definition accepts ([extending the base](extending-the-base.md#5-input-on-a-base-operation-operation-facets)) | `#[requires]` |
| `ledger.balances`, `ledger.entries` | inputs contributed by an extension, under its name | `#[requires]` |

**The outcome is returned, not written to the context.** The hook stays a
pure function: a test asserts on a value; a refusal cannot leave half-built
actions behind; and the signature itself says what C1 says, that contracts
return actions and the engine writes. The outcome's builder
(`Outcome::new().book(..).act(..).emit(..)`) gives the convenience a mutable
context would.

For tests, the SDK generates a builder per context:
`withdraw::Ctx::builder().views(..).params(..).build()`, so a hook can be
unit-tested without a simulation.

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

withdraw:         func(ctx: withdraw-ctx, op: withdraw) -> result<outcome, withdraw-refused>;
end-of-day:       func(ctx: end-of-day-ctx, run: run) -> outcome;
quote-withdrawal: func(ctx: quote-withdrawal-ctx, q: quote-withdrawal) -> result<withdrawal-quote, quote-refused>;
```

An implementation, with the SDK:

```rust
use tkone_contract_sdk::prelude::*;
use easy_saver::{Action, Event, Rule, Withdraw, WithdrawRefused, end_of_day, withdraw}; // generated

#[implementation(of = easy_saver)]
impl easy_saver::Contract for Saver {
    #[requires(view(AccountState))]
    fn withdraw(ctx: &withdraw::Ctx, op: Withdraw) -> Result<Outcome, WithdrawRefused> {
        let available = ctx.views.account_state.available;
        if op.amount > available {
            return Err(WithdrawRefused::InsufficientFunds { available });
        }
        Ok(Outcome::new().book(booking! {
            code:   BankTransactionCode::CASH_WITHDRAWAL,
            debit:  (This, Available, op.amount),
            credit: ("customer_withdrawals", op.amount),   // logical, mapped (C17)
        }))
    }

    #[requires(view(AccountState), ledger::balances(as_of = due))]
    fn end_of_day(ctx: &end_of_day::Ctx, run: Run) -> Outcome {
        let (p, balances) = (&ctx.params, &ctx.ledger.balances);
        let mut out = Outcome::new();
        if run.is_for(Rule::Accrue) {
            out = out.book(accrual(balances, p.RATE, run.due()));
        }
        if run.is_for(Rule::Capitalise) {
            let interest = balances.accrued.round(p.ROUNDING);
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
| **3. Extensions** | actions whose meaning needs code: the ledger's net-to-zero bookings | Rust crates compiled into the engine build | the extension | open, but rare and trusted |

The engine knows nothing about money. Banking is the core model's declared
actions plus the ledger extension; a metering, loyalty or insurance product
is its own models, with or without an extension of its own. Quantities,
guards and delivery, which make busy instances cheap, are the kernel's, not
the ledger's ([hot instances](hot-accounts.md)).

### 1. Kernel primitives

| Primitive | Applies to | Writes |
|---|---|---|
| `Append` | a `#[fact]` type | a fact in the home journal |
| `Set` | an `#[effective]` attribute | a new version, effective from a date |
| `Create`, `End` | an `#[effective]` entity: something that starts and stops | the entity, and its end |
| `Transition` | a state machine declared in a model | the state, effective from the due |
| `Deliver` | a fact for another instance | an insert into its inbox; pending if it writes a quantity in a direction guarded with acceptance ([hot instances](hot-accounts.md)) |
| `ScheduleChange` | the instance's tempo set | its next version (D12) |

These mirror what the store can do (C12, C13), which is why they rarely
change. `Emit`, for events, sits beside them in the outcome. Beside the
primitives, the kernel owns the mechanisms they rely on: **quantities**
folded from facts, **guards** on them, inboxes sharded and folded at each
period's close, **compensation** of refused deliveries, and the rare
**joint** commit across two instances
([hot instances](hot-accounts.md)). None of them knows a domain.

### 2. Declared actions

A model names its actions: which primitive each is, on which of its types,
and the **invariants** it must keep. Invariants are
[CEL](https://cel.dev) expressions over the action's payload, its parameters
and the instance's snapshot, evaluated by the engine at commit; a failed one
is a typed rejection.

```rust
#[model(name = "core", extensions = [ledger])]   // 1.5.0
mod core {
    #[effective(key = account_id)]
    #[action(create = Hold, end = Release)]
    #[invariant("amount.currency == state.currency")]
    #[invariant("amount <= state.available", reject = "hold-exceeds-available")]
    struct Hold { account_id: AccountId, balance: BalanceType, amount: Money,
                  until: Option<NaiveDate>, reason: HoldReason }

    #[state_machine(Account.status, initial = Pending)]
    transitions! { Pending -> Active | Closed, Active -> Dormant | Closed, Dormant -> Active | Closed }

    /// Validated against the parameter's own declaration: type, ranges,
    /// invariants and levels (C31).
    #[action(set = SetParameter, validate = parameter)]
    struct ParameterOverride;

    #[fact(key = booking_id)]
    #[action(extension = ledger::book, reverse = ledger::reverse)]   // tier 3
    struct Booking { /* below */ }
}
```

The same mechanism, with no banking in it:

```rust
#[model(name = "metering")]   // 1.0.0
mod metering {
    #[fact(key = subscription_id)]
    #[action(append = RecordUsage)]
    #[invariant("quantity > 0")]
    #[invariant("at <= due")]
    struct Usage { subscription_id: SubscriptionId, meter: MeterCode,
                   quantity: Quantity<KilowattHour>, at: DateTime }

    #[effective(key = subscription_id)]
    #[action(set = ChangeTariff)]
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

Some meanings cannot be a primitive plus invariants: a booking's entries
must net to zero per currency, its amounts must fit each currency's minor
units, and its logical accounts must resolve through the accounting mapping
as of the due. That is an **extension**, compiled into the engine build.

An extension brings **vocabulary**; the kernel keeps every **mechanism**.
So the trait lets an extension contribute everything a model author or a
hook would otherwise find hard-coded in the engine:

```rust
pub trait Extension {
    const NAME: &'static str;                    // "ledger"
    const VERSION: Version;

    /// The actions it gives models, with their payload types.
    fn actions(&self) -> &[ActionSpec];          // ledger.book, ledger.reverse
    /// The inputs hooks may require, resolved by the kernel's fetchers.
    fn inputs(&self) -> &[InputSpec];            // ledger::balances(as_of), ledger::entries(since, until, max)
    /// Model and definition attributes, expanded to kernel declarations.
    fn attributes(&self) -> &[AttrSpec];         // #[ledger::balances] -> #[quantity]; #[ledger(guard)] -> #[guard]
    /// Types and functions for CEL invariants and guards.
    fn cel(&self) -> CelLibrary;                 // Money, Rate, zero(currency), currency arithmetic
    /// State of its own, beside the kernel's, if it needs any.
    fn state(&self) -> &[StateSpec];             // the ledger: none; checkpoints are the kernel's

    /// Checks against the snapshot; a violation is a typed rejection.
    fn validate(&self, action: &ExtAction, snapshot: &Snapshot) -> Result<(), Violation>;
    /// The writes, in kernel primitives: facts per instance; the kernel picks home and deliveries.
    fn plan(&self, action: &ExtAction, snapshot: &Snapshot) -> Vec<Primitive>;
    /// The compensation for a delivered fact that was refused.
    fn compensate(&self, fact: &Fact, refusal: &Refusal) -> Vec<Primitive>;   // a return booking
    /// Its own checks at a period's close, after the kernel's.
    fn close(&self, period: Period, store: &dyn ReadStore) -> CloseReport;   // trial balance
}
```

- **Extensions plan; the engine writes.** An extension turns its action into
  kernel primitives, so storage, journals, views and events stay the
  engine's, on every store. Guards are checked by the kernel on what the
  plan writes, not by the extension.
- **Vocabulary is namespaced.** Everything an extension contributes is
  named with it (`ledger::balances`, `#[ledger::balances]`,
  `ledger::Entry`), so a model or hook shows which extension it depends on,
  and a kernel name never means money.
- **Required by models, checked at registration.** A model declares the
  extensions it needs (`extensions = [ledger]`); applying it to an
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
| `ledger.book` | entries net to zero per currency; amounts in the currency's minor units (or a balance type's declared scale); logical accounts resolve as of the due; `Input` accounts were inputs | the booking, and each account's entries as facts for that instance; the kernel homes it and delivers the rest by their guards (C22) |
| `ledger.reverse` | the booking is this instance's and not already reversed | the same, with opposite entries, naming the first |

That is the whole of the ledger. A guarded debit leaving a balance beyond
its limit is refused by the **kernel's** guard, whatever the hook thought:
the hook's check is product logic; the guard is an invariant, in reviewed
code, and the same for points, stock or any other quantity.

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
| **Action events** | the model or extension that declares the action | by the engine, for every committed action, whatever the contract does | `ledger.booking-committed`, `core.hold-created`, `core.hold-ended`, `core.account-status-changed`, `metering.usage-recorded`; and the engine's own: `TermsChanged` (a `commercial` parameter changed), `ScheduleChanged`, `InstanceUpgraded`, `IdentifierIssued`, `PartyMerged`, `SubjectErased` |
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
  "created": { "ledger::Booking": ["bkg_01JD…"] },
  "result": { "available": { "minor": "120000", "currency": "GBP" }, "holds": [] }
}
```

- **Exact as of the commit.** Only `with-instance` views may be selected; the
  engine holds their new values already, having written them in the commit,
  so it reads nothing more. `model plan` refuses a selection of an `eventual`
  view.
- **Typed per operation, with no code.** It comes from views, so it cannot
  disagree with what a read returns a moment later.
- **The receipt is always there:** the instance's new version and the ids
  of the facts it created, by type (here, the ledger's bookings), whether or
  not `returns` is declared.

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
POST /instances/acc_01JD3K…/queries/quote-withdrawal
{ "amount": { "minor": "500000", "currency": "GBP" }, "on": "2026-12-01" }

200 OK
{ "penalty": { "minor": "2740", "currency": "GBP" }, "available_on": "2027-02-28" }
```

A query runs the same pipeline (entitle, resolve, parameters, fetch, invoke)
and stops there: no validation of actions, no commit. The engine refuses an
outcome from a query hook; the generated WIT gives it no way to return one.
Queries are part of the interface's schema, with the same rules as
operations.
