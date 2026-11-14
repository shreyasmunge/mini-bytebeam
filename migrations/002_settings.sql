-- Single-row settings for red-zone thresholds (editable from the UI).
CREATE TABLE IF NOT EXISTS settings (
    id                   INT PRIMARY KEY CHECK (id = 1),
    engine_temp_alert_c  DOUBLE PRECISION NOT NULL,
    battery_low_pct      DOUBLE PRECISION NOT NULL,
    offline_after_secs   BIGINT NOT NULL,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO settings (id, engine_temp_alert_c, battery_low_pct, offline_after_secs)
VALUES (1, 110, 15, 60)
ON CONFLICT (id) DO NOTHING;
