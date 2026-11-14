-- Phase 4: Timescale hypertable when extension is available; otherwise plain Postgres.
-- Timescale requires the primary key to include the partition column (recorded_at).
-- Idempotency stays via UNIQUE(id).

CREATE UNIQUE INDEX IF NOT EXISTS telemetry_events_id_uidx ON telemetry_events (id);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'telemetry_events_pkey'
          AND conrelid = 'telemetry_events'::regclass
    ) THEN
        ALTER TABLE telemetry_events DROP CONSTRAINT telemetry_events_pkey;
        ALTER TABLE telemetry_events ADD PRIMARY KEY (id, recorded_at);
    END IF;
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'telemetry PK adjust skipped: %', SQLERRM;
END $$;

DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'timescaledb not installed — using plain Postgres for telemetry_events';
END $$;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_proc WHERE proname = 'create_hypertable') THEN
        PERFORM create_hypertable(
            'telemetry_events',
            'recorded_at',
            if_not_exists => TRUE,
            migrate_data => TRUE
        );
    END IF;
EXCEPTION
    WHEN OTHERS THEN
        RAISE NOTICE 'create_hypertable skipped: %', SQLERRM;
END $$;

-- Plain fallback rollup table (always available for charts / retention story).
CREATE TABLE IF NOT EXISTS telemetry_1m_plain (
    bucket            TIMESTAMPTZ NOT NULL,
    device_id         UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    avg_speed_kmh     DOUBLE PRECISION NOT NULL,
    avg_battery_pct   DOUBLE PRECISION NOT NULL,
    avg_engine_temp_c DOUBLE PRECISION NOT NULL,
    samples           BIGINT NOT NULL,
    PRIMARY KEY (bucket, device_id)
);
