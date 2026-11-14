import { useEffect, useMemo, useState } from "react";
import {
  fetchPipeline,
  isBrowserDemoMode,
  playgroundIngest,
  registerDevice,
  sendTelemetry,
  type PipelineStatus,
} from "../api";

export function MetricsPage() {
  const [data, setData] = useState<PipelineStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deviceName, setDeviceName] = useState("try-truck-1");
  const [apiKey, setApiKey] = useState("");
  const [deviceId, setDeviceId] = useState("");
  const [tryLog, setTryLog] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [lat, setLat] = useState(12.97);
  const [lon, setLon] = useState(77.59);
  const [battery, setBattery] = useState(78);
  const [temp, setTemp] = useState(91);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const row = await fetchPipeline();
        if (!cancelled) {
          setData(row);
          setError(null);
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : "Failed to load metrics");
        }
      }
    }
    load();
    const id = setInterval(load, 1500);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  const m = data?.metrics;
  const guide = data?.try_api;
  const sampleBody = useMemo(() => {
    const id =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : "00000000-0000-4000-8000-000000000099";
    return {
      id,
      recorded_at: new Date().toISOString(),
      latitude: lat,
      longitude: lon,
      speed_kmh: 42,
      battery_pct: battery,
      engine_temp_c: temp,
    };
  }, [lat, lon, battery, temp, tryLog]);

  async function onRegister() {
    setBusy(true);
    setTryLog("");
    try {
      const res = await registerDevice(deviceName.trim() || "try-truck");
      setApiKey(res.api_key);
      setDeviceId(res.id);
      setTryLog(`Registered ${res.name}\nid: ${res.id}\napi_key: ${res.api_key}`);
    } catch (e) {
      setTryLog(e instanceof Error ? e.message : "Register failed");
    } finally {
      setBusy(false);
    }
  }

  async function onSend() {
    if (!apiKey) {
      setTryLog("Register a device first (need api_key).");
      return;
    }
    setBusy(true);
    try {
      const body = {
        id: crypto.randomUUID(),
        recorded_at: new Date().toISOString(),
        latitude: lat,
        longitude: lon,
        speed_kmh: 30 + Math.random() * 40,
        battery_pct: battery,
        engine_temp_c: temp,
      };
      const res = await sendTelemetry(apiKey, body);
      setTryLog(
        `POST /telemetry → ${res.status}\nalerts: ${(res.alerts_created ?? []).join(", ") || "none"}\n` +
          JSON.stringify(body, null, 2),
      );
    } catch (e) {
      setTryLog(e instanceof Error ? e.message : "Send failed");
    } finally {
      setBusy(false);
    }
  }

  async function onBlast(count: number, vehicles = 50) {
    setBusy(true);
    setTryLog("");
    try {
      const res = await playgroundIngest({ count, vehicles });
      setTryLog(
        `POST /playground/ingest\naccepted=${res.accepted} duplicates=${res.duplicates} vehicles=${res.vehicles_used}\n${res.hint}`,
      );
    } catch (e) {
      setTryLog(e instanceof Error ? e.message : "Blast failed");
    } finally {
      setBusy(false);
    }
  }

  const apiBase =
    (import.meta.env.VITE_API_BASE as string | undefined)?.replace(/\/$/, "") ||
    guide?.base_url?.replace(/\/$/, "") ||
    "http://127.0.0.1:3000";
  const hostedMax = 200;
  const blastCurl = `curl -s -X POST ${apiBase}/playground/ingest \\
  -H 'content-type: application/json' \\
  -d '{"count":100}'`;

  return (
    <main className="mx-auto max-w-5xl px-4 pb-16 pt-28 md:pt-32">
      <h1 className="brand text-4xl text-[var(--ink)]">Pipeline</h1>
      <p className="mt-2 text-[var(--muted)]">
        Live ingest via the hosted API. Small demo blasts only on free hosting.
      </p>

      {isBrowserDemoMode() && (
        <p className="mt-3 text-sm text-amber-900">
          No VITE_API_BASE — browser demo only. After deploy, rebuild with the Render API URL.
        </p>
      )}

      {error && <p className="mt-4 text-sm text-red-800">{error}</p>}

      <section className="mt-8 border border-[#cbbba3] p-5">
        <p className="text-xs uppercase tracking-wide text-[var(--muted)]">
          Hosted demo endpoint (no API key)
        </p>
        <p className="mt-2 break-all font-mono text-lg text-[var(--ink)]">
          POST {apiBase}/playground/ingest
        </p>
        <p className="mt-2 text-sm text-[var(--muted)]">
          Free server demo is capped (~{hostedMax} events/request). Locally we tested{" "}
          <strong className="font-medium text-[var(--ink)]">
            1000 vehicles · ~29k events · 0 errors · ~445 msg/s
          </strong>{" "}
          — the hosted free tier will not match that.
        </p>
        <pre className="mt-3 overflow-x-auto border border-[#cbbba3] bg-[#f4ead8]/30 p-3 text-xs text-[var(--ink)]">
          {blastCurl}
        </pre>
        <div className="mt-4 flex flex-wrap gap-2">
          <button
            type="button"
            disabled={busy || isBrowserDemoMode()}
            onClick={() => onBlast(100, 10)}
            className="bg-[var(--chrome)] px-3 py-2 text-sm text-[#f4ead8] disabled:opacity-50"
          >
            Fire 100 events
          </button>
          <button
            type="button"
            disabled={busy || isBrowserDemoMode()}
            onClick={() => onBlast(hostedMax, 20)}
            className="bg-[var(--chrome)] px-3 py-2 text-sm text-[#f4ead8] disabled:opacity-50"
          >
            Fire {hostedMax} events
          </button>
        </div>
        {tryLog && tryLog.includes("playground") && (
          <pre className="mt-4 max-h-40 overflow-auto border border-[#cbbba3] p-3 text-xs text-[var(--ink)]">
            {tryLog}
          </pre>
        )}
      </section>

      {guide && (
        <section className="mt-8">
          <h2 className="text-lg text-[var(--ink)]">What we benchmarked</h2>
          <p className="mt-2 text-sm text-[var(--muted)]">{guide.last_local_benchmark}</p>
        </section>
      )}

      {data && m && (
        <section className="mt-8 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          <Stat label="Live ≈ msg/s" value={m.approx_ingest_per_sec.toFixed(0)} />
          <Stat label="Accepted events" value={String(m.ingested_total)} />
          <Stat label="Duplicates" value={String(m.duplicates_total)} />
          <Stat label="Errors" value={String(m.ingest_errors_total)} />
          <Stat label="Online devices" value={String(data.devices_online)} />
          <Stat label="Open alerts" value={String(data.open_alerts)} />
        </section>
      )}

      <section className="mt-12 grid gap-10 lg:grid-cols-2">
        <div>
          <h2 className="text-lg text-[var(--ink)]">Single-device try (auth path)</h2>
          <p className="mt-1 text-sm text-[var(--muted)]">
            Optional: register + Bearer key for production-style ingest.
          </p>

          <label className="mt-4 block text-xs uppercase text-[var(--muted)]">Device name</label>
          <input
            className="mt-1 w-full border border-[#cbbba3] bg-transparent px-3 py-2 text-[var(--ink)]"
            value={deviceName}
            onChange={(e) => setDeviceName(e.target.value)}
          />

          <div className="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={onRegister}
              className="bg-[var(--chrome)] px-3 py-2 text-sm text-[#f4ead8] disabled:opacity-50"
            >
              Register device
            </button>
            <button
              type="button"
              disabled={busy || !apiKey}
              onClick={onSend}
              className="bg-[var(--chrome)] px-3 py-2 text-sm text-[#f4ead8] disabled:opacity-50"
            >
              Send telemetry
            </button>
          </div>

          {apiKey && (
            <p className="mt-3 break-all text-xs text-[var(--muted)]">
              device {deviceId.slice(0, 8)}… · key {apiKey.slice(0, 18)}…
            </p>
          )}

          <div className="mt-4 grid grid-cols-2 gap-3">
            <Num label="latitude" value={lat} set={setLat} step={0.01} />
            <Num label="longitude" value={lon} set={setLon} step={0.01} />
            <Num label="battery %" value={battery} set={setBattery} step={1} />
            <Num label="engine °C" value={temp} set={setTemp} step={1} />
          </div>

          {tryLog && (
            <pre className="mt-4 max-h-48 overflow-auto border border-[#cbbba3] p-3 text-xs text-[var(--ink)]">
              {tryLog}
            </pre>
          )}
        </div>

        <div>
          <h2 className="text-lg text-[var(--ink)]">Endpoints to hit</h2>
          {guide && (
            <ul className="mt-3 space-y-4 text-sm">
              <EndpointCard ep={guide.register} base={guide.base_url} />
              <EndpointCard ep={guide.telemetry} base={guide.base_url} />
              <EndpointCard ep={guide.batch} base={guide.base_url} />
              <EndpointCard ep={guide.pipeline} base={guide.base_url} />
            </ul>
          )}
          <p className="mt-4 text-xs text-[var(--muted)]">Sample body shape:</p>
          <pre className="mt-1 max-h-40 overflow-auto border border-[#cbbba3] p-3 text-xs">
            {JSON.stringify(sampleBody, null, 2)}
          </pre>
        </div>
      </section>

      <section className="mt-12">
        <h2 className="text-lg text-[var(--ink)]">Recent requests</h2>
        <p className="mt-1 text-sm text-[var(--muted)]">
          What happened on each ingest (created / duplicate / partial / error).
        </p>
        <div className="mt-4 overflow-x-auto">
          <table className="w-full min-w-[640px] text-left text-sm">
            <thead>
              <tr className="border-b border-[#cbbba3] text-[var(--muted)]">
                <th className="py-2 pr-3 font-normal">Time</th>
                <th className="py-2 pr-3 font-normal">Device</th>
                <th className="py-2 pr-3 font-normal">Path</th>
                <th className="py-2 pr-3 font-normal">Outcome</th>
                <th className="py-2 pr-3 font-normal">Events</th>
                <th className="py-2 font-normal">Detail</th>
              </tr>
            </thead>
            <tbody>
              {(data?.recent ?? []).length === 0 && (
                <tr>
                  <td colSpan={6} className="py-4 text-[var(--muted)]">
                    No ingest yet — use Try the API or run load_fleet.
                  </td>
                </tr>
              )}
              {(data?.recent ?? []).map((r, i) => (
                <tr key={`${r.at}-${i}`} className="border-b border-[#cbbba3]/60">
                  <td className="py-2 pr-3 whitespace-nowrap text-[var(--muted)]">
                    {new Date(r.at).toLocaleTimeString()}
                  </td>
                  <td className="py-2 pr-3">{r.device_name}</td>
                  <td className="py-2 pr-3 font-mono text-xs">{r.path}</td>
                  <td className="py-2 pr-3">
                    <OutcomeBadge outcome={r.outcome} />
                  </td>
                  <td className="py-2 pr-3">{r.events}</td>
                  <td className="py-2 text-[var(--muted)]">
                    {r.detail}
                    {r.alerts?.length ? ` · alerts: ${r.alerts.join(", ")}` : ""}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </main>
  );
}

function Num({
  label,
  value,
  set,
  step,
}: {
  label: string;
  value: number;
  set: (n: number) => void;
  step: number;
}) {
  return (
    <label className="block text-xs text-[var(--muted)]">
      {label}
      <input
        type="number"
        step={step}
        className="mt-1 w-full border border-[#cbbba3] bg-transparent px-2 py-1 text-[var(--ink)]"
        value={value}
        onChange={(e) => set(Number(e.target.value))}
      />
    </label>
  );
}

function EndpointCard({
  ep,
  base,
}: {
  ep: {
    method: string;
    path: string;
    headers: string[];
    notes: string;
  };
  base: string;
}) {
  return (
    <li className="border-b border-[#cbbba3] pb-3">
      <p className="font-mono text-[var(--ink)]">
        <span className="text-[var(--muted)]">{ep.method}</span> {base}
        {ep.path}
      </p>
      {ep.headers.length > 0 && (
        <p className="mt-1 text-xs text-[var(--muted)]">{ep.headers.join(" · ")}</p>
      )}
      <p className="mt-1 text-xs text-[var(--muted)]">{ep.notes}</p>
    </li>
  );
}

function OutcomeBadge({ outcome }: { outcome: string }) {
  const color =
    outcome === "created"
      ? "text-emerald-800"
      : outcome === "error"
        ? "text-red-800"
        : "text-amber-900";
  return <span className={`font-medium ${color}`}>{outcome}</span>;
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="border-b border-[#cbbba3] pb-3">
      <p className="text-xs uppercase tracking-wide text-[var(--muted)]">{label}</p>
      <p className="mt-1 text-3xl text-[var(--ink)]">{value}</p>
    </div>
  );
}
