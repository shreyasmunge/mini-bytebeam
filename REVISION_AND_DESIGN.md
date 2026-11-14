# FleetPulse (mini-bytebeam) — Revision Notes, Design Decisions, System Design & LLD

> Use this file to revise before interviews. Written in simple words.
> Project folder name stays **mini-bytebeam** (Cargo package: `mini-bytebeam`). Product name in docs: **FleetPulse**.

---

## 0. What we have done so far (timeline)

| Step | What happened | Why it matters |
|------|----------------|----------------|
| Home folder was a Git repo | Cursor warned “too many active changes” because `/Users/shreyasmunge` itself had a `.git` | Git was tracking your whole home directory by mistake |
| Removed `/Users/shreyasmunge/.git` | Stopped treating home as a repo | Safe: no remotes; old tutorial commits only |
| `git init` inside `mini-bytebeam` | Real project got its own Git repo | Correct place for source control |
| Installed Rust via `rustup` | Got `rustc`, `cargo`, `rustup` | Need these to build/run Rust |
| `source "$HOME/.cargo/env"` | Put Cargo on PATH for that shell | Without this, `cargo` looks “not found” |
| `cargo init` + `cargo run` | Created Hello World and ran it | Proves toolchain works |
| Chose FleetPulse scope | Small IoT telemetry backend, not a full Bytebeam clone | Matches hiring skills: Rust, APIs, SQL, async, reliability |
| Keep name `mini-bytebeam` | Do not rename folder/package | Your choice — fine for GitHub too |
| Tried `brew install --cask docker` | Failed: needed Mac admin password for `/usr/local` links | Docker Desktop cask not usable from agent without sudo |
| Installed **Colima + Docker CLI + Compose + k6** via Homebrew | Lightweight Docker engine (no Desktop GUI) | Same `docker` / `docker compose` commands for the project |
| `colima start` + `docker run hello-world` | Engine works | Ready for Postgres + API containers |
| Ports 3000 & 5432 | Free on this machine | Defaults OK |
| Accepted local DB defaults | `fleetpulse` / `fleetpulse` / db `fleetpulse` | No cloud Postgres account |

**Implemented:** Axum API, Postgres migrations, auth, telemetry idempotency, health/alerts/sweeper, Docker Compose, simulator binary, k6 script, integration tests (6/6 pass), live smoke + k6 (20 VUs, p95 ~13ms, 0% fail).

---

## 1. What FleetPulse is (one paragraph)

FleetPulse is a **backend server** that pretends companies have many connected vehicles. Each vehicle (device) registers, gets a secret API key, and sends telemetry (GPS, speed, battery, engine temp). The server stores that data in PostgreSQL, marks devices online/offline, raises simple alerts for weird readings, and is careful about duplicates and errors. No real cars, no frontend, no Kubernetes.

---

## 2. Design decisions (and why)

These are the choices we lock in before building. Change only if there is a strong reason.

### 2.1 Product scope

| Decision | Choice | Why |
|----------|--------|-----|
| Scope | 4 features only: devices, telemetry, health, reliability | Enough for Bytebeam interview signal; finishable in ~1 week |
| No frontend | API + README + curl/k6 only | Frontend is not what they hire for here |
| No microservices | One Rust binary | Simpler ops, clearer code story |
| No cloud IoT (AWS IoT etc.) | Our own HTTP APIs | Shows you can design ingestion yourself |
| Project name | Folder/package: `mini-bytebeam`; product: FleetPulse | Avoid rename churn |

### 2.2 Language & framework

| Decision | Choice | Why |
|----------|--------|-----|
| Language | Rust (stable) | Job requires Rust working knowledge |
| HTTP | Axum | Modern, Tokio-native, common in industry |
| Async runtime | Tokio | Standard for Axum; many concurrent device connections |
| Serialization | Serde + JSON | Devices send JSON over HTTP |
| Errors | `thiserror` / `anyhow` (app layer) + typed API errors | Clear mapping to HTTP status codes |

### 2.3 Database

| Decision | Choice | Why |
|----------|--------|-----|
| DB | PostgreSQL 16 (Docker) | Industry default; Bytebeam-relevant |
| Access | SQLx (async, runtime-checked queries OK for speed of build; prefer compile-time where practical) | Fits Tokio; migrations supported |
| Migrations | SQL files under `migrations/` run at startup or via `sqlx migrate` | Reproducible schema |
| Hosting for local/dev | **Docker Compose Postgres only** — no Neon/Supabase/RDS required | Zero cloud accounts for MVP |
| Credentials | Defaults in `docker-compose.yml` (local only) | Simple clone-and-run |

### 2.4 Auth model

| Decision | Choice | Why |
|----------|--------|-----|
| Auth type | Per-device API key (Bearer token) | Matches “device authenticates to cloud” IoT model |
| Who creates keys | Server generates on `POST /devices` | No external auth provider (no Auth0/JWT SaaS) |
| Admin vs device | Single device-key model for MVP; optional simple admin token via env for listing | Keep auth small |
| Storage of keys | Store **hashed** API key in DB (e.g. SHA-256), return plaintext **once** at registration | Better than storing raw secrets; still interview-explainable |
| No OAuth / user login | Out of scope | Not needed for device telemetry demo |

### 2.5 Telemetry & idempotency

| Decision | Choice | Why |
|----------|--------|-----|
| Idempotency key | Client-supplied event `id` (UUID) as primary key | Duplicate POST → one row |
| Conflict behavior | Second insert with same `id`: treat as success (idempotent) or `200` with “already exists” — **prefer idempotent 200** | Clients can safely retry |
| Time fields | `recorded_at` (device clock) + `received_at` (server clock) | Debugging clock skew |
| Validation | Reject impossible ranges lightly (e.g. battery 0–100) | Shows input validation |

### 2.6 Health & alerts

| Decision | Choice | Why |
|----------|--------|-----|
| Online rule | `last_seen` within N seconds (e.g. 60s) ⇒ online | Simple, explainable |
| Offline detection | Tokio background task every few seconds | Shows async timers, not only request handlers |
| Alerts | Rows in `alerts` when engine temp high or battery low | Demonstrates “diagnose problems” |
| Thresholds | Constants or env vars (`ENGINE_TEMP_ALERT_C`, `BATTERY_LOW_PCT`) | Easy to tweak in demos |

### 2.7 Reliability & testing

| Decision | Choice | Why |
|----------|--------|-----|
| Duplicate events | Unique PK on telemetry `id` | Core hiring signal |
| Concurrent requests | Tokio + Postgres constraints | DB enforces correctness under race |
| DB failures | Map connection/query errors to 503/500 with structured JSON | Production-minded |
| Automated tests | `cargo test` integration tests with test DB or transactional cleanup | Auth, validation, duplicates |
| Load test | k6 script in `benchmarks/` | Latency + throughput numbers for README |
| Simulator | Small binary/script under `simulator/` | Fake N vehicles without real hardware |

### 2.8 Packaging

| Decision | Choice | Why |
|----------|--------|-----|
| Run story | `docker compose up --build` | Clone → run without “install Postgres by hand” |
| Config | Env vars: `DATABASE_URL`, `HOST`, `PORT`, thresholds | 12-factor style |
| Secrets in git | **Never** commit real production secrets; local compose defaults OK | Safe for public GitHub |
| Container engine on this Mac | **Colima** (not Docker Desktop) | Desktop cask needed sudo; Colima gives a real Docker API for compose |

**How to start Docker on this machine (revise this):**

```bash
# If docker commands fail with "Cannot connect to the Docker daemon":
colima start

# Check:
docker --version
docker compose version
```

---

## 3. System design (HLD)

### 3.1 Context diagram

```text
┌─────────────────────┐         HTTPS/HTTP JSON          ┌──────────────────────────┐
│  Vehicle Simulator  │ ───────────────────────────────► │     FleetPulse API        │
│  (or curl / k6)     │   Bearer <device_api_key>        │     (Rust + Axum + Tokio) │
└─────────────────────┘                                  └────────────┬─────────────┘
                                                                      │
                                                                      │ SQL (SQLx)
                                                                      ▼
                                                             ┌─────────────────┐
                                                             │  PostgreSQL     │
                                                             │  devices,       │
                                                             │  telemetry,     │
                                                             │  alerts         │
                                                             └─────────────────┘
```

**Actors**

- **Device / Simulator:** registers, then sends telemetry.
- **Operator (you):** runs Docker, hits health endpoints, reads alerts via API or SQL.
- **Load tester (k6):** many concurrent devices.

### 3.2 Request flows

**A. Register device**

```text
Client                API                         Postgres
  |                    |                             |
  | POST /devices      |                             |
  | { name }           |                             |
  |------------------->| generate uuid + api_key     |
  |                    | hash(api_key)               |
  |                    | INSERT devices --------------->|
  |                    |<------------------------------|
  | 201 { id, api_key }|                             |
  |<-------------------|  (plaintext key only once)  |
```

**B. Ingest telemetry (happy path + duplicate)**

```text
Client                API                         Postgres
  | POST /telemetry    |                             |
  | Authorization + body with event id               |
  |------------------->| auth: lookup hash(key) ------>|
  |                    | validate body               |
  |                    | INSERT telemetry ------------->|
  |                    | UPDATE device last_seen ----->|
  |                    | maybe INSERT alert ---------->|
  | 200/201            |                             |
  |<-------------------|                             |
  | POST same id again |                             |
  |------------------->| INSERT → unique violation   |
  |                    | treat as idempotent success |
  | 200                |                             |
  |<-------------------|                             |
```

**C. Health background loop**

```text
Tokio interval tick
  → SELECT devices WHERE last_seen < now() - interval
  → SET status = offline
```

### 3.3 Deployment view (local)

```text
docker compose
├── service: db     → postgres:16, port 5432, volume for data
└── service: api    → build Dockerfile, depends_on db, PORT 3000
```

Optional on host (not in compose for MVP): `k6`, `cargo test`.

### 3.4 Non-functional requirements (targets)

| Concern | Target for this project |
|---------|-------------------------|
| Correctness | Duplicates never double-insert |
| Latency | Measure with k6; aim for low p95 on local machine (document actuals, don’t fake) |
| Concurrency | 100–500 virtual devices in k6 |
| Operability | One compose command; health endpoint |
| Security (MVP) | Hashed device keys, no key in logs, validation |

---

## 4. Low-level design (LLD)

### 4.1 Suggested module layout

```text
mini-bytebeam/
├── src/
│   ├── main.rs              # boot: config, db pool, migrate, routes, background task
│   ├── config.rs            # read env into Config struct
│   ├── error.rs             # AppError → HTTP status + JSON body
│   ├── db.rs                # PgPool creation
│   ├── auth/
│   │   └── mod.rs           # extract Bearer, resolve device
│   ├── devices/
│   │   ├── mod.rs           # routes
│   │   ├── handlers.rs
│   │   └── models.rs
│   ├── telemetry/
│   │   ├── mod.rs
│   │   ├── handlers.rs
│   │   └── models.rs
│   └── health/
│       ├── mod.rs           # /health, /ready, alert helpers, offline sweeper
│       └── handlers.rs
├── migrations/
│   └── 001_init.sql
├── tests/
│   └── api_tests.rs         # integration tests
├── simulator/
│   └── ...                  # fake fleet
├── benchmarks/
│   └── telemetry.js         # k6
├── Dockerfile
├── docker-compose.yml
├── Cargo.toml
├── README.md
└── REVISION_AND_DESIGN.md   # this file
```

### 4.2 API contract (v1)

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| `GET` | `/health` | none | Process up |
| `GET` | `/ready` | none | DB reachable |
| `POST` | `/devices` | none (MVP) or admin token | Register device |
| `GET` | `/devices/:id` | device key (own id) | Get device |
| `POST` | `/telemetry` | device key | Ingest one event |
| `GET` | `/devices/:id/telemetry` | device key | Recent events (optional but useful) |
| `GET` | `/devices/:id/alerts` | device key | Alerts for device |

**Example register response**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "truck-42",
  "api_key": "fp_live_....",
  "status": "offline"
}
```

**Example telemetry body**

```json
{
  "id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
  "recorded_at": "2026-11-14T04:00:00Z",
  "latitude": 12.9716,
  "longitude": 77.5946,
  "speed_kmh": 42.5,
  "battery_pct": 78.0,
  "engine_temp_c": 91.0
}
```

### 4.3 Database schema (LLD)

```sql
-- devices
id            UUID PRIMARY KEY
name          TEXT NOT NULL
api_key_hash  TEXT NOT NULL UNIQUE
status        TEXT NOT NULL CHECK (status IN ('online', 'offline'))
last_seen_at  TIMESTAMPTZ NULL
created_at    TIMESTAMPTZ NOT NULL DEFAULT now()

-- telemetry_events
id              UUID PRIMARY KEY          -- idempotency key from client
device_id       UUID NOT NULL REFERENCES devices(id)
recorded_at     TIMESTAMPTZ NOT NULL
received_at     TIMESTAMPTZ NOT NULL DEFAULT now()
latitude        DOUBLE PRECISION NOT NULL
longitude       DOUBLE PRECISION NOT NULL
speed_kmh       DOUBLE PRECISION NOT NULL
battery_pct     DOUBLE PRECISION NOT NULL
engine_temp_c   DOUBLE PRECISION NOT NULL

CREATE INDEX idx_telemetry_device_recorded ON telemetry_events(device_id, recorded_at DESC);

-- alerts
id           UUID PRIMARY KEY
device_id    UUID NOT NULL REFERENCES devices(id)
kind         TEXT NOT NULL   -- e.g. high_engine_temp, low_battery
message      TEXT NOT NULL
created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
telemetry_id UUID NULL REFERENCES telemetry_events(id)

CREATE INDEX idx_alerts_device_created ON alerts(device_id, created_at DESC);
```

**Constraints that enforce design**

- `telemetry_events.id` PK ⇒ idempotency under concurrent retries  
- FK `device_id` ⇒ no orphan telemetry  
- `CHECK` on status ⇒ no garbage states  
- Indexes ⇒ list-by-device stays fast  

### 4.4 Key types (conceptual Rust)

```text
Config { database_url, bind_addr, offline_after_secs, engine_temp_alert_c, battery_low_pct }

Device { id, name, status, last_seen_at, created_at }   // never return api_key_hash

NewDeviceResponse { id, name, api_key, status }        // api_key only at creation

TelemetryCreate { id, recorded_at, lat, lon, speed, battery, engine_temp }

AppError::Unauthorized | Validation | Conflict | Db | Internal
  → StatusCode + { "error": "..." }
```

### 4.5 Auth middleware (LLD)

1. Read `Authorization` header.  
2. Expect `Bearer <token>`.  
3. Hash token the same way as registration.  
4. `SELECT` device by `api_key_hash`.  
5. If missing → 401.  
6. Inject `DeviceId` / `Device` into request extensions for handlers.

### 4.6 Telemetry handler algorithm

1. Authenticate device.  
2. Validate JSON + ranges.  
3. `INSERT INTO telemetry_events (...) VALUES (...)`.  
4. If unique violation on `id`: return 200 `{ "status": "duplicate_ignored" }` (idempotent).  
5. Else: update `devices.last_seen_at`, set `status = online`.  
6. If thresholds breached: insert `alerts` row.  
7. Return 201 `{ "status": "created" }`.

### 4.7 Offline sweeper algorithm

Every `SWEEP_INTERVAL_SECS` (e.g. 10):

```text
UPDATE devices
SET status = 'offline'
WHERE status = 'online'
  AND last_seen_at < now() - make_interval(secs => OFFLINE_AFTER_SECS)
```

### 4.8 Error → HTTP mapping

| Situation | HTTP |
|-----------|------|
| Missing/bad API key | 401 |
| Body invalid | 400 |
| Device not found (GET) | 404 |
| DB down | 503 |
| Unexpected bug | 500 |
| Duplicate telemetry | 200 (idempotent) |

---

## 5. What you need BEFORE we build (checklist)

### You do NOT need (common confusion)

- ❌ Neon / Supabase / AWS RDS account  
- ❌ Bytebeam API key  
- ❌ Auth0 / Firebase / JWT SaaS  
- ❌ Redis (optional later; not in MVP)  
- ❌ Kubernetes / cloud VM  
- ❌ Real vehicle hardware  
- ❌ Frontend hosting  

**Postgres user/password:** we create them locally in Docker Compose (e.g. user `fleetpulse`, password `fleetpulse`, db `fleetpulse`). You do not create a cloud Postgres account unless you *want* to later.

**Device API keys:** the **app generates** them when a device registers. You do not buy or paste device keys from anywhere.

### You DO need (install / confirm) — **setup completed**

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Rust (`rustc`, `cargo`) | ✅ | 1.100.0 — use `source "$HOME/.cargo/env"` in new shells if needed |
| 2 | Git repo in project | ✅ | `mini-bytebeam/.git` |
| 3 | Docker engine | ✅ | **Colima** + Docker CLI 29.9.3 (Docker Desktop skipped — needed sudo) |
| 4 | Docker Compose | ✅ | 5.7.1 — plugin path set in `~/.docker/config.json` |
| 5 | k6 | ✅ | v2.4.0 via Homebrew |
| 6 | Ports 3000 & 5432 | ✅ | Free |
| 7 | Local secrets defaults | ✅ | Accepted (section 6) |
| 8 | Smoke test | ✅ | `docker run hello-world` succeeded |

### Optional later (not blocking build)

| Item | Why |
|------|-----|
| GitHub account + new empty repo | Publish FleetPulse for the application |
| Public README screenshots / k6 output paste | Stronger application packet |
| Docker Desktop (GUI) | Only if you prefer it over Colima; needs your Mac password for `brew install --cask docker` |

---

## 6. Default local secrets we will use (unless you override)

You can accept these as-is. Only change if port 5432 is already taken or you prefer different passwords.

```text
POSTGRES_USER=fleetpulse
POSTGRES_PASSWORD=fleetpulse
POSTGRES_DB=fleetpulse
DATABASE_URL=postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse
API_HOST=0.0.0.0
API_PORT=3000
OFFLINE_AFTER_SECS=60
ENGINE_TEMP_ALERT_C=110
BATTERY_LOW_PCT=15
```

**Port decision:** Host **5433** → container 5432 because Homebrew Postgres already listened on 5432 (password auth failures were hitting the wrong server).

---

## 7. Interview talking points (from this design)

1. **Why Axum + Tokio:** async I/O for many devices posting at once.  
2. **Why Postgres constraints for idempotency:** correctness under concurrency without distributed locks.  
3. **Why hash API keys:** stolen DB ≠ stolen plaintext device credentials.  
4. **Why `recorded_at` vs `received_at`:** devices have bad clocks; server time is source of “when we saw it.”  
5. **Why background sweeper:** presence is time-based; HTTP alone cannot mark offline.  
6. **What you’d add next (honest):** MQTT, exactly-once pipelines, per-tenant isolation, observability (tracing/metrics), OTA — but MVP stays small on purpose.

---

## 8. Implementation status

**MVP built and verified.** See also `CODE_WALKTHROUGH.md` and `README.md`.

| Check | Result |
|-------|--------|
| `cargo build` | OK |
| `cargo test` (6 tests) | OK |
| Live register + telemetry + duplicate | OK |
| Alerts (high temp + low battery) | OK |
| k6 20 VUs / 10s | ~184 req/s, p95 ≈ 13ms, 0% fail |

---

## 9. Setup session log (what we installed and why)

| Tool | Version / how | Simple meaning |
|------|----------------|----------------|
| Rust | rustc/cargo 1.100.0 via rustup | Compiles and runs the API |
| Colima | 0.10.3 | Tiny Linux VM that runs Docker containers on Mac |
| Docker CLI | 29.9.3 | The `docker` command you type |
| Docker Compose | 5.7.1 | Starts API + Postgres together from a YAML file |
| k6 | 2.4.0 | Load-tests many fake devices at once |
| Postgres (later) | via compose image | Will hold devices, telemetry, alerts |

**Design decision revisited:** We do **not** require Docker Desktop. Colima is enough for interview demos and `docker compose up`. If someone clones on another machine with Docker Desktop, the same compose files still work.

*Last updated: MVP implemented and tested.*
