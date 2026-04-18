# `tkone-outbox` and `tkone-inbox` — Crate Design

## Motivation

The transactional outbox/inbox pattern is domain-agnostic infrastructure. The relay and processor logic currently embedded in `tkone-tempo` is coupled to scheduling by accident — it references hardcoded table names (`schedule_outbox`, `schedule_inbox`) and a scheduling-specific processor implementation.

Extracting these into standalone crates gives every future tkone domain (payments, notifications, audit, …) the same reliable messaging guarantees without reimplementing the machinery.

---

## Crate Split: Why Two Instead of One

| Concern | `tkone-outbox` | `tkone-inbox` |
|---|---|---|
| Direction | Domain → Broker (publish) | Broker → Domain (consume + process) |
| Domain logic | None — pure relay | Yes — via `InboxMessageProcessor` trait |
| Can be used independently | Yes — fire-and-forget domains need no inbox | Yes — event-consumer domains need no outbox |
| Write path owner | Domain transaction (INSERT in domain tx) | Crate-owned relay (broker → table) |

A single `tkone-messaging` crate would couple independent concerns and force every consumer to take both directions as a dependency.

---

## Table Ownership Model

The central design rule:

> **The crate owns the relay logic. The domain owns the table DDL.**

Each domain creates its own physical tables using the SQL schema template shipped by the crate. The table name is just `outbox` / `inbox` — no domain prefix. Isolation is at the PostgreSQL schema level:

```
schedule schema:    schedule.outbox    schedule.inbox
payments schema:    payments.outbox    payments.inbox
```

No domain shares tables with another. The crate is given a fully-qualified table reference at construction time.

---

## `tkone-outbox`

### What it does

Polls a domain-owned `outbox` table for `PENDING` rows, publishes them to the broker, marks rows `SENT` or `FAILED`. Runs as an independent background task — completely decoupled from the domain's write path.

### Configuration

```rust
pub struct OutboxConfig {
    pub table: String,           // e.g. "outbox" or "schedule.outbox"
    pub poll_interval: Duration, // default: 500ms
    pub batch_size: usize,       // default: 100
    pub max_attempts: u8,        // default: 5 — after this, row stays FAILED
}
```

### Required Table Schema

The crate ships this as a versioned SQL template. The domain embeds it in its own migration suite.

```sql
-- Replace <outbox> with the domain's chosen table name / schema-qualified name.
CREATE TABLE outbox (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    topic           VARCHAR     NOT NULL,
    partition_key   VARCHAR,
    payload         JSONB       NOT NULL,
    headers         JSONB,
    status          VARCHAR     NOT NULL DEFAULT 'PENDING', -- PENDING | SENT | FAILED
    attempts        INT         NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    sent_at         TIMESTAMPTZ,
    aggregate_type  VARCHAR,     -- informational: the domain aggregate kind (e.g. "instance_run")
    aggregate_id    UUID,        -- informational: the domain aggregate id
    correlation_id  UUID        NOT NULL
);

CREATE UNIQUE INDEX outbox_correlation_id_idx ON outbox (correlation_id);
-- Partial index: the relay polls only PENDING rows ordered by age
CREATE INDEX outbox_pending_created_idx ON outbox (created_at) WHERE status = 'PENDING';
```

No foreign keys into domain tables — the outbox table is self-contained infrastructure.

### Domain Integration

The domain writes to the outbox **within its own transaction**. The crate plays no role in the write path.

```rust
// Inside domain fire transaction:
sqlx::query!("INSERT INTO outbox (topic, payload, correlation_id, ...) VALUES (...)")
    .execute(&mut txn)
    .await?;
txn.commit().await?;
```

At startup the domain constructs and spawns the relay:

```rust
let relay = OutboxRelay::builder()
    .config(OutboxConfig { table: "outbox".into(), batch_size: 100, .. })
    .pool(pg_pool.clone())
    .broker(broker_backend.clone())
    .build();

tokio::spawn(relay.run());
```

### Retry and Dead-Letter Policy

- Rows that fail to publish are retried with exponential backoff up to `max_attempts`.
- After `max_attempts` the row remains `FAILED` — no automatic progression. An operational alert must fire; the domain decides on a dead-letter sweep or manual remediation.
- Rationale: silent infinite retries mask broken downstreams. A stuck `FAILED` row is observable.

---

## `tkone-inbox`

### What it does

Two internal tasks:
1. **InboxRelay** — consumes messages from the broker, writes raw rows to the domain's `inbox` table. Deduplication happens downstream, not here.
2. **InboxProcessor** — polls the `inbox` table, enforces idempotency via `correlation_id`, calls the domain's `InboxMessageProcessor` trait implementation, updates row status.

### Configuration

```rust
pub struct InboxConfig {
    pub table: String,           // e.g. "inbox" or "schedule.inbox"
    pub topics: Vec<String>,
    pub consumer_group: String,
    pub poll_interval: Duration, // default: 200ms
    pub batch_size: usize,       // default: 50
}
```

### Required Table Schema

```sql
-- Replace <inbox> with the domain's chosen table name.
CREATE TABLE inbox (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    received_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    topic           VARCHAR     NOT NULL,
    message_key     VARCHAR,
    payload         JSONB       NOT NULL,
    headers         JSONB,
    correlation_id  UUID        NOT NULL,
    status          VARCHAR     NOT NULL DEFAULT 'PENDING', -- PENDING | PROCESSED | DUPLICATE | FAILED
    processed_at    TIMESTAMPTZ,
    error_message   TEXT
);

CREATE UNIQUE INDEX inbox_correlation_id_idx ON inbox (correlation_id);
CREATE INDEX inbox_pending_received_idx ON inbox (received_at) WHERE status = 'PENDING';
```

Note: no foreign key to any domain table. The domain processor resolves the relationship between `correlation_id` and its own entities.

### Domain Processing Trait

```rust
pub trait InboxMessageProcessor: Send + Sync + 'static {
    /// Called inside a transaction that the crate manages.
    /// The crate has already verified the row is PENDING (not DUPLICATE).
    /// Implementations update domain state within the provided transaction.
    async fn process(
        &self,
        txn: &mut PgTransaction<'_>,
        msg: &InboxMessage,
    ) -> Result<(), InboxProcessError>;
}

pub struct InboxMessage {
    pub id:             Uuid,
    pub topic:          String,
    pub message_key:    Option<String>,
    pub payload:        serde_json::Value,
    pub headers:        Option<serde_json::Value>,
    pub correlation_id: Uuid,
}
```

### Idempotency Contract

```
InboxRelay writes:
  INSERT INTO inbox (…) ON CONFLICT (correlation_id)
  DO UPDATE SET status = 'DUPLICATE'
  RETURNING status

InboxProcessor reads PENDING rows and for each:
  BEGIN
    if status = 'PENDING':
      call processor.process(txn, msg)   ← domain updates its tables here
      UPDATE inbox SET status = 'PROCESSED'
    if status = 'DUPLICATE':
      UPDATE inbox SET status = 'PROCESSED'   ← acknowledge, skip domain logic
  COMMIT
```

The `UNIQUE INDEX` on `correlation_id` is the single hard guarantee. Even if the relay crashes and re-delivers, the second INSERT sets status to `DUPLICATE` and the processor skips domain logic.

### Domain Integration

```rust
struct TempoInboxProcessor { pool: PgPool }

impl InboxMessageProcessor for TempoInboxProcessor {
    async fn process(&self, txn: &mut PgTransaction<'_>, msg: &InboxMessage) -> Result<()> {
        // look up schedule_instance_run by correlation_id
        // update its status → COMPLETED or FAILED
        // insert schedule_defn_run_event
        Ok(())
    }
}

let inbox = InboxService::builder()
    .config(InboxConfig { table: "inbox".into(), topics: vec!["schedule.replies".into()], .. })
    .pool(pg_pool.clone())
    .broker(broker_backend.clone())
    .processor(TempoInboxProcessor { pool: pg_pool.clone() })
    .build();

tokio::spawn(inbox.run());
```

---

## Shared Broker Abstraction

Both crates depend on a common `tkone-broker` (or inlined `BrokerBackend` trait — TBD):

```rust
pub trait BrokerBackend: Send + Sync + 'static {
    async fn publish(
        &self,
        topic:         &str,
        key:           Option<&str>,
        payload:       &[u8],
        headers:       &[(String, String)],
    ) -> Result<(), BrokerError>;

    fn subscribe(
        &self,
        topic:          &str,
        consumer_group: &str,
    ) -> impl Stream<Item = Result<BrokerMessage, BrokerError>> + Send;
}
```

This is the same trait already designed in `tkone-tempo/docs/components.md`. The decision of whether to extract it into a shared crate or duplicate it (per crate) is deferred — currently `tkone-tempo` is the only consumer.

---

## Impact on `tkone-tempo`

| Before | After |
|---|---|
| `schedule_outbox` / `schedule_inbox` table names baked into tempo | Tempo owns DDL: tables named `outbox` / `inbox` within schedule schema |
| Relay + processor logic in tempo | Tempo configures `tkone-outbox` + `tkone-inbox`; provides `TempoInboxProcessor` |
| `instance_run_id FK` on inbox table (domain leak into generic table) | Removed — processor resolves domain entity via `correlation_id` |
| No reuse path for other domains | Any new domain configures the crates with its own tables |

The `correlation_id` lifecycle is unchanged — tempo generates it in the fire transaction; the downstream system echoes it in the reply; the processor matches it.

---

## Adding a Future Domain

A `payments` domain:

1. Creates `payments.outbox` and `payments.inbox` using the SQL templates.
2. Configures `OutboxRelay` with `table: "payments.outbox"`.
3. Implements `InboxMessageProcessor` for payment completion logic.
4. Spawns `InboxService` with `table: "payments.inbox"`.

Zero shared tables. Zero changes to other domains. Full isolation.
