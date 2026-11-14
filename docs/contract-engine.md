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

```rust
#[model(name = "deposits", version = "3.1.0")]
mod deposits {
    #[entity(key = account_id)]
    struct Account { account_id: AccountId, customer_id: CustomerId, status: Status, opened_on: NaiveDate }

    #[relation(Account.customer_id -> customers::Customer)]
    struct HeldBy;

    /// What a hook reads about its own account, kept with every commit.
    #[view(key = account_id, consistency = "with-instance")]
    struct AccountState { account_id: AccountId, status: Status, balances: Balances, accrued: Money }

    /// Read across accounts; may lag.
    #[view(key = customer_id, cluster = account_id, consistency = "eventual")]
    struct AccountsByCustomer { customer_id: CustomerId, account_id: AccountId, available: Money }
}
```

- **One source, two outputs.** The model crate generates the Rust types the
  engine uses and the WIT types implementations see, so the two cannot
  drift.
- **Apply.** `tkone model apply deposits@3.1.0` diffs against the applied
  version, emits and runs DDL, and records the version. Changes follow
  semver: adding a field or view is minor, removing or retyping is major and
  needs a migration.
- **Views are derived, never written by contracts** (C4). The engine maintains
  each from the actions that change its sources.

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

1. **Resolve.** Instance → its binding: definition version, implementation
   digest, overrides. A deprecated implementation still runs for bound
   instances; an archived one never does (C7).
2. **Parameters,** resolved as of the due (C8): instance override, else
   implementation value, else definition default.
3. **Fetch** the hook's `requires`, from views consistent with the
   instance's last commit (C5). For a bulk call, fetches are batched.
4. **Invoke** in a fresh component instance: no I/O, the clock passed in,
   with fuel and memory limits. With wasmtime's pooling allocator,
   instantiation is microseconds; the cost is in moving the inputs, which is
   why they are prepared and small.
5. **Validate** the returned actions: permitted by the definition, valid
   against the model's invariants (a posting balances; a transition is
   allowed from the current state).
6. **Commit** per instance, in one transaction: the actions, the entities and
   the `with-instance` views they change, the instance's version, the outbox
   (for `eventual` views and events), and tempo's schedule changes.
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

### C12. Postgres first

Transactions across an instance's entity, views, outbox and tempo make C5 and
C6 simple. Storage is behind a trait; ScyllaDB fits the `eventual` views at
scale, where its lack of transactions across tables does not matter.

---

## Crates and apps

| Crate or app | Role |
|---|---|
| `tkone-model`, `tkone-model-macros` | `#[model]`, `#[entity]`, `#[view]`; DDL diff; WIT generation |
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

## First slice

Before any framework: an easy-access savings account (deposit, withdraw,
end-of-day accrue, month-end capitalise) as one WASM implementation, on an
engine over Postgres, with tempo's schedules and a simulation test. Hand-write
its model and definition; extract `#[model]` and `#[definition]` when a second
product (a loan) shows what they share.
