-- =============================================================================
-- Tempo — PostgreSQL Schema
-- =============================================================================
-- All tables holding high-churn data (schedule_occurrence, schedule_instance_run,
-- schedule_defn_run_event, schedule_outbox, schedule_inbox) are declared as
-- range-partitioned by time. Use pg_partman to automate partition creation and
-- old-partition archival.
--
-- Naming conventions:
--   ts  = TIMESTAMP WITH TIME ZONE (timestamptz)
--   PK  = PRIMARY KEY
--   UK  = UNIQUE
--   FK  = FOREIGN KEY
-- =============================================================================


-- =============================================================================
-- Enum types
-- =============================================================================

CREATE TYPE calendar_state          AS ENUM ('ACTIVE', 'INACTIVE');

CREATE TYPE defn_state              AS ENUM ('DRAFT', 'ACTIVE', 'SUPERSEDED', 'CLOSED');

CREATE TYPE inst_state              AS ENUM ('ACTIVE', 'INACTIVE', 'CLOSED');

-- Unified status used for defn_run and instance_run
CREATE TYPE run_status              AS ENUM (
    'PENDING',
    'IN_PROGRESS',
    'COMPLETED',
    'BUFFERED',
    'SKIPPED',
    'FAILED'
);

-- Pre-generation / claim lifecycle for schedule_occurrence
CREATE TYPE occurrence_status       AS ENUM (
    'PENDING',
    'CLAIMED',
    'FIRED',
    'COMPLETED',
    'FAILED',
    'SKIPPED',
    'BUFFERED'
);

-- Mirrors tkone_schedule::Occurrence<T> variants
CREATE TYPE occurrence_kind         AS ENUM (
    'EXACT',
    'ADJUSTED_LATER',
    'ADJUSTED_EARLIER'
);

CREATE TYPE overlap_policy          AS ENUM ('BUFFER', 'ALLOW', 'SKIP');

CREATE TYPE dep_policy              AS ENUM ('BUFFER', 'ALLOW', 'SKIP');

CREATE TYPE run_event_type          AS ENUM (
    'FIRED',
    'COMPLETED',
    'FAILED',
    'SKIPPED',
    'BUFFERED'
);

CREATE TYPE outbox_status           AS ENUM ('PENDING', 'SENT', 'FAILED');

CREATE TYPE inbox_status            AS ENUM ('PENDING', 'PROCESSED', 'FAILED', 'DUPLICATE');

CREATE TYPE worker_state            AS ENUM ('ACTIVE', 'DRAINING', 'DEAD');

CREATE TYPE worker_role             AS ENUM (
    'CLAIMER',
    'OUTBOX_RELAY',
    'INBOX_RELAY',
    'INBOX_PROCESSOR',
    'RUN_AGGREGATOR',
    'COORDINATOR',
    'GENERATOR'
);


-- =============================================================================
-- Calendar
-- =============================================================================

CREATE TABLE calendar (
    id              uuid            NOT NULL DEFAULT gen_random_uuid(),
    name            varchar(100)    NOT NULL,
    timezone        varchar(100)    NOT NULL,
    is_default      boolean         NOT NULL DEFAULT false,
    -- { "mon": ["09:00","17:00"], "tue": ["09:00","17:00"], … }
    business_hours  jsonb           NOT NULL DEFAULT '{}',
    state           calendar_state  NOT NULL DEFAULT 'ACTIVE',
    created_at      timestamptz     NOT NULL DEFAULT now(),
    updated_at      timestamptz     NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    CONSTRAINT calendar_name_uk UNIQUE (name)
);

CREATE TABLE calendar_holiday (
    id          uuid        NOT NULL DEFAULT gen_random_uuid(),
    calendar_id uuid        NOT NULL,
    name        varchar(255),
    start_dtm   timestamptz NOT NULL,
    end_dtm     timestamptz NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    CONSTRAINT holiday_range_valid CHECK (end_dtm > start_dtm),
    CONSTRAINT fk_calendar_holiday_calendar
        FOREIGN KEY (calendar_id) REFERENCES calendar (id)
        ON UPDATE NO ACTION ON DELETE RESTRICT
);

CREATE INDEX calendar_holiday_calendar_idx
    ON calendar_holiday (calendar_id, start_dtm);


-- =============================================================================
-- Schedule Definition (versioned)
-- =============================================================================
--
-- `id` is the stable logical identifier, never changes across versions.
-- `version` increments on each update.  PK = (id, version).
--
-- run_spec JSON shape:
--   {
--     "spec":      "YY-1M-L~NBT11:00:00",   -- tkone-schedule spec string
--     "timezone":  "Europe/London",
--     "lookahead_days":    90,               -- optional generation horizon
--     "max_occurrences":   500               -- optional generation limit
--   }

CREATE TABLE schedule_defn (
    id              uuid            NOT NULL,
    version         integer         NOT NULL DEFAULT 1,
    type            varchar(100)    NOT NULL,
    type_ref        varchar(100)    NOT NULL,
    name            varchar(100)    NOT NULL,
    one_off         boolean         NOT NULL DEFAULT false,
    run_spec        jsonb           NOT NULL,
    calendar_id     uuid            NOT NULL,
    overlap_policy  overlap_policy  NOT NULL DEFAULT 'ALLOW',
    dep_policy      dep_policy      NOT NULL DEFAULT 'ALLOW',
    state           defn_state      NOT NULL DEFAULT 'DRAFT',
    effective_from  timestamptz     NOT NULL DEFAULT now(),
    effective_to    timestamptz,                            -- NULL = current version
    change_note     text,
    created_at      timestamptz     NOT NULL DEFAULT now(),
    created_by      varchar(255),
    PRIMARY KEY (id, version),
    CONSTRAINT fk_schedule_defn_calendar
        FOREIGN KEY (calendar_id) REFERENCES calendar (id)
        ON UPDATE NO ACTION ON DELETE RESTRICT
);

-- At most one ACTIVE version per logical key
CREATE UNIQUE INDEX defn_active_uk
    ON schedule_defn (type, type_ref, name)
    WHERE state = 'ACTIVE';

-- Fast lookup of current (un-superseded) version
CREATE INDEX defn_current_idx
    ON schedule_defn (id)
    WHERE effective_to IS NULL;


-- =============================================================================
-- Schedule Definition Dependencies
-- =============================================================================

CREATE TABLE schedule_defn_dep (
    defn_id                 uuid        NOT NULL,
    defn_version            integer     NOT NULL,
    depends_on_defn_id      uuid        NOT NULL,
    depends_on_defn_version integer     NOT NULL,
    dep_policy              dep_policy  NOT NULL DEFAULT 'BUFFER',
    PRIMARY KEY (defn_id, defn_version, depends_on_defn_id),
    CONSTRAINT fk_defn_dep_defn
        FOREIGN KEY (defn_id, defn_version)
        REFERENCES schedule_defn (id, version),
    CONSTRAINT fk_defn_dep_depends_on
        FOREIGN KEY (depends_on_defn_id, depends_on_defn_version)
        REFERENCES schedule_defn (id, version)
);


-- =============================================================================
-- Schedule Instances
-- =============================================================================
--
-- An instance is a subscription: one external entity enrolled in a definition.
-- Pinned to the defn version at enrolment time.
-- One entity (instance_ref) per definition (defn_id) — enforced by unique index.

CREATE TABLE schedule_instance (
    id              uuid            NOT NULL DEFAULT gen_random_uuid(),
    defn_id         uuid            NOT NULL,
    defn_version    integer         NOT NULL,
    instance_ref    varchar(100)    NOT NULL,
    one_off         boolean         NOT NULL DEFAULT false,
    run_spec        jsonb,                                  -- overrides defn run_spec when set
    overlap_policy  overlap_policy  NOT NULL DEFAULT 'ALLOW',
    state           inst_state      NOT NULL DEFAULT 'ACTIVE',
    created_at      timestamptz     NOT NULL DEFAULT now(),
    updated_at      timestamptz     NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    CONSTRAINT fk_instance_defn
        FOREIGN KEY (defn_id, defn_version)
        REFERENCES schedule_defn (id, version),
    CONSTRAINT instance_defn_ref_uk
        UNIQUE (defn_id, instance_ref)
);


-- =============================================================================
-- Pre-generated Occurrences
-- =============================================================================
--
-- Mirrors tkone_schedule::Occurrence<T>:
--   actual_dtm     = Occurrence::actual()     — raw calendar date
--   occurrence_dtm = Occurrence::observed()   — settlement / business-day-adjusted date
--
-- shard_key is a computed column used to partition the SKIP LOCKED hot path
-- across N claimer workers without inter-worker coordination.
--
-- Partitioned by occurrence_dtm (RANGE, monthly).

CREATE TABLE schedule_occurrence (
    id              uuid                NOT NULL DEFAULT gen_random_uuid(),
    defn_id         uuid                NOT NULL,
    defn_version    integer             NOT NULL,
    kind            occurrence_kind     NOT NULL DEFAULT 'EXACT',
    actual_dtm      timestamptz         NOT NULL,
    occurrence_dtm  timestamptz         NOT NULL,
    shard_key       smallint            NOT NULL
                        GENERATED ALWAYS AS (abs(hashtext(defn_id::text)) % 256) STORED,
    status          occurrence_status   NOT NULL DEFAULT 'PENDING',
    claimed_by      varchar(255),
    claimed_at      timestamptz,
    lease_expires_at timestamptz,
    fired_at        timestamptz,
    created_at      timestamptz         NOT NULL DEFAULT now(),
    PRIMARY KEY (id, occurrence_dtm),
    CONSTRAINT occurrence_defn_dtm_uk
        UNIQUE (defn_id, defn_version, occurrence_dtm),
    CONSTRAINT occurrence_kind_dates_check CHECK (
        (kind = 'EXACT'             AND actual_dtm = occurrence_dtm) OR
        (kind = 'ADJUSTED_LATER'    AND occurrence_dtm > actual_dtm) OR
        (kind = 'ADJUSTED_EARLIER'  AND occurrence_dtm < actual_dtm)
    ),
    CONSTRAINT fk_occurrence_defn
        FOREIGN KEY (defn_id, defn_version)
        REFERENCES schedule_defn (id, version)
) PARTITION BY RANGE (occurrence_dtm);

-- Hot path: claimer shard-filtered PENDING occurrences
CREATE INDEX occurrence_shard_pending_idx
    ON schedule_occurrence (shard_key, occurrence_dtm)
    WHERE status = 'PENDING';

-- Watchdog: find expired leases for dead-worker recovery
CREATE INDEX occurrence_lease_expiry_idx
    ON schedule_occurrence (lease_expires_at)
    WHERE status = 'CLAIMED';

-- Example monthly partition (create via pg_partman in production)
CREATE TABLE schedule_occurrence_2026_04
    PARTITION OF schedule_occurrence
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');


-- =============================================================================
-- Occurrence Watermark
-- =============================================================================

CREATE TABLE schedule_occurrence_watermark (
    defn_id             uuid        NOT NULL,
    defn_version        integer     NOT NULL,
    last_occurrence_dtm timestamptz NOT NULL,
    updated_at          timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (defn_id, defn_version),
    CONSTRAINT fk_watermark_defn
        FOREIGN KEY (defn_id, defn_version)
        REFERENCES schedule_defn (id, version)
);


-- =============================================================================
-- Definition-level Run
-- =============================================================================
--
-- One row per occurrence per definition version.
-- instance_count / completed_count / failed_count are updated asynchronously
-- by the Run Aggregator via schedule_defn_run_event rows.

CREATE TABLE schedule_defn_run (
    id              uuid        NOT NULL DEFAULT gen_random_uuid(),
    defn_id         uuid        NOT NULL,
    defn_version    integer     NOT NULL,
    occurrence_id   uuid        NOT NULL,
    actual_dtm      timestamptz NOT NULL,
    occurrence_dtm  timestamptz NOT NULL,
    status          run_status  NOT NULL DEFAULT 'PENDING',
    instance_count  integer     NOT NULL DEFAULT 0,
    completed_count integer     NOT NULL DEFAULT 0,
    failed_count    integer     NOT NULL DEFAULT 0,
    started_at      timestamptz,
    completed_at    timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    CONSTRAINT defn_run_occurrence_uk
        UNIQUE (defn_id, defn_version, occurrence_id),
    CONSTRAINT fk_defn_run_defn
        FOREIGN KEY (defn_id, defn_version)
        REFERENCES schedule_defn (id, version),
    CONSTRAINT fk_defn_run_occurrence
        FOREIGN KEY (occurrence_id)
        REFERENCES schedule_occurrence (id)
);

CREATE INDEX defn_run_defn_status_idx
    ON schedule_defn_run (defn_id, defn_version, status);


-- =============================================================================
-- Instance-level Run
-- =============================================================================
--
-- One row per active instance per occurrence.
-- Partitioned by created_at (RANGE, monthly).

CREATE TABLE schedule_instance_run (
    id              uuid        NOT NULL DEFAULT gen_random_uuid(),
    instance_id     uuid        NOT NULL,
    defn_run_id     uuid        NOT NULL,
    occurrence_dtm  timestamptz NOT NULL,
    status          run_status  NOT NULL DEFAULT 'PENDING',
    started_at      timestamptz,
    completed_at    timestamptz,
    error_message   text,
    metadata        jsonb,
    created_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id, created_at),
    CONSTRAINT fk_instance_run_instance
        FOREIGN KEY (instance_id) REFERENCES schedule_instance (id),
    CONSTRAINT fk_instance_run_defn_run
        FOREIGN KEY (defn_run_id) REFERENCES schedule_defn_run (id)
) PARTITION BY RANGE (created_at);

CREATE INDEX instance_run_defn_run_idx
    ON schedule_instance_run (defn_run_id);
CREATE INDEX instance_run_instance_idx
    ON schedule_instance_run (instance_id, occurrence_dtm);

CREATE TABLE schedule_instance_run_2026_04
    PARTITION OF schedule_instance_run
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');


-- =============================================================================
-- Definition Run Event  (async rollup source)
-- =============================================================================
--
-- Completions append cheap event rows here rather than hot-updating
-- schedule_defn_run counters directly.  The Run Aggregator batches these.
-- Partitioned by occurred_at (RANGE, monthly).

CREATE TABLE schedule_defn_run_event (
    id              uuid            NOT NULL DEFAULT gen_random_uuid(),
    defn_run_id     uuid            NOT NULL,
    instance_run_id uuid,
    event_type      run_event_type  NOT NULL,
    occurred_at     timestamptz     NOT NULL DEFAULT now(),
    PRIMARY KEY (id, occurred_at),
    CONSTRAINT fk_run_event_defn_run
        FOREIGN KEY (defn_run_id) REFERENCES schedule_defn_run (id)
) PARTITION BY RANGE (occurred_at);

CREATE INDEX run_event_defn_run_idx
    ON schedule_defn_run_event (defn_run_id, event_type);

CREATE TABLE schedule_defn_run_event_2026_04
    PARTITION OF schedule_defn_run_event
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');


-- =============================================================================
-- Outbox  (transactional outbox → Kafka / Iggy)
-- =============================================================================
--
-- Written atomically inside the same transaction that transitions an
-- occurrence to FIRED.  The Outbox Relay publishes and marks rows SENT.
-- Partitioned by created_at (RANGE, weekly or monthly).

CREATE TABLE schedule_outbox (
    id              uuid            NOT NULL DEFAULT gen_random_uuid(),
    created_at      timestamptz     NOT NULL DEFAULT now(),
    topic           varchar(255)    NOT NULL,
    partition_key   varchar(255),
    payload         jsonb           NOT NULL,
    headers         jsonb,
    status          outbox_status   NOT NULL DEFAULT 'PENDING',
    attempts        integer         NOT NULL DEFAULT 0,
    last_attempt_at timestamptz,
    sent_at         timestamptz,
    aggregate_type  varchar(50)     NOT NULL,    -- 'defn_run' | 'instance_run'
    aggregate_id    uuid            NOT NULL,
    correlation_id  uuid            NOT NULL DEFAULT gen_random_uuid(),
    PRIMARY KEY (id, created_at),
    CONSTRAINT outbox_correlation_uk UNIQUE (correlation_id)
) PARTITION BY RANGE (created_at);

CREATE INDEX outbox_pending_idx
    ON schedule_outbox (created_at)
    WHERE status = 'PENDING';
CREATE INDEX outbox_retry_idx
    ON schedule_outbox (attempts, last_attempt_at)
    WHERE status = 'FAILED';

CREATE TABLE schedule_outbox_2026_04
    PARTITION OF schedule_outbox
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');


-- =============================================================================
-- Inbox  (idempotent reception of completion messages)
-- =============================================================================
--
-- The Inbox Relay writes raw messages here.  The Inbox Processor deduplicates
-- on correlation_id and updates schedule_instance_run.
-- Partitioned by received_at (RANGE, monthly).

CREATE TABLE schedule_inbox (
    id              uuid            NOT NULL DEFAULT gen_random_uuid(),
    received_at     timestamptz     NOT NULL DEFAULT now(),
    topic           varchar(255)    NOT NULL,
    message_key     varchar(255),
    payload         jsonb           NOT NULL,
    headers         jsonb,
    correlation_id  uuid,
    status          inbox_status    NOT NULL DEFAULT 'PENDING',
    instance_run_id uuid,
    processed_at    timestamptz,
    error_message   text,
    PRIMARY KEY (id, received_at),
    CONSTRAINT inbox_correlation_uk
        UNIQUE (correlation_id)
        WHERE correlation_id IS NOT NULL
) PARTITION BY RANGE (received_at);

CREATE INDEX inbox_pending_idx
    ON schedule_inbox (received_at)
    WHERE status = 'PENDING';

CREATE TABLE schedule_inbox_2026_04
    PARTITION OF schedule_inbox
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');


-- =============================================================================
-- Worker Registry
-- =============================================================================
--
-- Tempo is deployed in a single Kubernetes cluster, single namespace.
--
-- node_id   = Kubernetes pod name (metadata.name via Downward API).
--             Unique within the namespace; correlates directly to kubectl output.
-- ordinal   = StatefulSet pod index for claimer pods (e.g. tempo-claimer-2 → 2).
--             NULL for Deployment pods (stateless roles).
-- shard_lo / shard_hi = informational only; computed from ordinal + replica count
--             at startup and stored here for observability dashboards.
--             The claimer derives its live shard range from the TEMPO_ORDINAL
--             environment variable — these columns are never read for routing.

CREATE TABLE tempo_worker (
    id              uuid            NOT NULL DEFAULT gen_random_uuid(),
    node_id         varchar(255)    NOT NULL,
    role            worker_role     NOT NULL,
    ordinal         integer,
    shard_lo        smallint,
    shard_hi        smallint,
    last_heartbeat  timestamptz     NOT NULL DEFAULT now(),
    started_at      timestamptz     NOT NULL DEFAULT now(),
    state           worker_state    NOT NULL DEFAULT 'ACTIVE',
    PRIMARY KEY (id),
    CONSTRAINT worker_node_id_uk UNIQUE (node_id),
    CONSTRAINT worker_shard_range_check CHECK (
        (shard_lo IS NULL AND shard_hi IS NULL) OR
        (shard_lo IS NOT NULL AND shard_hi IS NOT NULL AND shard_lo <= shard_hi)
    ),
    CONSTRAINT worker_ordinal_shard_check CHECK (
        -- ordinal only meaningful for StatefulSet (claimer) pods
        (ordinal IS NULL) OR (shard_lo IS NOT NULL AND shard_hi IS NOT NULL)
    )
);

CREATE INDEX worker_heartbeat_active_idx
    ON tempo_worker (last_heartbeat)
    WHERE state = 'ACTIVE';
