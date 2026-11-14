# Code walkthrough (simple words)

Read this file when revising for interviews. Every important file is explained.

---

## How a request travels

```text
HTTP request
  → Axum router (lib.rs)
  → maybe AuthDevice extractor (auth.rs)   // reads Bearer key
  → handler (devices / telemetry / health)
  → SQLx query → PostgreSQL
  → JSON response (or AppError → status code)
```

---

## File by file

### `Cargo.toml`
Lists dependencies. Important ones:

| Crate | Job |
|-------|-----|
| `axum` | HTTP routes |
| `tokio` | async runtime |
| `sqlx` | talk to Postgres |
| `serde` | JSON ↔ Rust structs |
| `sha2` + `hex` | hash API keys |
| `uuid` / `chrono` | ids and timestamps |
| `reqwest` | used by simulator + tests |

Two binaries:

- `mini-bytebeam` → the API (`src/main.rs`)
- `simulator` → fake cars (`src/bin/simulator.rs`)

### `src/main.rs`
Starts the program:

1. Turn on logs  
2. Load `Config` from env  
3. Connect DB + migrate  
4. Spawn offline sweeper (background)  
5. Bind TCP and serve Axum  

### `src/lib.rs`
Shared library used by the API binary and by tests.

- `AppState` = `{ pool, config }` cloned into every handler  
- `app_router` wires `/health`, `/devices`, `/telemetry`  
- `build_state` connects + migrates  

### `src/config.rs`
Reads environment variables with defaults.  
Example: `DATABASE_URL`, `API_PORT`, alert thresholds.

### `src/error.rs`
`AppError` variants → HTTP status + `{"error":"..."}`.

| Variant | HTTP |
|---------|------|
| Unauthorized | 401 |
| BadRequest | 400 |
| NotFound | 404 |
| Database | 503 |

`From<sqlx::Error>` lets handlers use `?` on DB calls.

### `src/db.rs`
- `connect` → connection pool (reuse connections)  
- `migrate` → run `migrations/*.sql` once  

### `migrations/001_init.sql`
Creates three tables:

1. `devices` — vehicles + hashed API key + online/offline  
2. `telemetry_events` — readings; **`id` is primary key** (idempotency)  
3. `alerts` — abnormal readings  

Indexes speed up “events for this device” queries.

### `src/auth.rs`
- `hash_api_key` — SHA-256 → hex (we store hash, not raw key)  
- `generate_api_key` — `fp_live_<uuid>`  
- `AuthDevice` — Axum extractor: parse `Authorization: Bearer …`, hash, lookup device  

If lookup fails → 401.

### `src/devices.rs`
- `POST /devices` — create device, return plaintext key **once**  
- `GET /devices/{id}` — only the owning device (same API key)  

### `src/telemetry.rs` (most important for interviews)
`POST /telemetry`:

1. Auth  
2. Validate ranges (lat/lon/battery/speed)  
3. `INSERT` event  
4. If unique violation on `id` → return `200 duplicate_ignored` (idempotent)  
5. Else mark device online, maybe insert alerts (hot engine / low battery)  
6. Return `201 created`  

**Explain this out loud:** “The database primary key is the idempotency key. Concurrent retries cannot create two rows.”

### `src/health.rs`
- `GET /health` — process alive  
- `GET /ready` — `SELECT 1` against DB  
- `GET /devices/{id}/alerts` — list alerts  
- `offline_sweeper` — every N seconds, mark devices offline if `last_seen_at` too old  

### `src/bin/simulator.rs`
Registers N devices, then loops posting telemetry concurrently with Tokio tasks.

### `tests/api_tests.rs`
Starts a real server on a random port against real Postgres. Covers:

- health / ready  
- missing auth → 401  
- bad battery → 400  
- create + duplicate event  
- high temp → alert  

### `docker-compose.yml`
- `db` = Postgres 16, host port **5433**  
- `api` = build Dockerfile, connects to `db` hostname  

Why 5433? This Mac already had Homebrew Postgres on 5432.

### `Dockerfile`
Multi-stage: compile in `rust` image, copy binary to slim Debian.

### `benchmarks/telemetry.js`
k6 script: register (or use `API_KEY`), then many VUs POST telemetry.

---

## Design choices you should be able to defend

1. **Hash API keys** — DB leak ≠ full credential leak.  
2. **Client event UUID as PK** — simplest correct idempotency.  
3. **Background sweeper** — offline needs time, not only HTTP.  
4. **One binary, not microservices** — scope fits a week-long portfolio piece.  
5. **Colima instead of Docker Desktop** — Desktop install needed sudo; Colima works with same Compose files.  

---

## Frontend (`frontend/`)

Bytebeam-aligned stack: **React + TypeScript + Vite + Tailwind + Leaflet**.

| Page | Route | What it does |
|------|-------|----------------|
| Map | `/` | Vintage full-bleed map, dots, hover health, polls `/fleet/snapshot` |
| Analytics | `/analytics` | Filter devices + simple charts |
| Red zone | `/thresholds` | Form → `PUT /settings/thresholds` |

New backend pieces for the UI:

- `GET /fleet/snapshot`
- `GET /devices` (list + filters)
- `GET /devices/:id/telemetry`
- `GET/PUT /settings/thresholds`
- CORS + Vite `/api` proxy

## Commands cheat sheet

```bash
colima start
docker compose up -d db
export DATABASE_URL=postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse
cargo run
# other terminal:
cd frontend && npm run dev
# open http://127.0.0.1:5173/

cargo test
cargo run --bin simulator
k6 run benchmarks/telemetry.js
```
