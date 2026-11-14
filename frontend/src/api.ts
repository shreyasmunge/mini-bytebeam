const API_BASE = import.meta.env.VITE_API_BASE ?? "/api";

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
  if (!res.ok) {
    const text = await res.text();
    throw new Error(text || res.statusText);
  }
  return res.json() as Promise<T>;
}

export function fetchFleetSnapshot() {
  return getJson<FleetVehicle[]>("/fleet/snapshot");
}

export function fetchDevices(params?: { status?: string; q?: string }) {
  const sp = new URLSearchParams();
  if (params?.status && params.status !== "all") sp.set("status", params.status);
  if (params?.q) sp.set("q", params.q);
  const qs = sp.toString();
  return getJson<Device[]>(`/devices${qs ? `?${qs}` : ""}`);
}

export function fetchThresholds() {
  return getJson<Thresholds>("/settings/thresholds");
}

export async function saveThresholds(body: Omit<Thresholds, "updated_at">) {
  const res = await fetch(`${API_BASE}/settings/thresholds`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<Thresholds>;
}

export function fetchDeviceTelemetry(id: string, limit = 40) {
  return getJson<TelemetryPoint[]>(`/devices/${id}/telemetry?limit=${limit}`);
}
