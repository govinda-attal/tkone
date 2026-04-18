-- =============================================================================
-- Auto-create time-range partitions for the current month + 13 months ahead.
-- Run once on first startup; idempotent (uses CREATE TABLE IF NOT EXISTS).
-- In production (Kubernetes), pg_partman handles this automatically.
-- =============================================================================

DO $$
DECLARE
    start_date date := date_trunc('month', now())::date;
    end_date   date := start_date + interval '14 months';
    cur_date   date := start_date;
    nxt_date   date;
    suffix     text;
BEGIN
    WHILE cur_date < end_date LOOP
        nxt_date := cur_date + interval '1 month';
        suffix   := to_char(cur_date, 'YYYY_MM');

        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS schedule_occurrence_%s
             PARTITION OF schedule_occurrence
             FOR VALUES FROM (%L::timestamptz) TO (%L::timestamptz)',
            suffix, cur_date, nxt_date
        );

        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS schedule_instance_run_%s
             PARTITION OF schedule_instance_run
             FOR VALUES FROM (%L::timestamptz) TO (%L::timestamptz)',
            suffix, cur_date, nxt_date
        );

        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS schedule_defn_run_event_%s
             PARTITION OF schedule_defn_run_event
             FOR VALUES FROM (%L::timestamptz) TO (%L::timestamptz)',
            suffix, cur_date, nxt_date
        );

        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS schedule_outbox_%s
             PARTITION OF schedule_outbox
             FOR VALUES FROM (%L::timestamptz) TO (%L::timestamptz)',
            suffix, cur_date, nxt_date
        );

        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS schedule_inbox_%s
             PARTITION OF schedule_inbox
             FOR VALUES FROM (%L::timestamptz) TO (%L::timestamptz)',
            suffix, cur_date, nxt_date
        );

        cur_date := nxt_date;
    END LOOP;

    RAISE NOTICE 'Partitions created from % to %', start_date, end_date;
END $$;
