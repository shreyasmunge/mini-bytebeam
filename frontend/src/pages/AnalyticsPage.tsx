import { useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  fetchDeviceTelemetry,
  fetchFleetSnapshot,
  type FleetVehicle,
  type TelemetryPoint,
} from "../api";

type FilterKind = "all" | "online" | "offline" | "red";

export function AnalyticsPage() {
  const [filter, setFilter] = useState<FilterKind>("all");
  const [q, setQ] = useState("");
  const [fleet, setFleet] = useState<FleetVehicle[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [series, setSeries] = useState<TelemetryPoint[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const data = await fetchFleetSnapshot();
        if (cancelled) return;
        const demos = data.filter((v) => v.name.startsWith("demo-"));
        setFleet(demos.length > 0 ? demos : data);
        setError(null);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : "Failed to load");
      }
    }
    load();
    const id = setInterval(load, 3000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  useEffect(() => {
    if (!selected) {
      setSeries([]);
      return;
    }
    let cancelled = false;
    fetchDeviceTelemetry(selected)
      .then((rows) => !cancelled && setSeries([...rows].reverse()))
      .catch((e) => !cancelled && setError(e.message));
    return () => {
      cancelled = true;
    };
  }, [selected]);

  const totals = useMemo(() => {
    const online = fleet.filter((d) => d.status === "online").length;
    const offline = fleet.filter((d) => d.status === "offline").length;
    const red = fleet.filter((d) => d.in_red_zone).length;
    return { total: fleet.length, online, offline, red };
  }, [fleet]);

  const filtered = useMemo(() => {
    const query = q.trim().toLowerCase();
    return fleet.filter((d) => {
      if (query && !d.name.toLowerCase().includes(query)) return false;
      if (filter === "online") return d.status === "online";
      if (filter === "offline") return d.status === "offline";
      if (filter === "red") return d.in_red_zone;
      return true;
    });
  }, [fleet, filter, q]);

  const filteredCounts = useMemo(() => {
    const online = filtered.filter((d) => d.status === "online").length;
    const offline = filtered.filter((d) => d.status === "offline").length;
    const red = filtered.filter((d) => d.in_red_zone).length;
    return { shown: filtered.length, online, offline, red };
  }, [filtered]);

  return (
    <main className="min-h-screen bg-[linear-gradient(160deg,#d9cdb8_0%,#c4b49a_45%,#b8a482_100%)] px-4 pb-16 pt-28 md:px-8">
      <div className="mx-auto max-w-6xl">
        <h1 className="brand text-4xl text-[var(--ink)]">Analytics</h1>
        <p className="mt-2 max-w-2xl text-[var(--muted)]">
          Fleet health counts, filters, then open a device for recent telemetry.
        </p>

        <div className="mt-6 grid grid-cols-1 gap-3 sm:grid-cols-3">
          <StatCard
            label="Online"
            value={totals.online}
            hint={`of ${totals.total} devices`}
            tone="green"
            active={filter === "online"}
            onClick={() => setFilter(filter === "online" ? "all" : "online")}
          />
          <StatCard
            label="Red zone"
            value={totals.red}
            hint="alerts or offline"
            tone="red"
            active={filter === "red"}
            onClick={() => setFilter(filter === "red" ? "all" : "red")}
          />
          <StatCard
            label="Offline"
            value={totals.offline}
            hint="no recent telemetry"
            tone="muted"
            active={filter === "offline"}
            onClick={() => setFilter(filter === "offline" ? "all" : "offline")}
          />
        </div>

        <div className="mt-6 flex flex-wrap gap-3">
          <select
            value={filter}
            onChange={(e) => setFilter(e.target.value as FilterKind)}
            className="rounded-sm border border-[#8a7358] bg-[#f4ead8] px-3 py-2 text-sm"
          >
            <option value="all">All devices</option>
            <option value="online">Online only</option>
            <option value="offline">Offline only</option>
            <option value="red">Red zone only</option>
          </select>
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Filter by name"
            className="min-w-[220px] flex-1 rounded-sm border border-[#8a7358] bg-[#f4ead8] px-3 py-2 text-sm"
          />
        </div>

        <div className="mt-4 flex flex-wrap gap-6 text-sm text-[var(--ink)]">
          <span>
            <strong>{filteredCounts.shown}</strong> shown
          </span>
          <span>
            <strong>{filteredCounts.online}</strong> online
          </span>
          <span>
            <strong>{filteredCounts.red}</strong> red zone
          </span>
          <span>
            <strong>{filteredCounts.offline}</strong> offline
          </span>
        </div>
        {error && <p className="mt-2 text-sm text-[var(--rust)]">{error}</p>}

        <div className="mt-6 overflow-x-auto rounded-sm border border-[#8a7358]/20 bg-[#f4ead8]/70">
          <table className="min-w-full text-left text-sm">
            <thead className="border-b border-[#8a7358]/30 text-[var(--muted)]">
              <tr>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Health</th>
                <th className="px-4 py-3 font-medium">Battery</th>
                <th className="px-4 py-3 font-medium">Engine</th>
                <th className="px-4 py-3 font-medium">Last seen</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((d) => (
                <tr
                  key={d.id}
                  onClick={() => setSelected(d.id)}
                  className={`cursor-pointer border-b border-[#8a7358]/15 hover:bg-[#ead9bc] ${
                    selected === d.id ? "bg-[#ead9bc]" : ""
                  }`}
                >
                  <td className="px-4 py-3 font-medium">{d.name}</td>
                  <td className="px-4 py-3 capitalize">{d.status}</td>
                  <td className="px-4 py-3">
                    {d.in_red_zone ? (
                      <span className="text-[var(--rust)]">Red zone</span>
                    ) : (
                      <span className="text-[var(--moss)]">OK</span>
                    )}
                  </td>
                  <td className="px-4 py-3">
                    {d.battery_pct != null ? `${d.battery_pct.toFixed(0)}%` : "—"}
                  </td>
                  <td className="px-4 py-3">
                    {d.engine_temp_c != null
                      ? `${d.engine_temp_c.toFixed(0)}°C`
                      : "—"}
                  </td>
                  <td className="px-4 py-3 text-[var(--muted)]">
                    {d.last_seen_at
                      ? new Date(d.last_seen_at).toLocaleString()
                      : "—"}
                  </td>
                </tr>
              ))}
              {filtered.length === 0 && (
                <tr>
                  <td className="px-4 py-6 text-[var(--muted)]" colSpan={6}>
                    No devices match these filters.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        {selected && (
          <section className="mt-8">
            <h2 className="brand text-2xl">Recent battery & temp</h2>
            <div className="mt-4 h-72 rounded-sm border border-[#8a7358]/20 bg-[#f4ead8]/80 p-3">
              {series.length === 0 ? (
                <p className="p-4 text-sm text-[var(--muted)]">No telemetry yet.</p>
              ) : (
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={series}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#c4b49a" />
                    <XAxis
                      dataKey="recorded_at"
                      tickFormatter={(v) =>
                        new Date(v).toLocaleTimeString([], {
                          hour: "2-digit",
                          minute: "2-digit",
                        })
                      }
                      hide={series.length > 20}
                    />
                    <YAxis />
                    <Tooltip
                      labelFormatter={(v) => new Date(String(v)).toLocaleString()}
                    />
                    <Bar dataKey="battery_pct" fill="#3d5a3c" name="Battery %" />
                    <Bar dataKey="engine_temp_c" fill="#8b3a2a" name="Engine °C" />
                  </BarChart>
                </ResponsiveContainer>
              )}
            </div>
          </section>
        )}
      </div>
    </main>
  );
}

function StatCard({
  label,
  value,
  hint,
  tone,
  active,
  onClick,
}: {
  label: string;
  value: number;
  hint: string;
  tone: "green" | "red" | "muted";
  active: boolean;
  onClick: () => void;
}) {
  const valueColor =
    tone === "green"
      ? "text-[var(--moss)]"
      : tone === "red"
        ? "text-[var(--rust)]"
        : "text-[var(--muted)]";

  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-sm border px-4 py-4 text-left transition ${
        active
          ? "border-[var(--ink)] bg-[#f4ead8]"
          : "border-[#8a7358]/25 bg-[#f4ead8]/70 hover:bg-[#f4ead8]"
      }`}
    >
      <p className="text-xs uppercase tracking-[0.16em] text-[var(--muted)]">
        {label}
      </p>
      <p className={`brand mt-1 text-4xl leading-none ${valueColor}`}>{value}</p>
      <p className="mt-2 text-xs text-[var(--muted)]">{hint}</p>
    </button>
  );
}
