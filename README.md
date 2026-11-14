# FleetPulse (`mini-bytebeam`)

Rust IoT telemetry backend + React map UI (Bytebeam-style portfolio).

**Industry path shipped:** device twin → alerts+ack → batch ingest → Timescale-ready storage + retention → MQTT bridge → auth cache/rotate → pipeline metrics. Designed to load-test **thousands of vehicles** (target ~1k–5k msg/s on a laptop).

## Quick start

```bash
colima start   # if needed

# Fresh Timescale volume recommended if you previously used plain postgres:16
docker compose down
docker compose up -d db mosquitto

export DATABASE_URL=postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse
cargo run
```

API: http://127.0.0.1:3000

### Frontend

```bash
cd frontend && npm install && npm run dev
```

Open http://127.0.0.1:5173/ — Map, Analytics, **Alerts**, **Pipeline**, Red zone.

## Scale features (interview talking points)

| Feature | What |
|---------|------|
| **Device twin** | `device_state` — map reads O(devices), not history |
| **Alerts + ack** | `GET /alerts`, `POST /alerts/:id/ack`, offline alerts from sweeper |
| **Batch ingest** | `POST /telemetry/batch` (up to 2000) + idempotent `ON CONFLICT (id)` |
| **Auth cache** | hashed API key → device in memory (TTL) |
| **Key rotate** | `POST /devices/:id/rotate-key` |
| **Timescale** | docker image + hypertable when extension present; retention rollup |
| **MQTT** | Mosquitto + `mqtt_bridge` → same HTTP ingest path |
| **Metrics** | `GET /metrics`, `GET /metrics/pipeline` |

## Load test (thousands of vehicles)

```bash
# API running first
# ~1000 vehicles @ 1 Hz
SIM_DEVICES=1000 SIM_INTERVAL_MS=1000 SIM_DURATION_SECS=60 \
  cargo run --release --bin load_fleet

# Higher throughput via batches
SIM_DEVICES=500 BATCH_SIZE=10 SIM_INTERVAL_MS=1000 SIM_DURATION_SECS=60 \
  cargo run --release --bin load_fleet

# k6
VUS=200 BATCH_SIZE=50 k6 run benchmarks/telemetry.js

# Watch live counters
curl -s http://127.0.0.1:3000/metrics/pipeline | jq
```

## MQTT path

```bash
docker compose up -d mosquitto
cargo run --bin mqtt_bridge
# or: docker compose --profile mqtt up -d mqtt_bridge

# Publish (payload needs api_key + telemetry fields):
# topic: fleet/{device_id}/telemetry
```

## API smoke

```bash
curl -s -X POST http://127.0.0.1:3000/devices \
  -H 'content-type: application/json' \
  -d '{"name":"truck-1"}' | jq

# Batch
curl -s -X POST http://127.0.0.1:3000/telemetry/batch \
  -H "Authorization: Bearer KEY" \
  -H 'content-type: application/json' \
  -d '{"events":[{"id":"'"$(uuidgen | tr A-Z a-z)"'","recorded_at":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'","latitude":12.97,"longitude":77.59,"speed_kmh":42,"battery_pct":78,"engine_temp_c":91}]}'
```

## Tests

```bash
docker compose up -d db
export DATABASE_URL=postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse
cargo test
```

## Docs

- `REVISION_AND_DESIGN.md` — decisions / HLD / LLD  
- `CODE_WALKTHROUGH.md` — file walkthrough  
- `BYTEBEAM_PRODUCT_LENS.md` — product lens  

## Defaults

| What | Value |
|------|--------|
| Postgres | `fleetpulse` / `fleetpulse` @ localhost:**5433** |
| API | `:3000` |
| MQTT | `:1883` |
| Retain raw telemetry | 14 days (`TELEMETRY_RETAIN_DAYS`) |
