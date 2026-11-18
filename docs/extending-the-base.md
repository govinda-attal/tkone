# Extending the base — Design decisions

Status: **architecture only**, like the [contract engine](contract-engine.md)
it belongs to. The code sketches are illustrative, not APIs.

The platform ships a **base**: the [core domain](core-domain.md), domain
models (`deposits`, `term-deposits`, `lending`), interfaces
(`deposit-account@1`), and products built on them (`easy-saver`, `fixed-12m`,
`personal-loan`). An **adopter** (a bank running the platform) will want
more: fields the base does not have, operations it does not offer, events
its own systems listen for, and products that behave a little differently.

This document decides how they get them without forking the base, and
without the base's next release breaking what they built. Each decision is
shown with what an adopter writes, then stated as a decision (C33 to C39)
at the end.

---

## The running example

An adopter wants a **goal saver**: an instant-access savings account where
the customer sets a target and a date, earns a bonus rate in any month they
stay on track, and keeps the bonus after a withdrawal they mark as an
emergency. They also want:

- a new channel, `branch-kiosk`, on every operation;
- every withdrawal recorded with a purpose and the channel's session, for
  their fraud team;
- a `GoalReached` event for their notifications service;
- the base's daily interest, exactly as `easy-saver` computes it;
- `tax_residency` required on every holder of a goal saver.

Nothing in the base knows about goals. Every one of these is added in the
adopter's own crates.

## Who owns what

```text
            base (the platform's)                    adopter's (namespace `bank`)
  ┌──────────────────────────────────┐      ┌──────────────────────────────────────┐
  │ tkone::core          1.5.0       │◄─────│ bank::savings_goals  model     1.0.0 │
  │ tkone::deposits      3.2.0       │◄─────│ bank:savings-goals   interface 1.0.0 │
  │ deposit-account      1.0.0  (if) │◄─────│ bank:goal-saver      definition 1.0.0│
  │ easy-saver           2.1.0  (def)│      │ goal-saver impl  (calls the crate ↓) │
  │ tkone-products-deposits   crate  │◄─────│                                      │
  └──────────────────────────────────┘      └──────────────────────────────────────┘
          arrows point one way: the adopter depends on the base, never the reverse
```

- **A namespace has one owner.** `tkone::*` is the platform's. An adopter
  registers its own (`bank`) and declares everything there: models,
  interfaces, definitions, parameter types, approval roles. Nobody else can
  publish into it.
- **Each namespace has its own `model.lock`** and its own space of field ids
  ([rule 1](contract-models.md)). An adopter's field 7 and the base's field
  7 are different fields; they cannot collide.
- **Facets are stored apart.** An adopter's facet on `core::Account` is its
  own table keyed by `account_id`, so a base migration never touches it and
  an adopter migration never touches the base's tables.
- **Forking is explicit.** An adopter may copy a base model into its own
  namespace (`tkone model fork deposits@3.2.0 --into bank`). From then on it
  is theirs: base releases no longer apply to it, and the CLI says so on
  every `plan`. It is the escape hatch, not the way to extend.

```text
$ tkone model fork deposits@3.2.0 --into bank
forked tkone::deposits 3.2.0 -> bank::deposits 1.0.0
  ! bank::deposits no longer receives base releases of tkone::deposits
  ! 3 base definitions use tkone::deposits; they are not moved
```

## What an adopter can add

In the order to reach for them:

| Adopter wants | Mechanism | Example below |
|---|---|---|
| Fields on a base entity | a **facet** in their namespace | [goal terms](#1-fields-a-facet-on-a-base-entity) |
| Values in a code set | reference data, for **open** code sets | [a new channel](#2-values-open-code-sets) |
| Fields required for their products | a **profile** on their definition | [tax residency](#3-requiring-what-the-base-leaves-optional-profiles) |
| Operations, queries, events | **their own interface**, implemented beside the base's | [savings goals](#4-operations-and-events-an-interface-of-their-own) |
| Input on a base operation | an **operation facet** | [withdrawal purpose](#5-input-on-a-base-operation-operation-facets) |
| Behaviour | **their own implementation**, reusing the base's product crate | [goal saver](#6-behaviour-reuse-by-library-not-override) |
| Reads and APIs | views and an `api!` of their own | [goal progress](#7-reads-and-apis) |

### 1. Fields: a facet on a base entity

Facets are how base domains extend the core already
([C15](contract-engine.md#c15-the-core-is-small-and-extended-never-redefined)).
An adopter uses the same macro, in their own model:

```rust
// src/lib.rs of the model crate
tkone::model!(name = "bank::savings-goals");   // 1.0.0; Cargo.toml: tkone-core = "1.5", tkone-deposits = "3.2"

// its items, in whichever of the crate's modules suit them
/// A goal saver's target. One to one with the account.
#[extends(core::Account, when = "definition = 'bank:goal-saver'")]
#[action(set = SetGoal)]
#[invariant("target.currency == account.currency")]
#[invariant("by > account.opened_on", reject = "goal-in-the-past")]
pub struct GoalTerms {
    #[effective] target: Money,
    #[effective] by: NaiveDate,
}

#[ledger::balances]
pub enum GoalBalance { #[ledger::balance(scale = 12)] BonusAccrued }
```

- The facet's fields have ids in `bank::savings-goals`'s lock, grow by its
  rules, and are classified like any other field (`#[personal]` works).
- `when` may name anything the account carries, including its definition, so
  an adopter's facet appears only on its products.
- Facets on `core::Party` and on base facets (`deposits::DepositTerms`) work
  the same way.

### 2. Values: open code sets

The base already separates closed code sets (enums in the model, which the
engine and the base's code rely on) from open ones (reference data, which
banks add to)
([values](contract-engine.md#values-constants-code-sets-parameters-and-reference-data)).
An adopter adds to an open one with data, under maker and checker:

```text
$ tkone reference load channels --add branch-kiosk "Branch self-service kiosk"
pending approval: channels +1 (branch-kiosk), maker: a.ng
```

Every operation that takes a `Channel` now accepts `branch-kiosk`. A closed
code set, such as balance types or account statuses, cannot be added to; see
[what stays closed](#what-stays-closed).

### 3. Requiring what the base leaves optional: profiles

`core::Party.tax_residency` is optional in the base
([core domain](core-domain.md#core)), because not every product needs it. An adopter cannot make it required for everyone: that would
break every other product and every caller built for the base (rule 2). They
narrow it **for their products**:

```rust
// src/lib.rs of bank:goal-saver
#[definition(name = "bank:goal-saver", /* … */)]
/// Checked by the engine when the account opens and whenever a holder is
/// added or changes: never by hook code.
#[profile(core::Party, role = Role::Holder, require = [tax_residency])]
#[profile(core::Account, require_identifier = [reference!("sort-code-account")])]
pub struct GoalSaver;
```

```text
POST /accounts            (bank:goal-saver, holder pty_01J…)
422 { "rejected": "profile-not-met",
      "profile": "bank:goal-saver/core::Party(Holder)", "missing": ["tax_residency"] }
```

A profile only removes options; it never adds fields (that is a facet) or
widens a type.

### 4. Operations and events: an interface of their own

An adopter cannot add an operation to `deposit-account@1`: the base owns it,
and every implementation of it would have to handle the new operation.
They declare an interface of their own and implement both:

```rust
// interfaces/savings-goals (crate savings_goals_if): src/lib.rs
#[interface(name = "bank:savings-goals", model = savings_goals,       // 1.0.0
            operations = [SetGoal, QueryGoalProgress])]
pub struct SavingsGoals;

// src/operations.rs
#[operation(returns = { target: GoalTerms.target, by: GoalTerms.by })]
pub struct SetGoal { pub target: Money, pub by: NaiveDate }

#[rejections(SetGoal)]
pub enum SetGoalRefused { GoalInThePast, BelowBalance { balance: Money } }

#[query(returns = GoalProgress)]
pub struct QueryGoalProgress { pub on: NaiveDate }

#[event] pub struct GoalReached { pub target: Money, pub on: NaiveDate }
#[event] pub struct BonusForfeited { pub month: YearMonth }
```

```rust
// products/goal-saver: src/lib.rs
#[definition(name = "bank:goal-saver", model = savings_goals,          // 1.0.0
             implements = [deposit_account, savings_goals_if],
             parameters = params::Params,
             hooks(scheduled = end_of_day, activation = open, closure = close),
             schedules = schedules::Rules)]
#[profile(core::Party, role = Role::Holder, require = [tax_residency])]
#[accepts(WithdrawalNote, required)]
pub struct GoalSaver;

permits! { Book, Reverse, Hold, Release, SetGoal,
           Transition(Active -> Dormant | Closed), ScheduleChange }

// src/params.rs
#[parameters]
impl Params {
    pub const RATE: Parameter<Rate> = Parameter::commercial()
        .definition(rate!(0.0300))
        .instance(Instance::optional())
        .range(rate!(0), rate!(0.15));
    pub const BONUS: Parameter<Rate> = Parameter::commercial()
        .definition(rate!(0.0100))
        .range(rate!(0), rate!(0.05));
}

// src/schedules.rs
schedules! {
    time_zone: "Europe/London", calendar: "GBLO", start: 2026-01-01,
    accrue:      "YY-MM-DD~SBT18:00:00",
    capitalise:  "YY-MM-L~PBT18:00:00",
    bonus_check: "YY-MM-L~PBT17:00:00",
}
```

A channel built for `deposit-account@1` deposits into a goal saver without
knowing what it is. The adopter's own app sends `SetGoal` with
`Tkone-Interface: bank:savings-goals@^1`. Events from both interfaces go out
in the same envelope, ordered per account, so `GoalReached` arrives after
the deposit that caused it, with that deposit's operation as its cause
([events](contract-hooks.md#events)).

### 5. Input on a base operation: operation facets

The fraud team wants every withdrawal to carry a purpose and the channel's
session. `Withdraw` is the base's, so the adopter cannot add fields to it.
They declare an **operation facet**: typed input that travels with a base
operation, allowed where the base declared the operation
`#[accepts_facets]` ([what the base owes](#what-the-base-owes-adopters)).

```rust
// in bank::savings-goals
#[extends(deposit_account::Withdraw)]
struct WithdrawalNote {
    purpose: WithdrawalPurpose,                  // Everyday | Emergency | Transfer
    #[personal(subject = "holder")] session_ref: Option<Text>,
}
```

The definition says whether its products accept it, or require it:

```rust
#[definition(name = "bank:goal-saver", /* … */)]
#[accepts(WithdrawalNote, required)]
pub struct GoalSaver;
```

On the engine's API it is named, and checked against its schema like the
operation itself. It is never a bag of free attributes:

```text
POST /instances/acc_01J…/operations/withdraw
Tkone-Interface: deposit-account@^1
{ "amount": { "minor": "40000", "currency": "GBP" }, "channel": "branch-kiosk",
  "facets": { "bank:withdrawal-note@1": { "purpose": "emergency", "session_ref": "k-88412" } } }
```

- **It is stored with the operation** in the journal, so it can be audited,
  indexed by a view, and erased by key like any personal field.
- **Only code that declares it reads it.** The adopter's implementation asks
  for it in `#[requires(facet(WithdrawalNote))]` and gets a typed value in
  `ctx.facets.withdrawal_note`. The
  base's `easy-saver` cannot name it, so it is never given it.
- **It cannot change what the base's code does.** That is the point of it,
  and the reason it is safe: an adopter's input riding on a base product's
  operation is a record, not a switch. A request to make a base
  implementation behave differently on it is a request for its own
  implementation (6).
- **A base product sent an operation facet it does not accept** answers
  `422 facet-not-accepted`, naming the facet, rather than dropping it.

### 6. Behaviour: reuse by library, not override

The goal saver should compute daily interest exactly as `easy-saver` does,
and add a bonus. There are three ways an engine could allow this; the design
takes one.

| Way | Why |
|---|---|
| Interceptors or overrides on the base's hooks ("before `end_of_day`, also …") | **Rejected.** It brings back what removing pre and post hooks removed: hidden ordering, two pieces of code sharing one outcome, and a base release changing an adopter's product without either noticing |
| A new implementation of the base's definition | **Allowed,** when only behaviour changes and the contract does not: it is registered and promoted like any implementation ([C7](contract-engine.md#c7-implementation-versions-four-states-and-a-default)), under the base's definition, with its operations, parameters and schedules |
| An adopter's own implementation of their own definition, calling the base's product code as a library | **The way to extend behaviour.** The adopter owns the outcome; the base's logic is pinned by crate version and tested by the same simulation |

Every base product ships twice: as a signed component, and as a Rust crate
of the pure functions it is made of. `easy-saver`'s own component is a thin
layer over the crate:

```rust
// tkone-products-deposits, published by the platform
pub mod interest {
    /// Daily accrual on a balance, as easy-saver books it. Plain values in,
    /// plain values out: no generated context types, no I/O.
    pub fn daily_accrual(balance: Money, rate: Rate, day: NaiveDate,
                         basis: DayCount) -> Unrounded<Money>;

    /// The booking that capitalises what has accrued.
    pub fn capitalise(accrued: Unrounded<Money>, rounding: Rounding,
                      code: BankTransactionCode) -> Option<Booking>;
}
```

The adopter's implementation calls it, and adds its own:

```rust
use tkone_products_deposits::interest;          // = 2.1, pinned in Cargo.lock
use goal_saver::{hooks, *};

#[implementation(of = goal_saver)]
pub struct GoalSaverImpl;

#[requires(view(SavingsState), ledger::balances(as_of = due))]
impl hooks::EndOfDay for GoalSaverImpl {
    fn end_of_day(ctx: &Self::Ctx, run: Run) -> Outcome {
        let (p, available) = (&ctx.params, ctx.ledger.balances.available);
        let mut out = Outcome::new();
        if run.is_for(Rule::Accrue) {
            let base = interest::daily_accrual(available, p.RATE, run.due().date(), DayCount::Act365);
            out = out.book(accrue(Accrued, base));                 // the base's interest, unchanged
            if on_track(&ctx.instance.goal_terms, available, run.due()) {   // the adopter's facet
                let bonus = interest::daily_accrual(available, p.BONUS, run.due().date(), DayCount::Act365);
                out = out.book(accrue(BonusAccrued, bonus));       // the adopter's bonus
            }
        }
        // capitalise, bonus_check, GoalReached …
        out
    }
}

#[requires(view(SavingsState), facet(WithdrawalNote))]
impl hooks::Withdraw for GoalSaverImpl {
    fn withdraw(ctx: &Self::Ctx, op: Withdraw) -> Result<Outcome, WithdrawRefused> {
        let available = ctx.views.savings_state.available;
        let mut out = tkone_products_deposits::withdraw::plan(op.amount, available)?;
        if ctx.facets.withdrawal_note.purpose != WithdrawalPurpose::Emergency {
            out = out.emit(BonusForfeited { month: ctx.time.business_date.month() });
        }
        Ok(out)
    }
}
```

The crate's functions take plain domain values (`Money`, `Rate`, dates),
never a product's generated context types, so they can be called from any
implementation whatever its `#[requires]`.

### 7. Reads and APIs

Views over base entities and adopter facets are declared in the adopter's
model, in the same four forms ([views](contract-models.md)):

```rust
#[view(key = account_id, consistency = "with-instance",
       compose = [deposits::SavingsState, GoalTerms],
       derive = { remaining: "max(target - available, zero(currency))",
                  bonus_accrued: "core::Balances[BonusAccrued]" })]
struct GoalProgressView;
```

And the adopter's API names what it exposes, from both namespaces, in the
`api!` declaration. Its version is the adopter's, not the base's:

```rust
#[api(name = "bank:savings", version = "1")]
pub struct SavingsApi;

expose! {
    get  "/goal-savers/{account}"          => view(GoalProgressView),
    post "/goal-savers/{account}/goal"     => operation(savings_goals_if::SetGoal),
    post "/goal-savers/{account}/withdraw" => operation(deposit_account::Withdraw)
                                              with(WithdrawalNote),    // flattened into the body
    events "bank.savings.v1"               => events(savings_goals_if::*, deposit_account::*),
}
```

---

## What stays closed

Each of these would let an adopter break the base's code or the base's
callers. Each has a way to get what was wanted:

| Not allowed | Because | Instead |
|---|---|---|
| Change a base field's type, or remove one | every base implementation and caller is built against it | a facet with the field as wanted |
| Make a base optional field required for everyone | breaks other products and old callers (rule 2) | a profile on their definitions (3) |
| Add a state to a base state machine | base implementations match on the states exhaustively | a sub-state in their own facet, with its own machine (below) |
| Add a variant to a closed code set or a sealed enum | the same; balance types and statuses are relied on | an open code set, if the base declares one; or their own enum in a facet |
| Add operations, rejections or events to a base interface | every implementation of it would have to handle them | their own interface (4) |
| Add fields to a base event | the base's code emits it, and could not fill them | their own event, correlated through the envelope |
| Override or intercept a base hook | hidden ordering, shared outcomes (6) | their own implementation, reusing the base's crate |
| Add fields at run time, without code | entity-attribute-value tables escape every model rule | a facet in a model crate ([decided](contract-models.md#decided-so-they-are-not-reopened-by-accident)) |
| Write an engine extension (tier 3) | extensions run in the engine's process, trusted | a declared action on kernel primitives; an extension only with the platform |

A sub-state instead of a new state, for an adopter whose collections team
wants loans in a repayment plan:

```rust
#[extends(lending::LoanTerms, when = "collection = 'Arrears'")]
struct RepaymentPlan { #[effective] plan: Option<PlanStatus> }   // Proposed | Agreed | Broken

#[state_machine(RepaymentPlan.plan, initial = Proposed)]
transitions! { Proposed -> Agreed | Broken, Agreed -> Broken }
```

The base still sees `Arrears`, and its code still works. The adopter's view
of a loan is `Arrears / Agreed`.

---

## What the base owes adopters

Being extensible is a promise the base keeps, release after release.

**Extension points are declared.** Anything not declared is closed:

```rust
// src/lib.rs of the model crate
tkone::model!(name = "tkone::core");   // 1.5.0

// its items, in whichever of the crate's modules suit them
#[code_set(open)]   pub enum ProductLine;            // reference data adds to it
#[code_set(closed)] pub enum AccountStatus { … }     // the engine relies on it
#[sealed]           pub enum Role { … }              // the default; stated where it matters

// in the deposit-account interface crate
#[operation(returns = { … })]
#[accepts_facets]                                    // operation facets allowed
pub struct Withdraw { pub amount: Money, pub channel: Channel }
```

**The product crates are a supported API.** Publishing `easy-saver`'s logic
as a crate makes its function signatures a compatibility surface, like the
schemas. They follow semver: a minor release adds functions and never
changes what an existing one returns for the same inputs; a change in a
computed result (a rounding fix) is a major release, with a note naming the
products it changes.

**Two majors are supported at once.** When `deposits` goes to 4.0, 3.x
keeps receiving fixes for a declared window, so adopters move when their own
facets and implementations are ready.

**Every base release can be checked against an adopter's distribution**
before it is applied:

```text
$ tkone base check tkone::deposits@3.3.0
distribution bank (locked: core 1.5.0, deposits 3.2.0, bank::savings-goals 1.0.0)
  compatible     bank::savings-goals 1.0.0   facets on DepositTerms: unchanged fields
  warning        bank::savings-goals 1.0.0   GoalTerms.target overlaps deposits::SavingsGoal.target (new in 3.3.0)
  compatible     bank:goal-saver 1.0.0       tkone-products-deposits 2.1 -> 2.2: additive
  plan           3 tables altered (base), 0 adopter tables touched
```

## Upgrading the base

What runs is a **distribution**: the base's models, interfaces and product
crates at pinned versions, plus the adopter's, locked together. Registration
checks the pair, not each side alone.

```toml
# distribution.lock: generated from Cargo.lock, with the schema digests Cargo does not know
[base]
"tkone::core"      = { version = "1.5.0", digest = "sha256:4be1…" }
"tkone::deposits"  = { version = "3.2.0", digest = "sha256:91ac…" }
"deposit-account"  = { version = "1.0.0", digest = "sha256:0d7e…" }

[adopter.bank]
"bank::savings-goals" = { version = "1.0.0", digest = "sha256:c3f2…", requires = ["tkone::core@^1.5", "tkone::deposits@^3.2"] }
"bank:savings-goals"  = { version = "1.0.0", digest = "sha256:7a10…" }
```

**When the base catches up with an adopter.** `deposits` 3.3 adds a
`SavingsGoal` facet with a `target`, the same idea as the adopter's
`GoalTerms.target`. Keeping both forever means two truths. The adopter, who
knows both sides, declares the move in their next model version; the base
never names an adopter's fields:

```rust
// src/lib.rs of the model crate
tkone::model!(name = "bank::savings-goals");   // 1.1.0; Cargo.toml: tkone-deposits = "3.3"

// its items, in whichever of the crate's modules suit them
#[extends(core::Account, when = "definition = 'bank:goal-saver'")]
pub struct GoalTerms {
    #[superseded_by(deposits::SavingsGoal.target, since = "1.1.0")]
    #[effective] target: Money,
    #[effective] by: NaiveDate,
}
```

```text
$ tkone plan bank::savings-goals@1.1.0
  backfill   deposits::SavingsGoal.target  from GoalTerms.target  (18,204 accounts, effective history kept)
  redirect   reads of GoalTerms.target -> deposits::SavingsGoal.target
  deprecate  GoalTerms.target (rule 2: kept, no longer written)
```

The field keeps its id and its history; implementations built for 1.0 still
read it, served from the base's field.

---

## Decisions

### C33. Namespaces have owners; adopters depend on the base, never the reverse

Everything the platform ships is in `tkone::*`; an adopter publishes in its
own namespace, with its own `model.lock` and field-id space, and its facets
in their own tables. The base never names an adopter's types. Forking a base
model into an adopter's namespace is allowed, explicit, and ends base
releases for it ([who owns what](#who-owns-what)).

### C34. Adopters extend data with facets, open code sets and profiles

New fields are facets on base entities, in the adopter's model; new values
go into open code sets as reference data; fields the base leaves optional
are required for the adopter's products by profiles on their definitions,
checked by the engine. Base types, closed code sets and state machines are
not changed by adopters ([1](#1-fields-a-facet-on-a-base-entity),
[2](#2-values-open-code-sets),
[3](#3-requiring-what-the-base-leaves-optional-profiles)).

### C35. New operations and events come in the adopter's own interfaces

An adopter's product implements base interfaces and its own side by side.
Base interfaces are sealed per version; their operations, rejections and
events grow only by the platform's releases. Adopter events are correlated
with base events through the envelope
([4](#4-operations-and-events-an-interface-of-their-own)).

### C36. Operation facets carry adopter input on base operations, and change nothing in base code

An operation facet is typed, named, schema-checked input attached to a base
operation the base declared `#[accepts_facets]`. A definition accepts or
requires it; it is stored with the operation; only implementations that
declare it in `#[requires]` read it. Base implementations are never given
it, so it cannot change their behaviour
([5](#5-input-on-a-base-operation-operation-facets)).

### C37. Behaviour is reused by library, never overridden

There are no interceptors or overrides on base hooks. An adopter changes
behaviour with a new implementation of a base definition, when the contract
is unchanged, or with its own definition and implementation, calling the
base's product crates: pure functions over plain domain values, published
beside each component and supported under semver
([6](#6-behaviour-reuse-by-library-not-override)).

### C38. The base declares its extension points; what is undeclared is closed

Open code sets, operations that accept facets and entities that may be
extended are declared in base models. Base models, interfaces and product
crates follow semver, with two majors supported for a declared window; a
change to what a product computes is a major release
([what the base owes](#what-the-base-owes-adopters)).

### C39. What runs is a distribution, upgraded and checked as one

The base's and the adopter's versions are locked together in a distribution;
registration checks the pair, and `tkone base check` checks a base release
against it before it is applied. When the base adds what an adopter already
holds, the adopter declares `#[superseded_by]`, and `plan` backfills, redirects
reads and deprecates their field ([upgrading the base](#upgrading-the-base)).

---

## Open questions

- **Facets without Rust.** Requiring code for every extension (C34) is right
  for behaviour and may be wrong for "one more field for a report". If that
  demand appears, the answer is a narrow, declarative facet authored in the
  admin UI that compiles to the same schema (typed, bounded, classified),
  never key-value attributes. Not in v1.
- **Operation facets that influence base products.** Adopters will ask for
  them. The answer here is no (C36); if real cases accumulate, the base
  should grow parameters or operations for them, not read adopter facets.
- **Base product crates in languages other than Rust.** Reuse by library
  (C37) assumes the adopter writes Rust. A component-level composition (WIT
  imports of the base's functions) would serve other languages; it waits for
  a second language.
- **Hosted or self-hosted.** On a hosted platform the operator can run
  `base check` against every adopter's distribution before a release; a
  self-hosted adopter runs it themselves. The release policy (C38) holds in
  both; who runs the check differs.
