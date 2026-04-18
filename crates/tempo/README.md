# Tempo

Enterprise-grade distributed scheduler built on the `tkone-schedule` spec language. Tempo adds a persistent PostgreSQL coordination layer, a transactional outbox/inbox messaging pipeline, and horizontal worker scaling on top of the in-process primitives already provided by `tkone-schedule` and `tkone-trigger`.

---

## Contents

- [What Tempo Is](#what-tempo-is)
- [Architecture Overview](#architecture-overview)
- [Key Concepts](#key-concepts)
- [Data Model](#data-model)
- [End-to-End Workflow](#end-to-end-workflow)
- [Scalability Model](#scalability-model)
- [Versioning](#versioning)
- [Outbox / Inbox Pattern](#outbox--inbox-pattern)
- [Overlap and Dependency Policies](#overlap-and-dependency-policies)
- [Further Reading](#further-reading)

---

## Quick Start (Docker Compose)

```bash
cd crates/tempo
cp .env.example .env
docker compose up -d                        # infrastructure only
docker compose --profile workers up -d     # add all Tempo workers
open http://localhost:8080                  # Redpanda console
```

See [docs/docker-compose.md](docs/docker-compose.md) for scaling, broker alternatives, and design differences from the Kubernetes deployment.

---

## What Tempo Is

`tkone-schedule` and `tkone-trigger` solve in-process scheduling: parse a spec, iterate occurrences, fan out async callbacks. That model is excellent for intra-day, time-based recurrences where a missed tick on restart is acceptable.

Tempo is the layer you reach for when those constraints cannot be accepted:

| Requirement | tkone-trigger | Tempo |
|---|---|---|
| In-process, no DB | yes | no |
| Survives process restart | no | yes |
| Distributed workers | no | yes |
| Per-entity instance runs | no | yes |
| Reliable downstream notification | no (direct call) | yes (outbox) |
| Completion acknowledgement | no | yes (inbox) |
| Definition versioning | no | yes |
| Date / datetime specs (biz-day aware) | yes | yes |
| 1M+ runs/day | no | yes |

Tempo is purpose-built for **date** and **datetime** specs — end-of-month settlement runs, quarterly rebalancing, per-customer billing cycles — where each occurrence may fan out to thousands of downstream entities and every fire must be accounted for.

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Tempo Process(es)                            │
│                                                                      │
│  ┌─────────────────────┐     ┌──────────────────────────────────┐   │
│  │  Occurrence          │     │  Messaging Workers               │   │
│  │  Generator           │     │                                  │   │
│  │  (tkone-schedule     │     │  ┌────────────┐                  │   │
│  │   iterator → DB)     │     │  │Outbox Relay│──► Kafka / Iggy  │   │
│  └─────────────────────┘     │  └────────────┘                  │   │
│                               │  ┌────────────┐                  │   │
│  ┌─────────────────────┐     │  │Inbox Relay │◄── Kafka / Iggy  │   │
│  │  Occurrence          │     │  └────────────┘                  │   │
│  │  Claimer             │     │  ┌─────────────────┐             │   │
│  │  (SKIP LOCKED batch) │     │  │Inbox Processor  │             │   │
│  └─────────────────────┘     │  └─────────────────┘             │   │
│                               └──────────────────────────────────┘   │
│  ┌─────────────────────┐     ┌──────────────────────────────────┐   │
│  │  Run Aggregator      │     │  Worker Coordinator              │   │
│  │  (event rollup)      │     │  (heartbeat + lease recovery)    │   │
│  └─────────────────────┘     └──────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
                    │  all workers read/write  │
                    ▼                          ▼
         ┌─────────────────────────────────────────┐
         │              PostgreSQL                  │
         │  schedule_defn      schedule_occurrence  │
         │  schedule_instance  schedule_defn_run    │
         │  schedule_instance_run                   │
         │  schedule_outbox    schedule_inbox        │
         │  tempo_worker       …                    │
         └─────────────────────────────────────────┘
```

See [docs/components.md](docs/components.md) for the detailed component diagram.

### Components

| Component | Responsibility |
|---|---|
| **Occurrence Generator** | Reads active `schedule_defn` rows, runs `tkone-schedule` iterators, pre-fills `schedule_occurrence` rows a configurable horizon ahead. Updates the watermark so restarts are idempotent. |
| **Occurrence Claimer** | Polls for `PENDING` occurrences whose `occurrence_dtm ≤ now()` using `SELECT FOR UPDATE SKIP LOCKED` on a shard slice. Atomically writes `schedule_defn_run`, all `schedule_instance_run` rows, and `schedule_outbox` messages in one transaction. |
| **Outbox Relay** | Reads `PENDING` outbox rows and publishes to Kafka / Iggy. Marks rows `SENT` on broker ack or `FAILED` after N retries. Runs independently of the claimer — the broker is the durability boundary. |
| **Inbox Relay** | Consumes from Kafka / Iggy and writes raw messages to `schedule_inbox`. Deduplication happens downstream (inbox processor), not here. |
| **Inbox Processor** | Reads `PENDING` inbox rows, validates the `correlation_id` for idempotency, updates the linked `schedule_instance_run` status, appends a `schedule_defn_run_event`, and marks the inbox row `PROCESSED`. |
| **Run Aggregator** | Reads unprocessed `schedule_defn_run_event` rows in batches and updates the `completed_count` / `failed_count` rollup on `schedule_defn_run`. Decoupled from the hot fire path to avoid row-level lock contention. |
| **Worker Coordinator** | Each process heartbeats its `tempo_worker` row. The coordinator detects dead workers (stale `last_heartbeat`) and resets their claimed occurrences back to `PENDING` for redistribution. |

---

## Key Concepts

### Schedule Definition (`schedule_defn`)

The template for a recurring job. Identified by `(type, type_ref, name)` — for example `("PAYMENT", "MONTHLY_SETTLEMENT", "END_OF_MONTH")`. Carries a `run_spec` JSON object containing the `tkone-schedule` spec string and timezone:

```json
{ "spec": "YY-1M-L~NBT11:00:00", "timezone": "Europe/London" }
```

Definitions are **versioned**. Each edit creates a new `(id, version)` row; the previous version transitions to `SUPERSEDED`. Instances and runs always pin the version they were created against.

### Schedule Instance (`schedule_instance`)

A subscription from an external entity (e.g. a customer, account, fund) to a definition. An instance can override the definition's `run_spec` and `overlap_policy`. One entity, one instance per definition, pinned to a definition version at enrolment time.

### Occurrence (`schedule_occurrence`)

A single pre-generated point in time when a definition should fire. Mirrors the `tkone_schedule::Occurrence<T>` enum:

| DB `kind` | Rust variant | Meaning |
|---|---|---|
| `EXACT` | `Occurrence::Exact(t)` | No business-day adjustment; `actual_dtm == occurrence_dtm` |
| `ADJUSTED_LATER` | `Occurrence::AdjustedLater(a, o)` | Settlement moved later; `occurrence_dtm > actual_dtm` |
| `ADJUSTED_EARLIER` | `Occurrence::AdjustedEarlier(a, o)` | Settlement moved earlier; `occurrence_dtm < actual_dtm` |

`occurrence_dtm` is the **observed / settlement date** — the date on which the event fires. `actual_dtm` is the **raw calendar date** before any business-day rule was applied. Both are always stored.

### Definition Run (`schedule_defn_run`)

The aggregate execution record for one occurrence across all enrolled instances. Tracks `instance_count`, `completed_count`, and `failed_count` via an async event-sourced rollup.

### Instance Run (`schedule_instance_run`)

The per-entity execution record. One row per active instance per occurrence. Status is driven by inbox completion messages from the downstream system.

---

## Data Model

See [docs/erd.md](docs/erd.md) for the full entity-relationship diagram.

High-level relationships:

```
CALENDAR ──< SCHEDULE_DEFN (versioned) ──< SCHEDULE_INSTANCE
                    │
                    ├──< SCHEDULE_OCCURRENCE ──── SCHEDULE_DEFN_RUN
                    │                                    │
                    └──< SCHEDULE_DEFN_DEP       SCHEDULE_INSTANCE_RUN
                                                        │
                    SCHEDULE_OUTBOX ◄── fire     SCHEDULE_INBOX ◄── complete
```

---

## End-to-End Workflow

See [docs/sequences.md](docs/sequences.md) for detailed sequence diagrams. Summary:

### 1. Startup — occurrence pre-generation

1. Occurrence Generator reads all `ACTIVE` schedule definitions.
2. For each definition, it reads the watermark (`schedule_occurrence_watermark`) to find the last generated occurrence.
3. It runs the `tkone-schedule` iterator forward by the configured horizon (e.g. 48 hours or 500 occurrences).
4. New `schedule_occurrence` rows are inserted (upsert on `(defn_id, defn_version, occurrence_dtm)` — idempotent on restart).
5. The watermark is updated.

### 2. Fire — claim and fan-out

1. Occurrence Claimer polls for `PENDING` occurrences with `occurrence_dtm ≤ now()`, filtered to its assigned shard range.
2. Batch-claims rows atomically with `UPDATE … FOR UPDATE SKIP LOCKED`.
3. Within one transaction per batch entry:
   - Writes `schedule_defn_run` (status = `IN_PROGRESS`).
   - Writes one `schedule_instance_run` row per active instance (status = `PENDING`).
   - Writes one `schedule_outbox` row per instance run with a unique `correlation_id`.
   - Transitions the occurrence to `FIRED`.
4. Outbox Relay picks up `PENDING` outbox rows and publishes to Kafka / Iggy.
5. Broker delivers messages to downstream systems.

### 3. Complete — inbox and rollup

1. Downstream system sends a completion message (carrying the `correlation_id`) to the reply topic.
2. Inbox Relay consumes and writes to `schedule_inbox`.
3. Inbox Processor deduplicates on `correlation_id`, updates the `schedule_instance_run` status, and appends a `schedule_defn_run_event`.
4. Run Aggregator reads new events and updates `schedule_defn_run` rollup counters. When `completed_count + failed_count == instance_count`, the defn run transitions to `COMPLETED` or `FAILED`.

### 4. Recovery — stale lease reclaim

1. Worker Coordinator queries for `CLAIMED` occurrences with `lease_expires_at < now()`.
2. Resets them to `PENDING`.
3. Marks the dead worker row as `DEAD` in `tempo_worker`.
4. Any healthy claimer worker picks them up on the next poll cycle.

---

## Scalability Model

| Volume | Configuration |
|---|---|
| < 100 fires/sec | Single process, batch_size=50, synchronous rollup |
| 100–1 000 fires/sec | 4–8 claimer workers across shard ranges, async rollup via event table |
| 1 000–10 000 fires/sec | 16–32 claimer workers, PgBouncer transaction-mode pooling, read replica for reporting, monthly partition drop |

### Shard key

Every `schedule_occurrence` row carries a `shard_key` computed column:

```sql
abs(hashtext(defn_id::text)) % 256
```

Claimer workers are assigned non-overlapping ranges of this 0–255 space. Adding or removing workers requires only updating the `tempo_worker` shard assignment — no schema migration, no downtime.

### Batch claiming

Workers claim N occurrences per round-trip rather than one at a time:

```sql
UPDATE schedule_occurrence
SET status = 'CLAIMED', claimed_by = $1, lease_expires_at = now() + interval '2 minutes'
WHERE id IN (
    SELECT id FROM schedule_occurrence
    WHERE  status    = 'PENDING'
    AND    shard_key % $shard_count = $shard_id
    AND    occurrence_dtm <= now()
    ORDER  BY occurrence_dtm
    LIMIT  $batch_size
    FOR UPDATE SKIP LOCKED
)
RETURNING *;
```

At `batch_size=100` with 8 workers, 800 occurrences are claimed per cycle without any lock convoy.

### Async defn_run rollup

Instance completions append lightweight `schedule_defn_run_event` rows rather than updating a shared counter directly. The Run Aggregator processes these in batches on a separate cadence. This removes the hot-row contention that would otherwise bottleneck at high instance fan-out.

---

## Versioning

Editing a `schedule_defn` is non-destructive:

1. A new row is inserted with `(same id, version + 1, state = ACTIVE)`.
2. The previous version's `effective_to` is set to `now()` and its `state` transitions to `SUPERSEDED`.
3. Both steps happen in one transaction.
4. Existing `schedule_instance` and `schedule_defn_run` rows retain their pinned `defn_version` — they are unaffected.
5. New enrolments and new occurrences generated after the version change use the new version.

A partial unique index enforces that at most one version per `(type, type_ref, name)` can be `ACTIVE` at a time:

```sql
CREATE UNIQUE INDEX defn_active_idx
    ON schedule_defn (type, type_ref, name) WHERE state = 'ACTIVE';
```

To roll back a version: transition the new version to `CLOSED` and re-activate the previous one. The `change_note` column is the audit trail.

---

## Outbox / Inbox Pattern

### Why

A naive approach calls the downstream API directly inside the fire transaction. If the API is unavailable the occurrence either fails silently or the transaction is held open. Neither is acceptable for financial or enterprise workloads.

The outbox pattern decouples the atomic database write from the network call:

```
fire transaction                outbox relay (separate goroutine)
─────────────────               ──────────────────────────────────
BEGIN                           loop:
  INSERT schedule_defn_run        SELECT … WHERE status='PENDING'
  INSERT schedule_instance_run    publish to broker
  INSERT schedule_outbox          UPDATE status='SENT'
  UPDATE schedule_occurrence
COMMIT
```

The broker is durable. Even if the relay crashes after publishing but before marking `SENT`, the relay retries and the broker delivers a duplicate. The `correlation_id` on the inbox side provides idempotency — a duplicate message is recognised and marked `DUPLICATE` without re-processing.

### Correlation ID lifecycle

```
schedule_outbox.correlation_id
        │ echoed in message payload
        ▼
schedule_inbox.correlation_id  ──► UNIQUE INDEX ──► duplicate detection
        │ matched on processing
        ▼
schedule_instance_run (status update)
```

---

## Overlap and Dependency Policies

### Overlap policy

Applies when an occurrence is ready to fire but the **previous run for the same definition** has not yet completed.

| Policy | Behaviour |
|---|---|
| `ALLOW` | Fire regardless. Both runs are active concurrently. |
| `SKIP` | Mark the new occurrence `SKIPPED`. No outbox messages are written. |
| `BUFFER` | Mark the new occurrence `BUFFERED`. Re-evaluate when the previous run completes. |

Overlap policy can be set at the definition level (applies to all instances) or overridden per instance.

### Dependency policy

Applies when a definition has declared dependencies via `schedule_defn_dep`. Before firing, the claimer checks whether the depended-on definition has a completed run for the same or an earlier occurrence date.

| Policy | Behaviour |
|---|---|
| `ALLOW` | Fire regardless of dependency run state. |
| `SKIP` | Skip if any dependency has not completed for this occurrence window. |
| `BUFFER` | Buffer until all dependencies have completed. |

---

## Further Reading

| Document | Contents |
|---|---|
| [docs/erd.md](docs/erd.md) | Full entity-relationship diagram |
| [docs/components.md](docs/components.md) | Component diagram with data-flow annotations |
| [docs/sequences.md](docs/sequences.md) | Sequence diagrams: normal fire, dead-worker recovery, version upgrade |
| [docs/schema.sql](docs/schema.sql) | Complete PostgreSQL DDL |
| [docs/kubernetes.md](docs/kubernetes.md) | Kubernetes hosting topology, workload mapping, and all hosting design decisions |
| [docs/docker-compose.md](docs/docker-compose.md) | Docker Compose topology for local development and simple single-host deployments; design decisions that differ from Kubernetes |
