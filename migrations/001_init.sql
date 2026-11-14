-- FleetPulse initial schema
-- Plain SQL on purpose so you can read and explain every line.

-- Connected vehicles / IoT devices
CREATE TABLE IF NOT EXISTS devices (
    id            UUID PRIMARY KEY,
    name          TEXT NOT NULL,
    -- We store a HASH of the API key, never the raw key.
    api_key_hash  TEXT NOT NULL UNIQUE,
    status        TEXT NOT NULL CHECK (status IN ('online', 'offline')),
    last_seen_at  TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One row per telemetry event from a device.
-- id is chosen by the CLIENT → used for idempotency (no duplicate rows).
CREATE TABLE IF NOT EXISTS telemetry_events (
    id              UUID PRIMARY KEY,
    device_id       UUID NOT NULL REFERENCES devices(id),
    recorded_at     TIMESTAMPTZ NOT NULL,
    received_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    latitude        DOUBLE PRECISION NOT NULL,
    longitude       DOUBLE PRECISION NOT NULL,
    speed_kmh       DOUBLE PRECISION NOT NULL,
    battery_pct     DOUBLE PRECISION NOT NULL,
    engine_temp_c   DOUBLE PRECISION NOT NULL
);

-- Fast "show me recent readings for this truck" queries
CREATE INDEX IF NOT EXISTS idx_telemetry_device_recorded
    ON telemetry_events (device_id, recorded_at DESC);

-- Simple alerts when readings look abnormal
CREATE TABLE IF NOT EXISTS alerts (
    id           UUID PRIMARY KEY,
    device_id    UUID NOT NULL REFERENCES devices(id),
    kind         TEXT NOT NULL,
    message      TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    telemetry_id UUID REFERENCES telemetry_events(id)
);

CREATE INDEX IF NOT EXISTS idx_alerts_device_created
    ON alerts (device_id, created_at DESC);
