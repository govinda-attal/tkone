# Contract engine — Hot instances: quantities, guards and delivery

Status: **architecture only**. Part of the [contract engine](contract-engine.md);
rule 7 of its [models](contract-models.md#7-hot-instances-are-not-serialised).

These are **kernel** mechanisms: they know instances, facts, quantities and
guards, not accounts or money. The ledger is their first user, and its terms
(balances, debits, credits, returns) appear here only in the examples and in
[how the ledger uses them](#how-the-ledger-uses-them)
([C40](contract-engine.md#c40-the-kernel-knows-mechanisms-extensions-bring-vocabulary)).

## The problem

Commits on one instance are serialised: one conditional append to its
journal, "only if still at version n" (C6, C12). That is what stops two
decisions both seeing the same value. But some instances are touched by
almost every commit:

- in banking, internal accounts (fee income, suspense, a nostro) and
  customer accounts that receive at volume (a merchant settled for every card
  payment);
- in loyalty, the scheme's own points pool;
- in metering, a shared counter every subscription reports to.

If every write to one of them must win a version check on its journal,
throughput is the throughput of one partition, and retries grow with load. A
row holding a running total is no better on Postgres: the row lock
serialises the same way.

## The rule

> A write is serialised only if a decision reads the value it changes.

A **quantity** is a value folded from facts, per instance: a sum of signed
amounts. A **guard** is a bound on a quantity that the instance enforces, in
one or both **directions**:

| Write to a quantity | Must it wait for the instance's other writes? |
|---|---|
| In a **guarded direction** (a decrease, where the bound is a floor) | **yes**: two writes could both pass a check that only one should |
| In an **unguarded direction** (an increase, where the bound is a floor) | **no**: it can never make a passed check fail |
| To a quantity with **no guard** | **no**: there is no check |

So every commit that writes facts to several instances has a **home**, and
everything else in it is **delivered**:

- **The home** is the instance whose operation produced the commit. Its
  guarded writes are checked and appended there, under the version check. A
  commit with no guarded write (an internal adjustment) is homed in a
  partition of its own, keyed by its id, and contends with nothing.
- **Delivered facts** go to the **inbox** of the instance they are for:
  writes in an unguarded direction, and any write to an unguarded quantity.
  They need no version check, because nothing they would be checked against
  can be wrong.
- **A delivery in a guarded direction** (an instance that must be able to
  refuse an increase) is still delivered, but lands **pending**:
  [delivery with acceptance](#delivery-with-acceptance).

A write in a guarded direction is therefore always an operation on that
instance. That makes almost every commit a **single-partition commit**, which
is what lets the engine run on ScyllaDB without sagas (C12).

## What models and definitions declare

A model declares a quantity over one of its fact types, with kernel
attributes:

```rust
#[model(name = "loyalty", version = "1.0.0")]
mod loyalty {
    /// Points moved to or from a member. Delivered; refused moves are revoked.
    #[fact(key = move_id)]
    #[action(deliver = "Award", compensate = "Revoke")]
    struct PointsMove { move_id: MoveId, member: MemberId, points: Quantity<Point> }

    /// A member's points: the kernel folds moves per member.
    #[quantity(of = PointsMove, instance = member, amount = points)]
    struct Points;
}
```

A definition chooses the guard, per product:

```rust
#[definition(name = "standard-member", version = "1.0.0", model = "loyalty@^1")]
#[guard(Points, decreases = "points >= 0")]                // spending is checked, in order
mod standard_member { /* ... */ }

#[definition(name = "capped-member", version = "1.0.0", model = "loyalty@^1")]
#[guard(Points, decreases = "points >= 0",
                increases = accept("points <= params.CAP"))]   // awards over the cap may be refused
mod capped_member { /* ... */ }

#[definition(name = "points-pool", version = "1.0.0", model = "loyalty@^1")]
#[guard(Points, none)]                                     // the scheme's pool: never checked
#[inbox(shards = 64)]                                      // a known giant may fix its width
mod points_pool { /* ... */ }
```

- **The bound is CEL** over the folded quantity, the instance's snapshot
  and its parameters, checked by the kernel at commit for guarded writes,
  whatever the hook thought.
- **An extension may give these a vocabulary of its own.** The ledger's
  `#[ledger::balances]` declares balance types as addresses of a kernel
  quantity over `core::Entry`, and `#[ledger(guard = "debits")]` expands to a
  `#[guard]`. Same mechanism, banking words
  ([how the ledger uses them](#how-the-ledger-uses-them)).

What the kernel refuses, at registration:

- a hook that requires an **unguarded** quantity `with-instance`: it is
  never exact, so it may only be read from an `eventual` view with a stated
  lag, and never used as a check;
- a model invariant that reads an unguarded quantity: there is nothing to
  enforce it against.

A contract never says where a fact goes. It returns actions; the kernel
decides home and deliveries from the instances' guards.

## How it is stored

```text
journal           (instance, bucket)                clustered by version         the home's writes
inbox             (instance, period, shard)         clustered by (commit, fact)  delivered facts, insert only
inbox_width       (instance, period)                → shards
fold_checkpoint   (instance, quantity, key, period) → folded value, inbox watermark
```

- **Shard** is `hash(commit_id) mod width`. The width is chosen for each
  instance and period from the period before's rate (1 for almost every
  instance; tens for a merchant or a pool), so resharding happens at a
  period boundary and never moves a row.
- **Insert only.** An inbox row is keyed by its commit and fact, so a
  delivery repeated after a failure writes the same row. No counters: database
  counters cannot be written idempotently.
- **A quantity's value** is the last checkpoint, plus the home journal's
  facts since, plus the inbox's facts since. Each period's close folds every
  inbox into its checkpoint, so a read touches at most one period's shards.
  The period is the instance's business day, from its calendar.

| | Postgres | ScyllaDB |
|---|---|---|
| Delivery | inbox rows inserted **in the home commit's transaction**: atomic and immediate; inserts take no lock on the receiving instance | written from the outbox after the home append, idempotently; visible after a lag |
| A guarded read | exact | home journal exact; inbox up to a watermark per shard, recorded in the snapshot |

## Why it is safe

- **Guarded checks never pass wrongly.** The only writes a check can miss
  are deliveries still in flight, and those are in the unguarded direction:
  missing them can only make the check stricter. A check may fail that would
  have passed a moment later; it never passes when it should fail.
- **Nothing is in limbo.** The commit, all its facts, is in its home.
  Delivery projects what is already committed; until it lands it is in the
  home journal and the outbox, and reconciliation shows it.
- **Replay is exact.** A snapshot records the inbox watermark it read to, so
  a replay reads the same facts (C3).

## What it costs

- **A delivery is visible a little later** on ScyllaDB: milliseconds
  normally, more under backlog.
- **A delivery can arrive at an instance that closed meanwhile.** The kernel
  checked the receiver's status from an `eventual` view when it validated
  the commit; if it has since closed, the delivery is refused and
  compensated, as a refusal by acceptance is (below).
- **Reactions run after delivery.** A hook that reacts to a delivery on the
  receiving instance (`#[hook(delivered(T))]`) is a separate operation on it,
  with its own commit, not part of the sender's.
- **Period close waits for deliveries.** A scheduled run at its due waits
  until the instance's inbox watermark has passed the due, so the run sees
  every delivery of the period. If it waits past a limit, the run is stale
  (D15) and an operator is told. A delivery after the run with an earlier
  valid time is a backdated fact, handled by the correction hook (C14).
- **Reconciliation is a job, not a given.** For each period, every delivery
  the outbox expected is in an inbox, and the extension's own check (for the
  ledger, that every booking nets to zero across home and deliveries) runs
  at close and reports breaks.

## Delivery with acceptance

Some instances must be able to refuse an increase:

- a capped loyalty member, over the cap;
- in banking: an ISA over its annual allowance, a fixed-term deposit after
  its funding window, an account under a court order.

Their definition guards increases with `accept(..)`. Commits stay
single-partition: the fact is still delivered, but it lands **pending**,
outside the quantity, and the receiving instance decides in its `delivered`
hook:

```text
 sender's journal (home)              receiver's inbox            receiver's journal
 ───────────────────────              ────────────────            ──────────────────
 commit: move 500 ───── deliver ────► +500 (pending) ───────────► delivered hook
                                                                     │
                                                          accept ────┤ commit: it counts
                                                                     │
                                                          refuse ────┘ commit: refusal, and the
                                                                        compensation, homed here:
 sender's inbox ◄──────────────── deliver ── +500 back ◄────────────── -500 pending
```

- **Pending facts are not in the quantity,** so nothing can use what the
  instance has not accepted. A view shows them separately.
- **The `delivered` hook runs as an operation on the receiver,** in its own
  commit, against its own snapshot: it sees exactly what it has accepted so
  far, in order. Two deliveries cannot both fit an allowance only one fits.
- **A refusal is compensated, never rolled back.** The kernel asks the
  fact's owner for the compensation (a declared action's `compensate`, or an
  extension's `compensate`), commits it homed on the refusing instance and
  delivers it back, with the refusal's typed reason. It never needs a commit
  across two partitions.
- **Senders see "sent", then perhaps "returned",** not an immediate refusal.
  A channel that wants to refuse up front asks first, with a query on the
  receiver. The answer is advice: the `delivered` hook still decides.
- **Writes from operations on the instance itself** are not delivered: they
  are that operation, and its hook refuses them directly.

## Joint commits: the one exception

A commit with guarded writes on **two** instances (a transfer between two
accounts that both enforce a floor, run as one operation) cannot be homed in
one. The kernel offers it as an explicit, rare mode:

- an extension's or a declared action's plan marks it `joint`, naming at
  most two instances; nothing else may;
- on Postgres it is one transaction; on ScyllaDB, a saga the kernel runs
  through the outbox, compensating with the same `compensate` as a refusal;
- every joint commit is counted and visible, because a product that needs
  them often has drawn its instances wrongly (C29).

## How the ledger uses them

The ledger adds no mechanism of its own. It names the kernel's:

| Kernel | Ledger |
|---|---|
| quantity over a fact | balances: `core::Entry` folded per account and balance address (`#[ledger::balances]`) |
| guard on decreases | `#[ledger(guard = "debits")]`, the default; the floor is the account's limit |
| guard on increases, with acceptance | `#[ledger(guard = "debits-and-credits")]`: guarded credits |
| no guard | `#[ledger(guard = "none")]`: internal accounts |
| delivered fact | a credit, or an entry to an unguarded account |
| `delivered` hook | the definition's hook for `ledger::Entry`, conventionally named `credited` |
| compensation | a return booking to a returns account |
| where an accepted delivery lands | `#[ledger(credits_to = "Principal")]`: the receiving product's choice, never the sender's |
| joint commit | a booking that debits two guarded accounts |
| period close | the business day's close; the ledger adds its trial balance and booking reconciliation |

```rust
#[definition(name = "cash-isa", version = "1.0.0", model = "deposits@^3.1")]
#[ledger(guard = "debits-and-credits")]          // = #[guard(ledger::Balance, decreases = .., increases = accept(..))]
mod cash_isa {
    #[hook(delivered(ledger::Entry))] fn credited;   // accept or refuse each subscription
}
```

What the ledger itself still decides, as an extension, is what the kernel
cannot: that a booking's entries net to zero per currency, that amounts fit
the currency's minor units or a balance type's declared scale, and that
logical accounts resolve through the accounting mapping
([extensions](contract-hooks.md#3-extensions)).

## Beyond guards: bounded and shared quantities

The rule is general: writes that **commute** and guard nothing are delivered
and folded; writes that **guard** something are serialised in their home.
A quantity shared by many instances under one bound (stock, a credit line
across accounts) is neither: its guard would be on an instance everyone
writes to. The tested answer is escrow: split the allowance into a share per
shard, check each locally, and rebalance shares when one runs low. It is
listed under [later](contract-models.md#later); a facility instance (C29)
serves until then.
