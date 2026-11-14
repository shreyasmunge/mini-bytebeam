-- Phase 1–3: device twin, alert acknowledge, key rotation metadata.

CREATE TABLE IF NOT EXISTS device_state (
    device_id       UUID PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    latitude        DOUBLE PRECISION NOT NULL,
    longitude       DOUBLE PRECISION NOT NULL,
    speed_kmh       DOUBLE PRECISION NOT NULL,
    battery_pct     DOUBLE PRECISION NOT NULL,
    engine_temp_c   DOUBLE PRECISION NOT NULL,
    recorded_at     TIMESTAMPTZ NOT NULL,
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_device_state_last_seen
    ON device_state (last_seen_at DESC);

ALTER TABLE alerts
    ADD COLUMN IF NOT EXISTS acknowledged_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_alerts_open
    ON alerts (created_at DESC)
    WHERE acknowledged_at IS NULL;

ALTER TABLE devices
    ADD COLUMN IF NOT EXISTS api_key_rotated_at TIMESTAMPTZ;

-- Backfill twin from latest telemetry (one-time for existing fleets).
INSERT INTO device_state (
    device_id, latitude, longitude, speed_kmh, battery_pct, engine_temp_c,
    recorded_at, last_seen_at, updated_at
)
SELECT DISTINCT ON (t.device_id)
    t.device_id,
    t.latitude,
    t.longitude,
    t.speed_kmh,
    t.battery_pct,
    t.engine_temp_c,
    t.recorded_at,
    COALESCE(d.last_seen_at, t.recorded_at),
    now()
FROM telemetry_events t
JOIN devices d ON d.id = t.device_id
ORDER BY t.device_id, t.recorded_at DESC
ON CONFLICT (device_id) DO NOTHING;
