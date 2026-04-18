# Sequence Diagrams

## 1. Normal Fire — Happy Path

The end-to-end flow from occurrence generation through to defn-run completion.

```mermaid
sequenceDiagram
    autonumber

    participant GEN  as Occurrence Generator
    participant DB   as PostgreSQL
    participant CLM  as Occurrence Claimer
    participant OR   as Outbox Relay
    participant BRK  as Kafka / Iggy
    participant DS   as Downstream System
    participant IR   as Inbox Relay
    participant IP   as Inbox Processor
    participant RA   as Run Aggregator

    Note over GEN,DB: Startup / periodic pre-generation

    GEN->>DB: read ACTIVE schedule_defn rows
    DB-->>GEN: defn list + watermarks
    GEN->>GEN: run tkone-schedule iterator per defn
    GEN->>DB: UPSERT schedule_occurrence (idempotent)
    GEN->>DB: UPDATE schedule_occurrence_watermark

    Note over CLM,DB: Claim cycle (runs every poll interval)

    CLM->>DB: UPDATE … SKIP LOCKED WHERE occurrence_dtm ≤ now()<br/>→ status = CLAIMED, lease set
    DB-->>CLM: batch of claimed occurrences

    loop for each claimed occurrence
        CLM->>DB: check overlap policy (last defn_run status)
        CLM->>DB: check dep policy (depended-on defn_run status)

        Note over CLM,DB: Atomic fire transaction
        CLM->>DB: INSERT schedule_defn_run (IN_PROGRESS)
        CLM->>DB: INSERT schedule_instance_run × N (PENDING)
        CLM->>DB: INSERT schedule_outbox × N (PENDING, unique correlation_id)
        CLM->>DB: UPDATE schedule_occurrence → FIRED
        DB-->>CLM: COMMIT
    end

    Note over OR,BRK: Outbox relay (independent loop)

    OR->>DB: SELECT … SKIP LOCKED WHERE outbox.status = PENDING
    DB-->>OR: batch of outbox rows
    OR->>BRK: publish message (topic, payload, correlation_id)
    BRK-->>OR: ack
    OR->>DB: UPDATE schedule_outbox → SENT

    BRK->>DS: deliver SCHEDULE_FIRED message
    DS->>DS: process job
    DS->>BRK: publish SCHEDULE_COMPLETED (correlation_id echoed)

    Note over IR,DB: Inbox relay (independent consumer loop)

    IR->>BRK: consume reply topic
    BRK-->>IR: SCHEDULE_COMPLETED message
    IR->>DB: INSERT schedule_inbox (PENDING, correlation_id)

    Note over IP,DB: Inbox processor

    IP->>DB: SELECT PENDING inbox rows (SKIP LOCKED)
    DB-->>IP: inbox row
    IP->>DB: check UNIQUE(correlation_id) — idempotency guard
    IP->>DB: UPDATE schedule_instance_run → COMPLETED
    IP->>DB: INSERT schedule_defn_run_event (COMPLETED)
    IP->>DB: UPDATE schedule_inbox → PROCESSED

    Note over RA,DB: Run aggregator (periodic batch)

    RA->>DB: SELECT new schedule_defn_run_event rows
    RA->>DB: UPDATE schedule_defn_run (completed_count++)
    RA->>DB: UPDATE schedule_defn_run → COMPLETED<br/>(when completed+failed == instance_count)
```

---

## 2. Dead Worker Lease Recovery

A claimer pod crashes after claiming occurrences but before firing them. The Worker Coordinator detects the stale lease and reschedules the work.

```mermaid
sequenceDiagram
    autonumber

    participant WA   as Worker A (claimer)
    participant DB   as PostgreSQL
    participant WC   as Worker Coordinator
    participant WB   as Worker B (claimer)

    WA->>DB: claim batch → status=CLAIMED, lease_expires_at=T+2m
    DB-->>WA: 10 occurrence rows claimed

    Note over WA: Worker A crashes (OOM, network partition, SIGKILL)

    Note over WC: Coordinator runs every 15 seconds

    WC->>DB: SELECT tempo_worker WHERE last_heartbeat < now()-30s AND state='ACTIVE'
    DB-->>WC: Worker A (stale heartbeat)

    WC->>DB: BEGIN
    WC->>DB: UPDATE schedule_occurrence SET status='PENDING'<br/>WHERE claimed_by='worker-a' AND status='CLAIMED'<br/>AND lease_expires_at < now()
    WC->>DB: UPDATE tempo_worker SET state='DEAD' WHERE node_id='worker-a'
    WC->>DB: COMMIT

    Note over WB: Worker B picks up on next poll cycle

    WB->>DB: UPDATE … SKIP LOCKED WHERE occurrence_dtm ≤ now()
    DB-->>WB: previously-claimed occurrences now PENDING again
    WB->>DB: fire transaction (defn_run, instance_runs, outbox)
    DB-->>WB: COMMIT

    Note over WB,DB: Normal processing resumes — no occurrences lost
```

---

## 3. Definition Version Upgrade

An operator updates a `schedule_defn`. Existing instances and in-flight runs are unaffected; the new version only governs future occurrences.

```mermaid
sequenceDiagram
    autonumber

    participant OPS  as Operator / API
    participant DB   as PostgreSQL
    participant GEN  as Occurrence Generator
    participant CLM  as Occurrence Claimer

    Note over OPS,DB: Version transition (atomic)

    OPS->>DB: BEGIN
    OPS->>DB: INSERT schedule_defn (id=X, version=2, state=ACTIVE,<br/>new run_spec, effective_from=now())
    OPS->>DB: UPDATE schedule_defn SET state='SUPERSEDED',<br/>effective_to=now()<br/>WHERE id=X AND version=1
    OPS->>DB: COMMIT

    Note over DB: Partial unique index ensures at most one ACTIVE<br/>version per (type, type_ref, name) — transaction<br/>would fail if two ACTIVE versions existed simultaneously

    Note over GEN,DB: Next generator cycle

    GEN->>DB: read ACTIVE schedule_defn — returns (id=X, version=2)
    GEN->>DB: read watermark for (X, version=2) — empty (new)
    GEN->>GEN: run iterator from now() with new run_spec
    GEN->>DB: INSERT schedule_occurrence (defn_id=X, defn_version=2)

    Note over CLM,DB: In-flight occurrences from v1

    CLM->>DB: claim PENDING occurrences (defn_version=1)
    Note over CLM: Still valid — pinned to v1 at generation time
    CLM->>DB: fire transaction against v1 instances

    Note over DB: v1 instances continue to run to completion<br/>New enrolments use v2<br/>Both versions coexist without conflict
```

---

## 4. Overlap Policy — BUFFER

An occurrence fires while the previous run for the same definition is still `IN_PROGRESS`. Overlap policy is `BUFFER`.

```mermaid
sequenceDiagram
    autonumber

    participant CLM  as Occurrence Claimer
    participant DB   as PostgreSQL
    participant RA   as Run Aggregator
    participant OR   as Outbox Relay

    Note over CLM,DB: First occurrence (T1) — fires normally

    CLM->>DB: claim occurrence T1
    CLM->>DB: fire tx — INSERT defn_run-1 (IN_PROGRESS)
    CLM->>DB: fire tx — INSERT instance_runs, outbox messages
    CLM->>DB: UPDATE occurrence T1 → FIRED

    Note over CLM,DB: Second occurrence (T2) arrives while T1 run still IN_PROGRESS

    CLM->>DB: claim occurrence T2
    CLM->>DB: SELECT last defn_run for this defn → defn_run-1 (IN_PROGRESS)
    Note over CLM: overlap_policy = BUFFER → do not fire yet

    CLM->>DB: BEGIN
    CLM->>DB: UPDATE occurrence T2 → BUFFERED
    CLM->>DB: COMMIT

    Note over DB: No defn_run-2 or outbox messages written yet

    Note over RA,DB: T1 run completes (all instance runs done)

    RA->>DB: UPDATE defn_run-1 → COMPLETED

    Note over CLM,DB: Claimer re-evaluates BUFFERED occurrences

    CLM->>DB: SELECT BUFFERED occurrences WHERE occurrence_dtm ≤ now()<br/>AND defn_run for previous occurrence = COMPLETED
    DB-->>CLM: occurrence T2

    CLM->>DB: UPDATE occurrence T2 → CLAIMED
    CLM->>DB: fire tx — INSERT defn_run-2 (IN_PROGRESS)
    CLM->>DB: fire tx — INSERT instance_runs, outbox messages
    CLM->>DB: UPDATE occurrence T2 → FIRED

    OR->>DB: pick up outbox messages for T2
    OR->>OR: publish to broker
```

---

## 5. Inbox Duplicate Detection

The Outbox Relay crashes after publishing to the broker but before marking the row `SENT`. The broker re-delivers. The Inbox Processor handles the duplicate safely.

```mermaid
sequenceDiagram
    autonumber

    participant OR   as Outbox Relay
    participant BRK  as Kafka / Iggy
    participant DS   as Downstream System
    participant IR   as Inbox Relay
    participant IP   as Inbox Processor
    participant DB   as PostgreSQL

    OR->>DB: SELECT PENDING outbox row (correlation_id = C1)
    OR->>BRK: publish (correlation_id = C1)
    BRK-->>OR: ack

    Note over OR: Relay crashes before marking SENT

    Note over BRK,DS: Message delivered normally

    DS->>DS: process job
    DS->>BRK: SCHEDULE_COMPLETED (correlation_id = C1)

    IR->>BRK: consume
    BRK-->>IR: message (correlation_id = C1)
    IR->>DB: INSERT schedule_inbox (correlation_id = C1, status = PENDING)

    IP->>DB: process — UPDATE instance_run → COMPLETED
    IP->>DB: UPDATE inbox → PROCESSED

    Note over OR: Relay restarts, retries the same outbox row

    OR->>DB: SELECT PENDING outbox row — finds same row (not marked SENT)
    OR->>BRK: publish again (correlation_id = C1) — duplicate
    BRK-->>OR: ack
    OR->>DB: UPDATE outbox → SENT

    DS->>BRK: SCHEDULE_COMPLETED (correlation_id = C1) — second delivery

    IR->>BRK: consume
    BRK-->>IR: duplicate message (correlation_id = C1)
    IR->>DB: INSERT schedule_inbox (correlation_id = C1)<br/>ON CONFLICT (correlation_id) → status = DUPLICATE

    IP->>DB: read inbox row — status = DUPLICATE
    Note over IP: Skip processing — no double-update to instance_run
    IP->>DB: UPDATE inbox → PROCESSED (no further action)
```
