# Contract engine — Design

Status: **architecture only**. Nothing here is built. It describes the crates
and apps that would run a core banking product (a savings account, a loan) as
versioned contracts over versioned models, with [tempo](schedule-document.md)
running their schedules. The code sketches are illustrative, not APIs.

## Motivation

A banking product is mostly rules about an account's life: what may be
deposited, how interest accrues and when it is capitalised, what a repayment
holiday does, how the account closes. Those rules change by product, by
version, and sometimes by account. They should be:

- **written by people who know the product,** against a model of the domain,
  without writing to the database themselves;
- **fast,** because the data they need is computed before they run;
- **versioned and auditable:** which code, with which values, decided this
  posting, and the same answer on replay;
- **scheduled** by tempo, with the account's own dates.

The paradigm is a *smart contract* without a blockchain. A contract is a pure
function: given an operation, the account's prepared data and its parameters,
it returns the **actions** it wants. The engine checks them, and writes them
to the entities, the ledger and every view that depends on them.

The paradigm is proven: products as versioned contracts, accounts bound to a
version, hooks that receive the data they declared and return what they want
done instead of writing. Its usual weak points, the upgrade of live accounts
and the limits on what a hook can see, are decided here on purpose (C9, C5).

---

## Layers

| Layer | Owns | Lives in | Made available by |
|---|---|---|---|
| **Model** | Entities, relations, views (read shapes), invariants, with a version | Rust crates | `tkone model apply`: DDL, and a row in the model registry |
| **Definition** | A product's interface: operations, hooks, permitted actions, parameters and defaults, schedules, the model version it needs | Rust crates (and a WIT package) | registration: its defaults, schedules and version persisted |
| **Implementation** | Code for one definition version: the hooks, their data requirements, its own parameter values and schedules | a WASM component in an OCI registry, signed | registration by digest, then promotion (C7) |
| **Instance** | One account bound to an implementation: its overrides, its state, its schedule instances | the database | the engine, when the account is opened |
| **Engine** | Running hooks: resolve, fetch, invoke, validate, commit | `tkone-engine`, an app | — |

Models and definitions have no execution of their own: they are crates
because code is where types and checks belong, and they are persisted because
the engine and the implementations need their versions and defaults at run
time. Implementations execute, so they are artifacts, pinned by digest.

```text
 crates                     database (registries)               OCI registry
┌───────────────┐  apply   ┌────────────────────────────────┐
│ model vN      │────────► │ model versions, applied DDL     │
├───────────────┤ register │ definitions: parameters,        │
│ definition vN │────────► │   defaults, schedules (tempo)   │
└───────────────┘          │ implementations: digest, state, │ ◄─── wasm component,
                           │   values, manifest              │      signed, by digest
                           │ instances: binding, overrides   │
                           └───────────────┬────────────────┘
      operations, bulk ───────►      ENGINE      ◄─────── tempo: due runs, with sources
                         resolve → fetch → invoke → validate → commit
```

---

## Models

A model crate declares entities, their relations, and **views**: read shapes
keyed for the way hooks and APIs read them, as a ScyllaDB table is designed
per query.

### A core, and domains that extend it

Some types belong to no single product: a party, an account, the ledger.
They live in one **core** model, small and stable. Domain models (deposits,
lending, cards) depend on a range of its versions and **extend** its
entities; they never redefine them (C15).

```text
core
├── party       Party (person or organisation), PartyRole, contact points
├── account     Account, AccountPartyRole (holder, joint holder, guarantor),
│               status, product binding
├── ledger      LedgerAccount (chart of accounts), Posting, balance addresses
└── reference   currencies, calendars, rate indices, rate cards, FX
deposits  ── extends core::Account ──►  DepositTerms (notice period, access)
lending   ── extends core::Account ──►  LoanTerms (principal, term)
```

```rust
#[model(name = "core", version = "1.4.0")]
mod core {
    #[entity(key = party_id)]
    struct Party { party_id: PartyId, kind: PartyKind, #[effective] name: Name }

    #[entity(key = account_id)]
    struct Account { account_id: AccountId, opened_on: NaiveDate, #[effective] status: Status }

    /// Parties and accounts are many to many, each in a role.
    #[relation(Account <-> Party)]
    struct AccountPartyRole { #[effective] role: Role }

    #[fact(key = account_id)]
    struct Posting { account_id: AccountId, address: BalanceAddress, amount: Money }
}

#[model(name = "deposits", version = "3.1.0", requires = "core@^1.4")]
mod deposits {
    /// A facet of a core account: one to one, keyed by its id.
    #[extends(core::Account, when = "product_line = 'deposits'")]
    struct DepositTerms { #[effective] notice_days: u16 }

    /// What a hook reads about its own account, kept with every commit.
    #[view(key = account_id, consistency = "with-instance")]
    struct AccountState { account_id: AccountId, status: Status, balances: Balances, accrued: Money }

    /// Read across accounts; may lag.
    #[view(key = party_id, cluster = account_id, consistency = "eventual")]
    struct AccountsByParty { party_id: PartyId, account_id: AccountId, available: Money }
}
```

- **Party, not customer.** A customer is a role a party plays; a guarantor, a
  joint holder or an attorney is the same party in another. Accounts and
  parties are many to many, with roles.
- **One account, whatever it carries.** A deposit's terms are a facet of the
  core account, so the account keeps one identity, one status history and
  one list of parties.
- **One source, two outputs.** A model crate generates the Rust types the
  engine uses and the WIT types implementations see, so the two cannot
  drift.
- **Apply.** `tkone model apply deposits@3.1.0` checks that the core range it
  requires is applied, diffs against the applied version, emits and runs DDL,
  and records the version. Changes follow semver: adding a field or view is
  minor, removing or retyping is major and needs a migration. A major core
  version moves every domain, which is why the core stays small.
- **Views are derived, never written by contracts** (C4). The engine maintains
  each from the actions that change its sources.

### Names

Types are named in full English words (`Party`, `Account`,
`BalanceType::Available`), with the vocabulary taken from the industry's
interchange standards: IFX for parties, accounts and their relationships,
balance and account types; ISO 20022 for payments and its code sets. A
vocabulary page maps each core term and code to its equivalents, and where
messages are exchanged in those formats, serialisation renames to them. The
standards are sources of terms and codes, not schemas: their abbreviated
message names (`Acct`, `Amt`, `Dt`) and request and response structures are
not a storage model.

### Filters

Filters appear in three places, always declared, never ad hoc:

1. **What an extension or view covers:** `DepositTerms` exists where the
   product line is deposits; a view of accounts in arrears holds only those.
2. **What a hook fetches:** postings since the last capitalisation, balances
   of one type, as of the due (the manifest's `requires`).
3. **Who may see what:** see [Entitlements](#entitlements).

In a store designed per read, a filter on a column that is not a key is a
scan. So a filter that is read often is a view keyed by what it filters on:
"accounts in arrears by branch" is a view, not a query. A domain may define
filtered views over core types, but never a filter that changes what a core
entity means: the deposits model has a view of deposit accounts; it cannot
make `core::Account` mean one.

---

## Time

Banking data has two times, and they often differ:

- **Valid time:** when something is true in the world: a deposit's value
  date, the day a rate takes effect.
- **System time:** when the bank recorded it: the booking.

On 3 June a branch books £1,000 with value 1 June. The accruals of 1 and 2
June were wrong in valid time, and right as known then. Both answers are
needed: the balance on 1 June as known now drives the correction; as known on
2 June, it answers audit and replay. So the model is **bitemporal by kind**
(C13):

| Kind | Declared as | Examples | Time | Changes by |
|---|---|---|---|---|
| **Facts** | `#[fact]` | postings, operations, actions, hook calls | value time and booking time | never: appended; a correction is a reversal and a new fact |
| **Effective attributes** | `#[effective]` | status, rate, limits, roles, product binding | valid from and to, recorded at | a new version, future-dated or backdated |
| **Series** | `#[series]` | index fixings, FX, calendars | per date, versioned when restated | new points, restatements |
| **Fixed attributes** | plain fields | ids, `opened_on` | recorded at | never |
| **Derived views** | `#[view]` | balances, accrued interest, arrears | follow their sources | the engine |

Balances are not samples: they are a step function of postings, kept as
checkpoints (a closing balance per value date) and folded forward from the
nearest one.

**Authors never write temporal queries.** A hook's `requires` names what it
needs as of the due; the engine resolves it at one point and passes a plain
snapshot:

| Field | Meaning | In a catch-up of Friday's accrual on Monday |
|---|---|---|
| `due` | valid time: the business date | Friday 18:00 |
| `known_as_of` | system time: what the snapshot holds | commit N, Monday 09:02 |
| `lateness` | how late the run is | about 63 hours |

System time is the snapshot's, taken once, in commit order, and recorded with
the call; replay reads the recorded one, never the clock. Reading "as known
now" is deliberate: a fact booked on Saturday with value Friday is in Friday's
accrual first time; one with value Saturday is not. Reading "as it would have
been known on Friday" is for audit and reconstruction, not for running hooks.
Commit order, not wall clock, because engines on several nodes disagree on
the time, and Monday's call must see Friday's commit (D15).

### Backdated facts

A fact whose valid time is before results already computed makes them wrong.
The engine calls the instance's **`correction` hook** with the facts that
moved the past; it returns adjustments (an interest correction posting) as
new facts. History is never rewritten, and nothing re-runs (C14).

---

## Reference data and parameters

Values shared across products are **reference data**, not parameters:

| | Examples | Owned by | Time |
|---|---|---|---|
| **Market reference data** | SONIA, the base rate, FX, calendars | the market, a provider | series, restated in place |
| **Bank reference data** | rate cards (treasury's tiered rates), fee tariffs | treasury, pricing | effective, versioned |
| **Accounting reference data** | the chart of accounts (ledger accounts) | finance | effective |
| **Parameters** | a margin, a notice period, a negotiated rate | definition, implementation, instance (C8) | effective |

Parameters refer to reference data: `rate = card("easy-saver", tier(balance))
+ margin`, or `index("SONIA") + margin`. Ledger accounts are the bank's: a
product maps its postings to them with an **accounting mapping** ("interest
expense to 5100"), a definition or implementation parameter. A customer's
`Account` and a `LedgerAccount` are different things: an account's balances
are addressed by account, balance type and currency, and the mapping rolls
them up into ledger accounts.

---

## Entitlements

Who may do and see what, decided by the engine, never by contracts:

- **Operations:** whether this caller may invoke this operation on this
  instance: a teller may deposit; only a supervisor may waive a fee above a
  limit; a customer may withdraw from accounts where they hold a role.
- **Data:** which instances and fields a caller may read: rows filtered by
  legal entity, brand or jurisdiction; fields such as a party's tax id masked
  for most roles.
- **Lifecycle:** who may register, approve and promote a contract version
  (C7), and who may set which parameters (a rate on one account).

Entitlements are checked before a call is resolved, and applied to every read
and API. A contract does not see who called, unless a definition makes an
attribute of the caller an input (a channel: branch or online). Entitlements
are attributes and policies over the model (party roles, the account's legal
entity), so they are declared next to it, not coded in hooks.

---

## Definitions

A definition is a product's contract: what an implementation must do and may
return.

```rust
#[definition(name = "easy-saver", version = "2.0.0", model = "deposits@^3.1")]
mod easy_saver {
    #[parameter(level = "instance", default = "0.0325", min = "0", max = "0.15")]
    const RATE: Decimal;
    #[parameter(level = "implementation", default = "GBLO")]
    const CALENDAR: CalendarCode;

    #[operation] fn deposit(amount: Money);
    #[operation] fn withdraw(amount: Money);

    #[hook(pre = "withdraw")]  fn check_withdrawal;   // may reject
    #[hook(post = "deposit")]  fn after_deposit;
    #[hook(scheduled)]         fn end_of_day;         // tempo runs
    #[hook(activation)]        fn open;
    #[hook(closure)]           fn close;
    #[hook(upgrade)]           fn upgrade;            // C9
    #[hook(correction)]        fn correct;            // C14: backdated facts

    permits! { Posting, Transition(Active -> Dormant | Closed), Notify, ScheduleChange }

    schedules! { "end-of-day.yaml" }                  // a tempo definition document
}
```

Registering a definition persists its version, parameters and defaults, and
its schedule documents as tempo definitions. It never runs.

---

## Implementations

An implementation is a WASM component implementing one definition version's
WIT world, published to an OCI registry, signed, and registered by digest.
It is built with `tkone-contract-sdk` for `wasm32-wasip2`, or in another
language that targets components (componentize-py for Python).

It ships a **manifest**, read by the engine without running the code:

```yaml
implements: easy-saver@2.0.0
values:                       # its own parameter values, over the defaults
  CALENDAR: GBLO
schedules: [bonus-review.yaml] # schedules of its own, as tempo definitions
requires:                     # per hook: what to fetch before it runs
  end_of_day:
    - view: AccountState
    - balances: { as_of: due }
    - postings: { since: last(capitalise), until: due }
  check_withdrawal:
    - view: AccountState
```

The `requires` section is what makes hooks fast: the engine fetches exactly
that, in bulk, before the call, and the hook does no I/O at all (C3).

---

## The invocation pipeline

One operation on one instance:

1. **Entitle and resolve.** The caller's [entitlements](#entitlements) for
   this operation and instance; then the instance's binding: definition
   version, implementation digest, overrides. A deprecated implementation
   still runs for bound instances; an archived one never does (C7).
2. **Parameters,** resolved as of the due (C8): instance override, else
   implementation value, else definition default, with the reference data
   they refer to.
3. **Fetch** the hook's `requires` as of the due, in a snapshot taken at the
   instance's last commit (C5, C13). For a bulk call, fetches are batched.
4. **Invoke** in a fresh component instance: no I/O, the clock passed in,
   with fuel and memory limits. With wasmtime's pooling allocator,
   instantiation is microseconds; the cost is in moving the inputs, which is
   why they are prepared and small.
5. **Validate** the returned actions: permitted by the definition, valid
   against the model's invariants (a posting balances; a transition is
   allowed from the current state).
6. **Commit** per instance: one conditional append to the instance's
   journal, the actions with "only if the instance is still at version n"
   (C12). The `with-instance` views they change, the outbox (for `eventual`
   views and events) and tempo's schedule changes go with it.
7. **Project** `eventual` views from the outbox, asynchronously
   ([outbox and inbox](outbox-inbox-design.md)).

A hook may also **reject** (a pre-hook refusing a withdrawal): nothing is
committed, and the rejection, with its reason, is the result.

### Bulk

A bulk call is many invocations in one request, each committed on its own:
the result is per item, never all or nothing. Items for different instances
run in parallel; items for the same instance run in order, each seeing the
previous one's commit (C6).

---

## Actions

The set of actions is the platform's, closed, and versioned with the engine:

| Action | What the engine writes |
|---|---|
| `Posting` | balanced ledger entries; balances views |
| `Transition` | the entity's status, if the model allows it from the current one |
| `SetParameter` | an instance override, effective from a date |
| `ScheduleChange` | a new version of the instance's tempo set (D12, D13) |
| `Notify` | an outbox event for the application |
| `Reject` | nothing; the reason is returned |

A definition narrows the set (`permits!`). A contract can never return "write
this row": if it could, the engine could not keep the views true.

---

## Tempo

| Contract side | Tempo side |
|---|---|
| A definition's or implementation's schedule documents | tempo **definitions** (shared, product-wide) |
| Opening an account | its **instances**: selections with the account's dates (D11) |
| A due run | an engine call of the `scheduled` hook, with the slot's sources: `is_for("capitalise")` |
| A missed run | the instance's catch-up policy, `stale_after` (D15) |
| `ScheduleChange`, an upgrade | the next version of the account's set (D12, D13) |
| Parameters as of the due | the run acts on its due, not on now (D15) |

Several rules due at once are one run (D14): end of day accrues, then
capitalises at the month end, in one hook.

---

## Storage

The engine relies on **single-partition atomicity only**: a commit is one
conditional append to the instance's journal. Everything else is a
projection of journals. Postgres does the append with a version check in a
transaction; ScyllaDB with a lightweight transaction (`IF version = n`) on
one partition. The framework is opinionated about the shape, so the store is
a deployment choice:

- **The instance is the partition key.** Its journal, facts and effective
  attributes are clustered by valid time and recorded time: an as-of read is
  a slice of one partition.
- **Append only.** Facts are never updated; effective attributes gain
  versions.
- **A table per read**, no joins on a hook's path. Filters that are read
  often are views keyed by them.
- **Views declare their consistency.** `with-instance` views are written by
  the engine with the commit: in Postgres in the same transaction; in
  ScyllaDB right after the append, idempotently, and repaired from the
  journal if that write fails. `eventual` views are projected from the
  outbox.

Database materialized views are a storage detail the model never sees. A
ScyllaDB materialized view re-keys the same rows with a limited filter: it
fits a pure re-keying view ("accounts by party"), and with
`synchronous_updates` it can serve a `with-instance` one. It is not a
transaction (the base row can be written when the view's write fails), it
costs availability (a write fails when view replicas cannot be reached), and
it cannot aggregate or join. Balances, accruals and arrears are folds, so the
engine computes them on any store.

The exception is the ledger: a transfer debits one account and credits
another, atomically (C11). Postgres does it in one transaction; ScyllaDB has
no transaction across partitions, so there it would be a saga (a transfer
record, steps to each journal through the outbox, compensation). That cost,
paid before the product exists, is why Postgres comes first (C12).

---

## Decisions

### C1. Contracts return actions; the engine writes

Hooks never touch the database. The engine is the only writer, so every
write is checked against the definition and the model, every view stays
derivable, and a hook is a pure function that can be replayed.

### C2. Implementations are pinned by digest

Instances bind to an OCI digest, never a tag: a tag can be moved to other
code, which would make an account's history unexplainable. Signatures
(sigstore/cosign) are verified when the implementation is registered and when
it is loaded.

### C3. Hooks are deterministic

No I/O, no clock, no randomness: time is passed in, data is fetched per the
manifest. The engine records a hash of each call's inputs. Same component,
same inputs, same actions: that gives audit, replay and simulation.

### C4. Views are derived, never written

Views are projections of entities and postings, maintained by the engine
from actions. Each view declares its consistency: `with-instance` views are
written in the commit; `eventual` views are projected from the outbox.

### C5. A hook reads its own instance as of its last commit

What a hook reads about its own account is `with-instance`: committed with the
last action, so two operations cannot both see the same balance. Data about
other instances (an offset savings balance for a mortgage's interest) is a
declared, read-only input from an `eventual` view, as of a time the hook
states.

### C6. One writer per instance

Operations on one instance are serialised: by partitioning on the instance,
with the instance's version checked at commit (optimistic, retried). Different
instances never wait for each other. "Every invocation is isolated" holds
between instances, not within one.

### C7. Implementation versions: four states and a default

| State | New bindings | Calls | Notes |
|---|---|---|---|
| `draft` | test and simulation instances only | there | may be re-published |
| `live` | yes | yes | immutable and signed; may be the default |
| `deprecated` | no | for instances already bound | runs until they move or close |
| `archived` | no | no | only once no instance is bound; history stays readable |

**Default** is not a state but a pointer, per definition: the `live` version
new instances get. As a state it would allow "default and deprecated".
Moves are forward only: `draft → live → deprecated → archived`.

### C8. Parameters are effective-dated and resolved per call

A value is resolved as instance override, else implementation value, else
definition default, as of the call's due. A rate change on 1 July is a new
value effective from 1 July: a catch-up accrual for 30 June uses June's rate.
Values shared across products (rate cards, indices, the chart of accounts)
are reference data, which parameters refer to.

### C9. Moving an instance to a new version is an upgrade

Rebinding an account is a new binding version. The new implementation's
`upgrade` hook converts the instance's state, and its schedules become the
next version of the account's tempo set (D12). Upgrades are explicit, per
instance or in bulk, never implied by promoting a new default.

### C10. Versions are checked at registration

An implementation targets exactly one definition version; a definition, a
semver range of one model. The engine refuses at registration what does not
fit (a model version not applied, a parameter missing), never at call time.

### C11. The ledger is part of the core

Money moves only as balanced postings. Balances are views of postings.
An operation across accounts (a transfer) is one posting batch, committed by
the engine across the instances it touches. That is the one exception to C6's
one instance per commit, and it goes through the ledger, not through hooks
writing to each other.

### C12. A commit is a journal append; Postgres first

A commit is one conditional append to the instance's journal; views are
projections ([Storage](#storage)). That needs atomicity in one partition
only, which Postgres and ScyllaDB both have, so storage is behind a trait.
Postgres comes first, with tables designed as for ScyllaDB: transactions make
C5, C6 and C11 simple while the product takes shape. A ScyllaDB store is added
when load says so, and if the shape has held, no model or contract changes.

### C13. The model is bitemporal by kind

Facts carry value and booking time; effective attributes, valid and recorded
time; series, a point per date; fixed attributes, recorded time only
([Time](#time)). A hook reads valid time at its due and system time at a
snapshot taken once, in commit order, and recorded. Authors never write a
temporal query.

### C14. Backdated facts go to a correction hook

A fact that changes results already computed calls the instance's
`correction` hook, which returns adjustments as new facts. History is not
rewritten and hooks are not re-run: re-running would post a second set of
accruals to net against the first.

### C15. The core is small, and extended, never redefined

Party, account, ledger and reference data are the core. Domain models extend
its entities with facets and define their own views over it, filtered as they
need; they never change what a core entity means. Names are full English
words, with the vocabulary and codes of IFX and ISO 20022.

---

## Crates and apps

| Crate or app | Role |
|---|---|
| `tkone-model`, `tkone-model-macros` | `#[model]`, `#[entity]`, `#[extends]`, `#[fact]`, `#[effective]`, `#[series]`, `#[view]`; DDL diff; WIT generation |
| `tkone-core` | The core model: party, account, ledger, reference data; the vocabulary |
| `tkone-store` | The storage trait, journals and projections; Postgres first |
| `tkone-contract` | Definitions: operations, hooks, parameters, actions, `permits!`; their WIT worlds |
| `tkone-contract-sdk` | Writing an implementation in Rust for `wasm32-wasip2`; the manifest |
| `tkone-ledger` | Postings, balances, invariants |
| `tkone-engine` | Registries, OCI loading and verification, the pipeline, commits, projections |
| `tkone-tempo` | Scheduling ([schedule documents](schedule-document.md)) |
| engine server (app) | APIs: registries, lifecycle (C7), bindings, operations, bulk |
| `tkone` CLI (app) | `model apply`, `contract publish`, `promote`, `deprecate`, `archive`, `simulate` |

Existing crates to build on: wasmtime (components, pooling, fuel), wit-bindgen
and cargo-component, `oci-client` and wasm-pkg-tools, sigstore-rs, sqlx; CEL
(`cel-interpreter`) if guards in definitions need expressions.

## Simulation

An engine over an in-memory store, stepped by a [timeline](timeline.md) of the
instance's schedules: open an account, deposit, run ten years of end-of-day
hooks, and assert on postings. It runs the real component (C3), so it is how
a product is tested before it goes `live`, and how a change is shown to
differ from the version before.

## Open questions

- **Who writes contracts?** Domain experts rarely write Rust. WIT allows
  Python through componentize-py, but the authoring experience (SDK, local
  simulation, errors) is a product of its own.
- **WIT and model evolution.** WIT records are strict; a minor model version
  that adds a field needs a rule for components built against the version
  before.
- **Limits.** How much a hook may fetch (`postings since last capitalise` on
  a ten-year account) and how long it may run.
- **Hooks across instances** beyond read-only inputs and ledger batches
  (C5, C11): wait for a real product that needs them.
- **Corrections far back.** A fact backdated by months: how much the
  correction hook is given, and whether a limit sends it to an operator.
- **Entitlement policies:** roles and attributes in the model, or an
  external policy engine called by the engine.

## First slice

Before any framework: an easy-access savings account (deposit, withdraw,
end-of-day accrue, month-end capitalise) as one WASM implementation, on an
engine over Postgres, with tempo's schedules and a simulation test. Hand-write
its model and definition; extract `#[model]` and `#[definition]` when a second
product (a loan) shows what they share.
