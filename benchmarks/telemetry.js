/**
 * k6 load test — many virtual devices posting telemetry.
 *
 * Setup once (register one device, paste api key + use random event ids):
 *   curl -s -X POST http://127.0.0.1:3000/devices \
 *     -H 'content-type: application/json' \
 *     -d '{"name":"k6-device"}'
 *
 * Run:
 *   export API_KEY='fp_live_...'
 *   k6 run benchmarks/telemetry.js
 *
 * Or with defaults that register in setup():
 *   k6 run benchmarks/telemetry.js
 */

import http from "k6/http";
import { check, sleep } from "k6";
import { uuidv4 } from "https://jslib.k6.io/k6-utils/1.4.0/index.js";

export const options = {
  vus: 50, // virtual users ≈ concurrent devices
  duration: "30s",
  thresholds: {
    http_req_failed: ["rate<0.05"],
    http_req_duration: ["p(95)<500"],
  },
};

const BASE = __ENV.API_BASE || "http://127.0.0.1:3000";

export function setup() {
  // One shared device for the run (simple). For true multi-device, register per VU.
  if (__ENV.API_KEY) {
    return { apiKey: __ENV.API_KEY };
  }
  const res = http.post(
    `${BASE}/devices`,
    JSON.stringify({ name: `k6-${Date.now()}` }),
    { headers: { "Content-Type": "application/json" } }
  );
  check(res, { "register 201": (r) => r.status === 201 });
  const body = res.json();
  return { apiKey: body.api_key };
}

export default function (data) {
  const payload = JSON.stringify({
    id: uuidv4(),
    recorded_at: new Date().toISOString(),
    latitude: 12.97,
    longitude: 77.59,
    speed_kmh: 40 + Math.random() * 20,
    battery_pct: 50 + Math.random() * 40,
    engine_temp_c: 80 + Math.random() * 20,
  });

  const res = http.post(`${BASE}/telemetry`, payload, {
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${data.apiKey}`,
    },
  });

  check(res, {
    "telemetry accepted": (r) => r.status === 201 || r.status === 200,
  });

  sleep(0.1);
}
