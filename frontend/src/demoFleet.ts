/**
 * Browser-side demo fleet used on Vercel when no Rust API is configured.
 * Mirrors the realistic demo_fleet binary: mostly green, 1 heat + 1 offline cycle.
 */

import type { FleetVehicle, TelemetryPoint, Thresholds } from "./api";

type Seed = {
  id: string;
  name: string;
  lat: number;
  lon: number;
  role: "healthy" | "heat" | "drop";
};

const SEEDS: Seed[] = [
  { id: "d1", name: "demo-airport-shuttle", lat: 13.1989, lon: 77.7068, role: "healthy" },
  { id: "d2", name: "demo-whitefield-van", lat: 12.9698, lon: 77.75, role: "healthy" },
  { id: "d3", name: "demo-ecity-truck", lat: 12.8452, lon: 77.6602, role: "heat" },
  { id: "d4", name: "demo-koramangala-cab", lat: 12.9352, lon: 77.6245, role: "healthy" },
  { id: "d5", name: "demo-yelahanka-bus", lat: 13.1007, lon: 77.5963, role: "healthy" },
  { id: "d6", name: "demo-mysore-road-lorry", lat: 12.951, lon: 77.52, role: "drop" },
  { id: "d7", name: "demo-indiranagar-scooter", lat: 12.9784, lon: 77.6408, role: "healthy" },
  { id: "d8", name: "demo-hebbal-ev", lat: 13.0358, lon: 77.597, role: "healthy" },
  { id: "d9", name: "demo-jayanagar-auto", lat: 12.9308, lon: 77.5838, role: "healthy" },
  { id: "d10", name: "demo-marathahalli-van", lat: 12.9591, lon: 77.6974, role: "healthy" },
];

const startedAt = Date.now();
let thresholds: Thresholds = {
  engine_temp_alert_c: 110,
  battery_low_pct: 15,
  offline_after_secs: 20,
  updated_at: new Date().toISOString(),
};

const history = new Map<string, TelemetryPoint[]>();

export function isBrowserDemoMode() {
  // Local Vite uses /api proxy → real backend.
  // Vercel has no backend unless VITE_API_BASE is set at build time.
  const base = import.meta.env.VITE_API_BASE as string | undefined;
  if (base && base.trim()) return false;
  return import.meta.env.PROD;
}

export function getDemoThresholds(): Thresholds {
  return { ...thresholds };
}

export function setDemoThresholds(
  body: Omit<Thresholds, "updated_at">,
): Thresholds {
  thresholds = { ...body, updated_at: new Date().toISOString() };
  return getDemoThresholds();
}

export function getDemoSnapshot(): FleetVehicle[] {
  const t = (Date.now() - startedAt) / 1000;
  const now = new Date().toISOString();

  // Heat: red for 35s every 105s, starting at t=25
  const heatCycle = (t + 80) % 105;
  const heating = heatCycle < 35;

  // Drop: offline for 45s every 125s, starting at t=35
  const dropCycle = (t + 90) % 125;
  const dropped = dropCycle < 45;

  return SEEDS.map((s, i) => {
    const drift = t * 0.05 + i;
    let lat = s.lat + Math.sin(drift) * 0.005;
    let lon = s.lon + Math.cos(drift) * 0.005;
    let speed = 22 + ((Math.floor(t) + i) % 28);
    let battery = 62 + ((Math.floor(t) + i) % 20) * 0.8;
    let temp = 84 + ((Math.floor(t) + i) % 12) * 0.7;
    let status = "online";
    let lastSeen = now;

    if (s.role === "heat" && heating) {
      temp = 116 + (Math.floor(t) % 3);
    }
    if (s.role === "drop" && dropped) {
      status = "offline";
      lastSeen = new Date(Date.now() - 60_000).toISOString();
      // freeze last position slightly
      lat = s.lat;
      lon = s.lon;
    }

    const in_red_zone =
      status === "offline" ||
      battery < thresholds.battery_low_pct ||
      temp > thresholds.engine_temp_alert_c;

    const point: TelemetryPoint = {
      id: `${s.id}-${Math.floor(t)}`,
      recorded_at: now,
      latitude: lat,
      longitude: lon,
      speed_kmh: speed,
      battery_pct: battery,
      engine_temp_c: temp,
    };
    const prev = history.get(s.id) ?? [];
    if (status === "online") {
      history.set(s.id, [...prev, point].slice(-40));
    }

    return {
      id: s.id,
      name: s.name,
      status,
      last_seen_at: lastSeen,
      latitude: lat,
      longitude: lon,
      speed_kmh: speed,
      battery_pct: battery,
      engine_temp_c: temp,
      recorded_at: now,
      in_red_zone,
    };
  });
}

export function getDemoDevices(params?: { status?: string; q?: string }) {
  let rows = getDemoSnapshot().map((v) => ({
    id: v.id,
    name: v.name,
    status: v.status,
    last_seen_at: v.last_seen_at,
    created_at: new Date(startedAt).toISOString(),
  }));
  if (params?.status && params.status !== "all") {
    rows = rows.filter((r) => r.status === params.status);
  }
  if (params?.q) {
    const q = params.q.toLowerCase();
    rows = rows.filter((r) => r.name.toLowerCase().includes(q));
  }
  return rows;
}

export function getDemoTelemetry(id: string, limit = 40): TelemetryPoint[] {
  return (history.get(id) ?? []).slice(-limit);
}

type DemoAlert = {
  id: string;
  device_id: string;
  kind: string;
  message: string;
  created_at: string;
  telemetry_id: string | null;
  acknowledged_at: string | null;
  device_name: string | null;
};

const demoAlerts: DemoAlert[] = [
  {
    id: "a-heat",
    device_id: "d3",
    kind: "high_engine_temp",
    message: "engine_temp_c above threshold (demo)",
    created_at: new Date(startedAt + 5_000).toISOString(),
    telemetry_id: null,
    acknowledged_at: null,
    device_name: "demo-ecity-truck",
  },
  {
    id: "a-off",
    device_id: "d6",
    kind: "offline",
    message: "device stopped sending telemetry (demo)",
    created_at: new Date(startedAt + 12_000).toISOString(),
    telemetry_id: null,
    acknowledged_at: null,
    device_name: "demo-mysore-road-lorry",
  },
];

export function getDemoAlerts(status: "open" | "all" = "open") {
  if (status === "all") return [...demoAlerts];
  return demoAlerts.filter((a) => !a.acknowledged_at);
}

export function ackDemoAlert(id: string) {
  const row = demoAlerts.find((a) => a.id === id);
  if (row) row.acknowledged_at = new Date().toISOString();
  return { status: "acknowledged", id };
}

type DemoIngest = {
  at: string;
  device_id: string;
  device_name: string;
  path: string;
  outcome: string;
  events: number;
  alerts: string[];
  detail: string;
};

const demoRecent: DemoIngest[] = [
  {
    at: new Date(startedAt + 8_000).toISOString(),
    device_id: "d3",
    device_name: "demo-ecity-truck",
    path: "/telemetry",
    outcome: "created",
    events: 1,
    alerts: ["high_engine_temp"],
    detail: "lat=12.8452 lon=77.6602 batt=55 temp=118",
  },
  {
    at: new Date(startedAt + 15_000).toISOString(),
    device_id: "d1",
    device_name: "demo-airport-shuttle",
    path: "/telemetry",
    outcome: "created",
    events: 1,
    alerts: [],
    detail: "lat=13.1989 lon=77.7068 batt=80 temp=90",
  },
];

export function recordDemoIngest(body: {
  latitude: number;
  longitude: number;
  battery_pct: number;
  engine_temp_c: number;
}) {
  const alerts: string[] = [];
  if (body.engine_temp_c > thresholds.engine_temp_alert_c) alerts.push("high_engine_temp");
  if (body.battery_pct < thresholds.battery_low_pct) alerts.push("low_battery");
  demoRecent.unshift({
    at: new Date().toISOString(),
    device_id: "try",
    device_name: "try-device",
    path: "/telemetry",
    outcome: "created",
    events: 1,
    alerts,
    detail: `lat=${body.latitude} lon=${body.longitude} batt=${body.battery_pct} temp=${body.engine_temp_c}`,
  });
  if (demoRecent.length > 40) demoRecent.pop();
  return { status: "created", alerts_created: alerts };
}

export function getDemoPipeline() {
  const fleet = getDemoSnapshot();
  const elapsed = Math.max(1, (Date.now() - startedAt) / 1000);
  return {
    metrics: {
      ingested_total: Math.floor(elapsed * 12) + demoRecent.length,
      duplicates_total: 2,
      batch_flushes_total: Math.floor(elapsed / 2),
      batch_events_total: Math.floor(elapsed * 12),
      alerts_created_total: demoAlerts.length,
      auth_cache_hits_total: Math.floor(elapsed * 10),
      auth_cache_misses_total: 10,
      ingest_errors_total: 0,
      uptime_secs: Math.floor(elapsed),
      approx_ingest_per_sec: 12,
    },
    devices_online: fleet.filter((v) => v.status === "online").length,
    devices_offline: fleet.filter((v) => v.status === "offline").length,
    open_alerts: demoAlerts.filter((a) => !a.acknowledged_at).length,
    twin_rows: fleet.length,
    recent: [...demoRecent],
    try_api: {
      base_url: "http://127.0.0.1:3000",
      register: {
        method: "POST",
        path: "/devices",
        headers: ["Content-Type: application/json"],
        body_example: { name: "my-truck-1" },
        notes: "Returns api_key once — save it.",
      },
      telemetry: {
        method: "POST",
        path: "/telemetry",
        headers: ["Content-Type: application/json", "Authorization: Bearer <api_key>"],
        body_example: {},
        notes: "Reuse the same id to test idempotency.",
      },
      batch: {
        method: "POST",
        path: "/telemetry/batch",
        headers: ["Content-Type: application/json", "Authorization: Bearer <api_key>"],
        body_example: {},
        notes: "Up to 2000 events per request.",
      },
      pipeline: {
        method: "GET",
        path: "/metrics/pipeline",
        headers: [],
        body_example: {},
        notes: "Live counters + recent ingest feed.",
      },
      last_local_benchmark:
        "1000 vehicles × 1 Hz, ~60s → 28,962 events, 0 errors, ~445 msg/s (laptop, single-event path)",
    },
  };
}
