# Docker Compose — Local and Simplified Deployment

Docker Compose provides two things Kubernetes cannot easily offer: a zero-prerequisite local development environment and a simple single-host production deployment for workloads that do not need horizontal scale.

---

## Contents

- [Service Topology](#service-topology)
- [Quick Start](#quick-start)
- [Design Decisions — Differences from Kubernetes](#design-decisions--differences-from-kubernetes)
  - [PgBouncer as a shared service](#1-pgbouncer-as-a-shared-service)
  - [Coordinator-managed shard assignment](#2-coordinator-managed-shard-assignment)
  - [PostgreSQL advisory locks for leader election](#3-postgresql-advisory-locks-for-leader-election)
  - [Manual scaling instead of KEDA](#4-manual-scaling-instead-of-keda)
  - [Container ID as node identity](#5-container-id-as-node-identity)
  - [Topic bootstrap via init container](#6-topic-bootstrap-via-init-container)
- [Compose Profiles](#compose-profiles)
- [Scaling Claimers](#scaling-claimers)
- [Alternative Broker — Iggy](#alternative-broker--iggy)
- [Partition Management](#partition-management)
- [Environment Variables Reference](#environment-variables-reference)
- [Comparison Table — Docker Compose vs Kubernetes](#comparison-table--docker-compose-vs-kubernetes)

---

## Service Topology

```mermaid
graph TB
    subgraph Compose["docker-compose.yml"]

        subgraph Infra["Infrastructure (always up)"]
            PG["postgres:16-alpine\n─────────────────\nport 5432"]
            PGB["bitnami/pgbouncer\n─────────────────\ntransaction mode\nport 5433 (host)"]
            RP["redpanda\n─────────────────\nKafka-compatible\nport 9092"]
            RPI["redpanda-init\n─────────────────\ncreates topics\n(one-shot, exits)"]
            RPC["redpanda-console\n─────────────────\nbrowser UI\nport 8080"]
        end

        subgraph Workers["Application Workers (profile: workers)"]
            GEN["tempo-generator"]
            COORD["tempo-coordinator"]
            CLM["tempo-claimer\n(scalable)"]
            OR["tempo-outbox-relay"]
            IR["tempo-inbox-relay"]
            IP["tempo-inbox-processor"]
            RA["tempo-run-aggregator"]
        end
    end

    PG    -->|"healthcheck ready"| PGB
    RP    -->|"healthcheck ready"| RPI
    RPI   -->|"topics created"| GEN
    RPI   -->|"topics created"| CLM
    RPI   -->|"topics created"| OR
    RPI   -->|"topics created"| IR
    PGB   -->|"all workers connect via"| PGB
    COORD -->|"assigns shards to"| CLM

    GEN   -->|"writes occurrences"| PGB
    CLM   -->|"SKIP LOCKED claim"| PGB
    CLM   -->|"fire outbox"| PGB
    OR    -->|"reads outbox"| PGB
    OR    -->|"publishes"| RP
    IR    -->|"consumes"| RP
    IR    -->|"writes inbox"| PGB
    IP    -->|"reads inbox"| PGB
    RA    -->|"reads events"| PGB
```

---

## Quick Start

```bash
# 1. Copy and optionally edit environment variables
cp .env.example .env

# 2. Start infrastructure only (Postgres, PgBouncer, Redpanda)
docker compose up -d

# 3. Verify infrastructure is healthy
docker compose ps

# 4. Start all Tempo workers
docker compose --profile workers up -d

# 5. Watch logs
docker compose logs -f

# 6. Open the Redpanda broker UI
open http://localhost:8080
```

To tear down completely (preserving the Postgres volume):
```bash
docker compose --profile workers down
```

To also wipe the database:
```bash
docker compose --profile workers down -v
```

---

## Design Decisions — Differences from Kubernetes

### 1. PgBouncer as a shared service

**In Kubernetes:** PgBouncer runs as a sidecar container in every pod. This eliminates a network hop and means a PgBouncer failure takes only its pod offline, not the whole application.

**In Docker Compose:** PgBouncer runs as a named service that all worker containers share. A sidecar pattern is possible in Compose (putting all containers in one service using multiple `command` entries is awkward), but the operational benefit in a single-host deployment does not justify the complexity.

**Consequence:** If PgBouncer crashes in Docker Compose, all workers temporarily lose their DB proxy. Docker Compose's `restart: unless-stopped` brings it back within seconds. In Kubernetes, the sidecar sidecar crash only affects one pod and Kubernetes restarts it automatically — no cluster-wide impact.

The connection budget in Docker Compose is smaller than Kubernetes anyway (`default_pool_size=10` × a handful of containers), so a shared service is not a bottleneck.

---

### 2. Coordinator-managed shard assignment

**In Kubernetes:** Each claimer pod derives its shard range from its StatefulSet ordinal and the total replica count at runtime. No database involvement needed for shard routing.

**In Docker Compose:** There are no stable ordinals. When workers are scaled with `--scale tempo-claimer=N`, all replicas share the same environment variables — ordinal-based derivation is not possible.

Instead, each claimer registers itself in `tempo_worker` with `shard_lo = NULL` on startup. The Coordinator detects unassigned claimers (any `tempo_worker` row with `role = 'CLAIMER'` and `shard_lo IS NULL`) and distributes the 0–255 shard space evenly:

```
1 claimer → owns shards 0–255
2 claimers → 0–127 and 128–255
4 claimers → 0–63, 64–127, 128–191, 192–255
```

The Coordinator writes the assigned `shard_lo` / `shard_hi` back to `tempo_worker`. Each claimer reads its own row by `node_id` and begins claiming against that range. Scaling up adds new unassigned rows which the Coordinator picks up on the next cycle; scaling down lets the dead rows expire via `lease_expires_at`.

Set `TEMPO_SHARD_BACKEND=postgres` in the claimer environment (the default in `docker-compose.yml`) to activate this path. `TEMPO_SHARD_BACKEND=statefulset` (the Kubernetes default) uses the ordinal path instead.

---

### 3. PostgreSQL advisory locks for leader election

**In Kubernetes:** The Generator and Coordinator use the `coordination.k8s.io/v1` Lease API. The Lease has a configurable TTL; when a pod dies, the TTL expires and the standby pod acquires the Lease within that window.

**In Docker Compose:** There is no Kubernetes API server. Instead, `pg_try_advisory_lock` provides equivalent semantics:

```sql
-- Attempt to acquire the generator leader lock (non-blocking)
SELECT pg_try_advisory_lock(hashtext('tempo-generator'));
-- Returns true if acquired, false if another session holds it
```

The lock is tied to the PostgreSQL **session** (not transaction). If the holder crashes, the TCP connection drops, and PostgreSQL releases the lock automatically — driven by `tcp_keepidle=30` set in `pgbouncer.ini`, which propagates keepalive settings to the server-side connection.

**Key difference from k8s Lease:** The release latency is `tcp_keepidle` (30s) rather than the Lease TTL (configurable, typically 15s). For local development and simple deployments this difference is immaterial.

Set `TEMPO_LEADER_BACKEND=postgres` (the default in `docker-compose.yml`) to activate advisory locks. `TEMPO_LEADER_BACKEND=k8s` activates the Kubernetes Lease path.

---

### 4. Manual scaling instead of KEDA

**In Kubernetes:** KEDA ScaledObjects watch PostgreSQL queue depths and broker consumer lag, scaling relay and processor Deployments automatically.

**In Docker Compose:** KEDA is not available. Scaling is manual:

```bash
# Scale outbox relay to 3 replicas when queue depth is high
docker compose --profile workers up -d --scale tempo-outbox-relay=3

# Scale back down
docker compose --profile workers up -d --scale tempo-outbox-relay=1
```

For local development this is fully sufficient. For a simple single-host production deployment, a cron job or lightweight monitoring script can trigger `docker compose` rescale commands based on queue depth queries.

---

### 5. Container ID as node identity

**In Kubernetes:** Pod name is injected via the Downward API (`metadata.name`) — stable, human-readable, unique.

**In Docker Compose:** When scaled with `--scale`, containers are named `<service>-<index>` (e.g. `tempo-claimer-3`), but this naming is not guaranteed to be stable across restarts.

The Tempo entrypoint resolves `node_id` at startup:

```
TEMPO_NODE_ID env var  →  use as-is (set explicitly in .env or compose override)
TEMPO_NODE_ID=""       →  fall back to /proc/self/cgroup container ID (first 12 chars)
```

The container ID is stable for the lifetime of the container and unique on the host. It appears in `docker ps` output, making it easy to correlate `tempo_worker` rows with running containers.

For fixed single-replica services (generator, coordinator, relays), `TEMPO_NODE_ID` is set explicitly in `docker-compose.yml` to a human-readable name. For scalable claimers it is left empty so each replica gets its own container-ID-based name.

---

### 6. Topic bootstrap via init container

**In Kubernetes:** Topics are created by a Kubernetes Job or a Helm hook before workers start.

**In Docker Compose:** The `redpanda-init` service creates the required topics using `rpk` and exits (`restart: "no"`). Worker services declare `depends_on: redpanda-init: condition: service_completed_successfully`, ensuring topics exist before any producer or consumer starts.

```yaml
redpanda-init:
  image: redpandadata/redpanda:v24.1.1
  entrypoint: ["/bin/sh", "-c"]
  command:
    - |
      rpk -X brokers=redpanda:9092 topic create \
        tempo.schedule.fired \
        tempo.schedule.completed \
        --partitions 12 --replicas 1
  restart: "no"
```

---

## Compose Profiles

| Command | What starts |
|---|---|
| `docker compose up -d` | `postgres`, `pgbouncer`, `redpanda`, `redpanda-init`, `redpanda-console` |
| `docker compose --profile workers up -d` | All of the above + all Tempo worker services |

Infrastructure-only mode is the recommended starting point for application development — the schema is initialised, the broker is ready, and the application can be run locally with `cargo run` pointing at the Compose-managed services via the host-side ports.

---

## Scaling Claimers

```bash
# Start with 1 claimer (default)
docker compose --profile workers up -d

# Scale to 4 claimers
docker compose --profile workers up -d --scale tempo-claimer=4

# The coordinator detects 3 new unassigned workers and redistributes shards:
#   tempo-claimer-1  → shards 0–63
#   tempo-claimer-2  → shards 64–127
#   tempo-claimer-3  → shards 128–191
#   tempo-claimer-4  → shards 192–255

# Scale back to 2 (Compose stops the highest-indexed containers)
docker compose --profile workers up -d --scale tempo-claimer=2
# Coordinator reclaims dead leases; surviving claimers expand their ranges
```

---

## Broker Choice

### Why Redpanda is the default

Redpanda is a single binary, needs no ZooKeeper, ships with the `rpk` CLI, and is Kafka wire-protocol compatible. It is the lowest-friction local broker that is also production-grade.

### Kafka ↔ Redpanda — zero-change swap

Kafka and Redpanda speak the same wire protocol. The `rdkafka` Rust crate talks to both identically. Switching is an infrastructure decision only:

```bash
# .env — point at a Kafka bootstrap instead
TEMPO_BROKER_BACKEND=kafka
TEMPO_BROKER_BOOTSTRAP=kafka-broker:9092
```

No code changes, no compose changes beyond the service definition.

### Iggy — same logic, different client

[Iggy](https://iggy.rs) is a native Rust streaming server with its own binary protocol (not Kafka-compatible). Tempo abstracts the broker behind a `BrokerBackend` trait (see [components.md](components.md)), so the Outbox Relay and Inbox Relay are unaffected — only the concrete backend implementation differs.

To run with Iggy, use the compose override file:

```bash
docker compose -f docker-compose.yml -f docker-compose.iggy.yml up -d
docker compose -f docker-compose.yml -f docker-compose.iggy.yml --profile workers up -d
```

`docker-compose.iggy.yml` replaces the `redpanda` / `redpanda-init` / `redpanda-console` services with an `iggy` service and overrides `TEMPO_BROKER_BACKEND` and `TEMPO_BROKER_BOOTSTRAP` on all workers:

```yaml
# docker-compose.iggy.yml  (overlay — do not use standalone)
services:
  redpanda:
    profiles: ["_disabled"]       # effectively removes the service

  redpanda-init:
    profiles: ["_disabled"]

  redpanda-console:
    profiles: ["_disabled"]

  iggy:
    image: iggyrs/iggy:latest
    restart: unless-stopped
    ports:
      - "8090:8090"    # binary TCP protocol
      - "3000:3000"    # HTTP API
      - "8080:8080"    # web UI
    volumes:
      - iggy_data:/var/lib/iggy
    healthcheck:
      test: ["CMD-SHELL", "curl -sf http://localhost:3000/api/ping || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 6

  # Override broker env on every worker
  tempo-generator:
    environment:
      TEMPO_BROKER_BACKEND:    iggy
      TEMPO_BROKER_BOOTSTRAP:  iggy:8090
    depends_on:
      iggy:
        condition: service_healthy

  tempo-claimer:
    environment:
      TEMPO_BROKER_BACKEND:    iggy
      TEMPO_BROKER_BOOTSTRAP:  iggy:8090
    depends_on:
      iggy:
        condition: service_healthy

  tempo-outbox-relay:
    environment:
      TEMPO_BROKER_BACKEND:    iggy
      TEMPO_BROKER_BOOTSTRAP:  iggy:8090
    depends_on:
      iggy:
        condition: service_healthy

  tempo-inbox-relay:
    environment:
      TEMPO_BROKER_BACKEND:    iggy
      TEMPO_BROKER_BOOTSTRAP:  iggy:8090
    depends_on:
      iggy:
        condition: service_healthy

volumes:
  iggy_data:
```

Iggy creates streams and topics on first use if they do not exist, so no init job is needed.

---

## Partition Management

The `docker/init-partitions.sql` script runs automatically on first Postgres container start (via `/docker-entrypoint-initdb.d/`). It creates monthly partitions for all five partitioned tables from the current month through 14 months ahead.

On a long-running Compose deployment, partitions must be extended periodically:

```bash
# Re-run the partition script against a running Postgres instance
docker compose exec postgres psql -U tempo -d tempo \
  -f /docker-entrypoint-initdb.d/02-partitions.sql
```

Schedule this as a monthly cron job on the host, or use `pg_cron` inside Postgres. In Kubernetes, `pg_partman` handles this automatically.

---

## Environment Variables Reference

| Variable | Default | Description |
|---|---|---|
| `POSTGRES_DB` | `tempo` | Database name |
| `POSTGRES_USER` | `tempo` | Database user |
| `POSTGRES_PASSWORD` | `tempo` | Database password |
| `POSTGRES_PORT` | `5432` | Host-side Postgres port |
| `PGBOUNCER_PORT` | `5433` | Host-side PgBouncer port |
| `REDPANDA_KAFKA_PORT` | `9092` | Kafka API port |
| `REDPANDA_CONSOLE_PORT` | `8080` | Browser console port |
| `TEMPO_DB_URL` | `postgres://tempo:tempo@pgbouncer:5432/tempo` | DSN passed to all workers |
| `TEMPO_BROKER_BOOTSTRAP` | `redpanda:9092` | Broker address for producers/consumers |
| `TEMPO_OUTBOX_TOPIC` | `tempo.schedule.fired` | Topic outbox relay publishes to |
| `TEMPO_INBOX_TOPIC` | `tempo.schedule.completed` | Topic inbox relay consumes from |
| `RUST_LOG` | `info` | Tracing filter for all workers |

---

## Comparison Table — Docker Compose vs Kubernetes

| Concern | Docker Compose | Kubernetes |
|---|---|---|
| **PgBouncer** | Shared service | Sidecar per pod |
| **Shard assignment** | Coordinator-managed (DB rows) | StatefulSet ordinal (`TEMPO_ORDINAL`) |
| **Leader election** | PostgreSQL advisory lock | `coordination.k8s.io/v1` Lease |
| **Autoscaling** | Manual `--scale` | KEDA ScaledObject on queue depth |
| **Node identity** | Container ID or explicit name | Pod name via Downward API |
| **Topic creation** | `redpanda-init` one-shot service | Kubernetes Job / Helm hook |
| **Graceful shutdown** | SIGTERM (same) | SIGTERM (same) |
| **Partition management** | Manual cron / `pg_cron` | `pg_partman` |
| **Health gating** | Docker healthcheck | Kubernetes readiness probe |
| **Broker swap** | Override with `docker-compose.iggy.yml`; `TEMPO_BROKER_BACKEND` env var | Same env var; broker service swapped in Helm values |
| **Config delivery** | `.env` file | ConfigMap + Secret |
