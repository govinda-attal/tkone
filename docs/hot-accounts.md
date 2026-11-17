# Contract engine — Hot accounts

Status: **architecture only**. Part of the [contract engine](contract-engine.md);
rule 7 of its [models](contract-models.md#7-hot-accounts-are-not-serialised).

## The problem

Commits on one instance are serialised: one conditional append to its
journal, "only if still at version n" (C6, C12). That is what stops two
withdrawals both seeing the same balance. But some accounts are in almost
every booking:

- **internal accounts:** fee income, interest expense, suspense, a clearing
  account, a nostro;
- **customer accounts that receive at volume:** a merchant settled for every
  card payment, a payroll account, a tax authority's collection account.

If every booking that touches one of them must win a version check on its
journal, the bank's throughput is the throughput of one partition, and
retries grow with load. A row holding a balance is no better on Postgres: the
row lock serialises the same way.

## The rule

> A write is serialised only if a decision reads the value it changes.

| Write | Must it wait for the account's other writes? |
|---|---|
| A debit to an account whose balance guards it (no overdraft beyond a limit) | **yes**: two debits could both pass a check that only one should |
| A credit to any account | **no**: a credit never makes a passed check fail (an account that may refuse credits takes them as pending: [guarded credits](#guarded-credits)) |
| Any entry to an account with no guard (internal accounts) | **no**: there is no check |

So every booking has a **home**, and everything else in it is **delivered**:

- **The home** is the journal of the instance whose operation produced the
  booking: the account being withdrawn from, transferred from, charged. Its
  guarded debits are checked and appended there, under the version check, as
  today. A booking with no guarded account (an internal adjustment) is homed
  in a partition of its own, keyed by booking id, and contends with nothing.
- **Delivered entries** go to the **inbox** of the account they are for:
  credits to anyone, and any entry to an unguarded account. They need no
  version check, because nothing they would be checked against exists.

Debiting a guarded account is therefore always an operation on that account.
A transfer from A to B is an operation on A: A's debit is checked in A's
journal; B's credit is delivered. A direct debit pulled from B is an
operation on B. That makes almost every booking a **single-partition commit**,
which is what lets the ledger run on ScyllaDB without a saga (C11, C12).

## What authors write

Usually nothing. The guard is chosen by the definition, per product:

```rust
#[definition(name = "easy-saver", version = "2.0.0", model = "deposits@^3.1")]
#[ledger(guard = "debits")]            // the default
mod easy_saver { /* ... */ }

#[definition(name = "fee-income", version = "1.0.0", model = "core@^1.4")]
#[ledger(guard = "none")]
mod fee_income { /* ... */ }

// Credits must be accepted by the product before they count: see guarded credits.
#[definition(name = "cash-isa", version = "1.0.0", model = "deposits@^3.1")]
#[ledger(guard = "debits-and-credits")]
mod cash_isa { /* ... */ }

// A known giant may fix its inbox width; otherwise it is chosen per day.
#[definition(name = "merchant-settlement", version = "1.0.0", model = "core@^1.4")]
#[ledger(guard = "debits", inbox_shards = 64)]
mod merchant_settlement { /* ... */ }
```

And what the model refuses, at registration:

- a hook that requires an **unguarded** account's balance `with-instance`:
  it is never exact, so it may only be read from an `eventual` view with a
  stated lag, and never used as a check;
- a limit, or a hold, on an unguarded account: there is nothing to enforce
  it against.

A contract that books never says where an entry goes. It returns a booking;
the engine decides home and deliveries from the accounts' guards.

## How it is stored

```text
journal (home)       (instance, bucket)            clustered by version     guarded debits, everything else of the instance
ledger_inbox         (account, business_date, shard) clustered by (booking, entry)   delivered entries, insert only
inbox_width          (account, business_date)      → shards
balance_checkpoint   (account, address, business_date) → closing balance, inbox watermark
```

- **Shard** is `hash(booking_id) mod width`. The width is chosen for each
  account and business day from the day before's rate (1 for almost every
  account; tens for a merchant), so resharding happens at a day boundary and
  never moves a row.
- **Insert only.** An inbox row is keyed by its booking and entry, so a
  delivery repeated after a failure writes the same row. No counters: database
  counters cannot be written idempotently.
- **A balance** is the last closing checkpoint, plus the home journal's
  entries since, plus the inbox's entries since. The day's close folds each
  account's inbox into its checkpoint, so a read touches at most one day's
  shards.

| | Postgres | ScyllaDB |
|---|---|---|
| Delivery | inbox rows inserted **in the home commit's transaction**: atomic and immediate; inserts take no lock on the receiving account | written from the outbox after the home append, idempotently; visible after a lag |
| A guarded balance read | exact | home journal exact; inbox up to a watermark per shard, recorded in the snapshot |

## Why it is safe

- **Guarded checks never pass wrongly.** The only entries a check can miss
  are delivered ones still in flight, and those are credits: missing them can
  only make the balance look lower. A check may fail that would have passed a
  moment later; it never passes when it should fail.
- **Money is never in limbo.** The booking, all its entries, is committed in
  its home. Delivery projects what is already committed; until it is
  delivered it is in the home journal and the outbox, and a reconciliation
  shows it.
- **Replay is exact.** A snapshot records the inbox watermark it read to, so
  a replay reads the same entries (C3).

## What it costs

- **A credit is visible a little later** on ScyllaDB: milliseconds normally,
  more under backlog. A customer sees "received" when delivery lands.
- **A credit can arrive at an account that closed meanwhile.** The engine
  checked the creditor's status from an `eventual` view when it validated the
  booking; if the account has since closed or been blocked, the delivery is
  refused, and the engine books a **return** to a returns account and notifies.
  That is what payment schemes do anyway.
- **Credit hooks run after delivery.** A hook that reacts to a deposit on the
  receiving account (`#[hook(credited)]`) is a separate operation on that
  account, with its own commit, not part of the sender's.
- **End of day waits for deliveries.** A scheduled run at its due waits until
  the account's inbox watermark has passed the due, so the day's accrual
  includes every credit of the day. If it waits past a limit, the run is
  stale (D15) and an operator is told. A credit delivered after the run with
  an earlier value date is a backdated fact, handled by the correction hook
  (C14).
- **Reconciliation is a job, not a given.** For each business date, every
  booking's home entries and delivered entries net to zero, and every
  delivery expected from the outbox is in an inbox. A check runs at each
  close and reports breaks.

## Guarded credits

Some products must be able to refuse money:

- an ISA, over its annual subscription allowance;
- a fixed-term deposit, after its funding window has closed;
- an account under a court order, or closed with a final balance owed.

On those, the definition guards credits too
(`guard = "debits-and-credits"`). Commits stay single-partition: the credit
is still delivered, but it lands **pending**, outside the balance, and the
account decides:

```text
 A's journal (home)                    ISA's inbox                 ISA's journal
 ─────────────────                     ───────────                 ─────────────
 book: debit A 5,000 ──── deliver ───► credit 5,000 (pending) ──► credited hook
       credit ISA 5,000                                             │
                                                         accept ────┤ commit: the credit counts
                                                                    │
                                                         refuse ────┘ commit: refusal, and a
                                                                       return booking homed here:
 A's inbox ◄──────────────── deliver ── credit A 5,000 (return) ◄──── debit ISA-pending 5,000
```

- **Pending credits are not in the balance,** so nothing (an accrual, a
  withdrawal) can use money the account has not accepted. A view shows them
  separately: "pending: £5,000".
- **The `credited` hook runs as an operation on the receiving account,** in
  its own commit, against its own snapshot: so it sees exactly what it has
  accepted so far, in order. Two transfers in cannot both fit an allowance
  only one fits.
- **A refusal is a return.** The engine books it, homed on the refusing
  account, debiting the pending credit and delivering a credit back to the
  sender, with the reason as a typed code. That is what payment schemes do
  with money a receiving bank will not accept; it never needs a commit across
  two partitions.
- **Senders see "sent", then perhaps "returned",** not an immediate refusal.
  A channel that wants to refuse up front asks first, with a query on the
  receiving account (`check_subscription`). The answer is advice: the
  `credited` hook still decides, because another credit may land in between.
- **Where a credit lands is the receiving product's choice,** never the
  sender's: `#[ledger(credits_to = "...")]` names the balance type
  (`Available` by default; `Principal` for a fixed deposit; `Incoming` for a
  loan that allocates each payment).
- **Credits from operations on the account itself** (a cash deposit at a
  branch) are not delivered: they are that operation, and its hook refuses
  them directly.

## Beyond the ledger

The rule is general: writes that **commute** and guard nothing are delivered
and summed; writes that **guard** something are serialised. Usage metering,
counters and votes are the first kind. Stock, or a credit line shared by
several accounts, is a bounded case: neither freely delivered nor cheaply
serialised. The tested answer is escrow: split the allowance into a share per
shard, check each locally, and rebalance shares when one runs low. It is
listed under [later](contract-models.md#later).
