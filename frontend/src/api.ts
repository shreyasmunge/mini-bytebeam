import {
  getDemoDevices,
  getDemoSnapshot,
  getDemoTelemetry,
  getDemoThresholds,
  isBrowserDemoMode,
  setDemoThresholds,
} from "./demoFleet";

/** Local: Vite proxies /api → Rust. Vercel: set VITE_API_BASE or use browser demo. */
const API_BASE = (import.meta.env.VITE_API_BASE as string | undefined)?.trim() || "/api";

export type FleetVehicle = {
  id: string;
  name: string;
  status: string;
  last_seen_at: string | null;
  latitude: number | null;
  longitude: number | null;
  speed_kmh: number | null;
  battery_pct: number | null;
  engine_temp_c: number | null;
  recorded_at: string | null;
  in_red_zone: boolean;
};

export type Device = {
  id: string;
  name: string;
  status: string;
  last_seen_at: string | null;
  created_at: string;
};

export type Thresholds = {
  engine_temp_alert_c: number;
  battery_low_pct: number;
  offline_after_secs: number;
  updated_at: string | null;
};

export type TelemetryPoint = {
  id: string;
  recorded_at: string;
  latitude: number;
  longitude: number;
  speed_kmh: number;
  battery_pct: number;
  engine_temp_c: number;
};

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`);
  const text = await res.text();
  // Vercel SPA fallback returns HTML for /api/* when no backend exists.
  if (!res.ok || text.trimStart().startsWith("<!")) {
    throw new Error(
      text.trimStart().startsWith("<!")
        ? "API not configured on this host"
        : text || res.statusText,
    );
  }
  return JSON.parse(text) as T;
}

export async function fetchFleetSnapshot() {
  if (isBrowserDemoMode()) return getDemoSnapshot();
  try {
    return await getJson<FleetVehicle[]>("/fleet/snapshot");
  } catch {
    return getDemoSnapshot();
  }
}

export async function fetchDevices(params?: { status?: string; q?: string }) {
  if (isBrowserDemoMode()) return getDemoDevices(params);
  try {
    const sp = new URLSearchParams();
    if (params?.status && params.status !== "all") sp.set("status", params.status);
    if (params?.q) sp.set("q", params.q);
    const qs = sp.toString();
    return await getJson<Device[]>(`/devices${qs ? `?${qs}` : ""}`);
  } catch {
    return getDemoDevices(params);
  }
}

export async function fetchThresholds() {
  if (isBrowserDemoMode()) return getDemoThresholds();
  try {
    return await getJson<Thresholds>("/settings/thresholds");
  } catch {
    return getDemoThresholds();
  }
}

export async function saveThresholds(body: Omit<Thresholds, "updated_at">) {
  if (isBrowserDemoMode()) return setDemoThresholds(body);
  try {
    const res = await fetch(`${API_BASE}/settings/thresholds`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<Thresholds>;
  } catch {
    return setDemoThresholds(body);
  }
}

export async function fetchDeviceTelemetry(id: string, limit = 40) {
  if (isBrowserDemoMode()) return getDemoTelemetry(id, limit);
  try {
    return await getJson<TelemetryPoint[]>(`/devices/${id}/telemetry?limit=${limit}`);
  } catch {
    return getDemoTelemetry(id, limit);
  }
}

export type FleetAlert = {
  id: string;
  device_id: string;
  kind: string;
  message: string;
  created_at: string;
  telemetry_id: string | null;
  acknowledged_at: string | null;
  device_name: string | null;
};

export type IngestActivity = {
  at: string;
  device_id: string;
  device_name: string;
  path: string;
  outcome: string;
  events: number;
  alerts: string[];
  detail: string;
};

export type EndpointExample = {
  method: string;
  path: string;
  headers: string[];
  body_example: unknown;
  notes: string;
};

export type PipelineStatus = {
  metrics: {
    ingested_total: number;
    duplicates_total: number;
    batch_flushes_total: number;
    batch_events_total: number;
    alerts_created_total: number;
    auth_cache_hits_total: number;
    auth_cache_misses_total: number;
    ingest_errors_total: number;
    uptime_secs: number;
    approx_ingest_per_sec: number;
  };
  devices_online: number;
  devices_offline: number;
  open_alerts: number;
  twin_rows: number;
  recent: IngestActivity[];
  try_api: {
    base_url: string;
    register: EndpointExample;
    telemetry: EndpointExample;
    batch: EndpointExample;
    pipeline: EndpointExample;
    last_local_benchmark: string;
  };
};

export async function fetchAlerts(status: "open" | "all" = "open") {
  if (isBrowserDemoMode()) {
    const { getDemoAlerts } = await import("./demoFleet");
    return getDemoAlerts(status);
  }
  try {
    return await getJson<FleetAlert[]>(`/alerts?status=${status}`);
  } catch {
    const { getDemoAlerts } = await import("./demoFleet");
    return getDemoAlerts(status);
  }
}

export async function ackAlert(id: string) {
  if (isBrowserDemoMode()) {
    const { ackDemoAlert } = await import("./demoFleet");
    return ackDemoAlert(id);
  }
  try {
    const res = await fetch(`${API_BASE}/alerts/${id}/ack`, { method: "POST" });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  } catch {
    const { ackDemoAlert } = await import("./demoFleet");
    return ackDemoAlert(id);
  }
}

export async function fetchPipeline() {
  if (isBrowserDemoMode()) {
    const { getDemoPipeline } = await import("./demoFleet");
    return getDemoPipeline();
  }
  try {
    return await getJson<PipelineStatus>("/metrics/pipeline");
  } catch {
    const { getDemoPipeline } = await import("./demoFleet");
    return getDemoPipeline();
  }
}

export async function registerDevice(name: string) {
  if (isBrowserDemoMode()) {
    return {
      id: crypto.randomUUID(),
      name,
      api_key: `fp_demo_${crypto.randomUUID().slice(0, 8)}`,
      status: "offline",
    };
  }
  const res = await fetch(`${API_BASE}/devices`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<{
    id: string;
    name: string;
    api_key: string;
    status: string;
  }>;
}

export async function sendTelemetry(
  apiKey: string,
  body: {
    id: string;
    recorded_at: string;
    latitude: number;
    longitude: number;
    speed_kmh: number;
    battery_pct: number;
    engine_temp_c: number;
  },
) {
  if (isBrowserDemoMode()) {
    const { recordDemoIngest } = await import("./demoFleet");
    return recordDemoIngest(body);
  }
  const res = await fetch(`${API_BASE}/telemetry`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify(body),
  });
  const text = await res.text();
  if (!res.ok) throw new Error(text || res.statusText);
  return JSON.parse(text) as { status: string; alerts_created: string[] };
}

/** Public load-test endpoint — no API key. Up to 5000 events. */
export async function playgroundIngest(body: {
  count?: number;
  vehicles?: number;
  events?: unknown[];
}) {
  if (isBrowserDemoMode()) {
    return {
      accepted: body.count ?? body.events?.length ?? 0,
      duplicates: 0,
      vehicles_used: body.vehicles ?? 50,
      endpoint: "POST /playground/ingest",
      hint: "Demo mode — start local API for a real blast.",
    };
  }
  const res = await fetch(`${API_BASE}/playground/ingest`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const text = await res.text();
  if (!res.ok) throw new Error(text || res.statusText);
  return JSON.parse(text) as {
    accepted: number;
    duplicates: number;
    vehicles_used: number;
    endpoint: string;
    hint: string;
  };
}

export { isBrowserDemoMode };
