/**
 * k6 load test — thousands of virtual vehicles.
 *
 * Single-event:
 *   k6 run benchmarks/telemetry.js
 *
 * Batch (higher msg/s, fewer HTTP calls):
 *   BATCH_SIZE=50 VUS=200 k6 run benchmarks/telemetry.js
 *
 * Multi-device (each VU registers its own vehicle):
 *   MULTI_DEVICE=1 VUS=1000 k6 run benchmarks/telemetry.js
 */

import http from "k6/http";
import { check, sleep } from "k6";
import { uuidv4 } from "https://jslib.k6.io/k6-utils/1.4.0/index.js";

const VUS = Number(__ENV.VUS || 200);
const DURATION = __ENV.DURATION || "30s";
const BATCH_SIZE = Number(__ENV.BATCH_SIZE || 1);
const MULTI = __ENV.MULTI_DEVICE === "1";

export const options = {
  vus: VUS,
  duration: DURATION,
  thresholds: {
    http_req_failed: ["rate<0.05"],
    http_req_duration: ["p(95)<500"],
  },
};

const BASE = __ENV.API_BASE || "http://127.0.0.1:3000";

export function setup() {
  if (__ENV.API_KEY && !MULTI) {
    return { apiKey: __ENV.API_KEY, multi: false };
  }
  if (MULTI) {
    return { multi: true };
  }
  const res = http.post(
    `${BASE}/devices`,
    JSON.stringify({ name: `k6-${Date.now()}` }),
    { headers: { "Content-Type": "application/json" } },
  );
  check(res, { "register 201": (r) => r.status === 201 });
  return { apiKey: res.json().api_key, multi: false };
}

function oneEvent() {
  return {
    id: uuidv4(),
    recorded_at: new Date().toISOString(),
    latitude: 12.97,
    longitude: 77.59,
    speed_kmh: 40 + Math.random() * 20,
    battery_pct: 50 + Math.random() * 40,
    engine_temp_c: 80 + Math.random() * 20,
  };
}

export default function (data) {
  let apiKey = data.apiKey;
  if (data.multi) {
    if (!__ENV[`KEY_${__VU}`]) {
      const res = http.post(
        `${BASE}/devices`,
        JSON.stringify({ name: `k6-vu-${__VU}` }),
        { headers: { "Content-Type": "application/json" } },
      );
      apiKey = res.json().api_key;
    }
  }

  // Per-VU registration once (MULTI path without sticky env).
  if (data.multi && !apiKey) {
    const res = http.post(
      `${BASE}/devices`,
      JSON.stringify({ name: `k6-vu-${__VU}-${Date.now()}` }),
      { headers: { "Content-Type": "application/json" } },
    );
    apiKey = res.json().api_key;
  }

  const headers = {
    "Content-Type": "application/json",
    Authorization: `Bearer ${apiKey}`,
  };

  let res;
  if (BATCH_SIZE > 1) {
    const events = [];
    for (let i = 0; i < BATCH_SIZE; i++) events.push(oneEvent());
    res = http.post(`${BASE}/telemetry/batch`, JSON.stringify({ events }), {
      headers,
    });
  } else {
    res = http.post(`${BASE}/telemetry`, JSON.stringify(oneEvent()), { headers });
  }

  check(res, {
    "telemetry accepted": (r) => r.status === 201 || r.status === 200,
  });

  sleep(Number(__ENV.SLEEP || 0.05));
}
