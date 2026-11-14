# FleetPulse

FleetPulse is a small IoT fleet telemetry platform. Vehicles register, send GPS and sensor readings over HTTP (or MQTT), and an ops dashboard shows live map status, alerts, analytics, and pipeline health. The goal is a clear path from “one truck demo” to something that still holds up when many devices talk at once.

## What we built

The API is Rust (Axum + Tokio + SQLx) on Postgres. Devices authenticate with API keys. Ingest is idempotent by event UUID so retries do not double-count. A device twin table keeps the latest known state so the map can refresh in O(devices) instead of scanning full history. Alerts fire for high engine temp, low battery, and offline devices, and operators can acknowledge them. Batch ingest and an optional MQTT bridge share the same validation and twin update path. Pipeline metrics and a capped public playground endpoint make it easy to show live traffic without exposing private keys. A React UI covers the map, alerts, analytics, thresholds, and pipeline controls. The stack can run locally with Docker Compose, or hosted (API + Postgres + static UI) for a shareable demo.

## Architecture choices (and why)

**Rust + Axum** gives a fast async server with predictable latency under concurrent device traffic, without a heavy framework. **Postgres** is the system of record; we use a Timescale-compatible schema so hypertables and retention can turn on when the extension is present, but plain Postgres still works for simple deploys. **Device twin (`device_state`)** separates “what the map needs now” from “what we store for history,” which keeps live reads cheap as event volume grows. **Idempotent inserts and batch endpoints** cut round-trips and make load tests and flaky networks safer. **In-memory auth cache** avoids hashing and looking up the same API key on every message. **Offline sweeper** is a background job rather than trusting the client to report disconnects. **MQTT as a thin bridge into HTTP ingest** keeps one business path for validation, alerts, and twin updates. The **hosted playground** is intentionally capped and runs a small live scenario (mostly healthy vehicles, with occasional overheat and dropouts) so the public demo looks like an ops console without melting a free-tier host.

## Live demo

- UI: https://mini-bytebeam.vercel.app  
- API health: https://fleetpulse-api-99xd.onrender.com/health  

Free hosting may sleep; the first request can take a short warm-up.

## Quick start (local)

```bash
docker compose up -d db mosquitto

export DATABASE_URL=postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse
cargo run
```

API: http://127.0.0.1:3000

```bash
cd frontend && npm install && npm run dev
```

UI: http://127.0.0.1:5173/

## Load test

```bash
# API running first — ~1000 vehicles @ 1 Hz
SIM_DEVICES=1000 SIM_INTERVAL_MS=1000 SIM_DURATION_SECS=60 \
  cargo run --release --bin load_fleet

# Higher throughput via batches
SIM_DEVICES=500 BATCH_SIZE=10 SIM_INTERVAL_MS=1000 SIM_DURATION_SECS=60 \
  cargo run --release --bin load_fleet

VUS=200 BATCH_SIZE=50 k6 run benchmarks/telemetry.js
curl -s http://127.0.0.1:3000/metrics/pipeline | jq
```

## MQTT

```bash
docker compose up -d mosquitto
cargo run --bin mqtt_bridge
# topic: fleet/{device_id}/telemetry
```

## Tests

```bash
docker compose up -d db
export DATABASE_URL=postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse
cargo test
```

## More detail

See `REVISION_AND_DESIGN.md` for design notes and `CODE_WALKTHROUGH.md` for a file-by-file tour.

## Defaults

| What | Value |
|------|--------|
| Postgres | `fleetpulse` / `fleetpulse` @ localhost:**5433** |
| API | `:3000` |
| MQTT | `:1883` |
| Retain raw telemetry | 14 days (`TELEMETRY_RETAIN_DAYS`) |
