import { useEffect, useState } from "react";
import { ackAlert, fetchAlerts, type FleetAlert } from "../api";

export function AlertsPage() {
  const [alerts, setAlerts] = useState<FleetAlert[]>([]);
  const [status, setStatus] = useState<"open" | "all">("open");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  async function load() {
    try {
      const rows = await fetchAlerts(status);
      setAlerts(rows);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load alerts");
    }
  }

  useEffect(() => {
    load();
    const id = setInterval(load, 3000);
    return () => clearInterval(id);
  }, [status]);

  async function onAck(id: string) {
    setBusy(id);
    try {
      await ackAlert(id);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Ack failed");
    } finally {
      setBusy(null);
    }
  }

  return (
    <main className="mx-auto max-w-4xl px-4 pb-16 pt-28 md:pt-32">
      <h1 className="brand text-4xl text-[var(--ink)]">Alerts</h1>
      <p className="mt-2 text-[var(--muted)]">
        Open incidents from twin rules — acknowledge to clear the inbox.
      </p>

      <div className="mt-6 flex gap-2">
        {(["open", "all"] as const).map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => setStatus(s)}
            className={`px-3 py-1 text-sm ${
              status === s
                ? "bg-[var(--chrome)] text-[#f4ead8]"
                : "bg-[#e8dcc8] text-[var(--ink)]"
            }`}
          >
            {s}
          </button>
        ))}
      </div>

      {error && <p className="mt-4 text-sm text-red-800">{error}</p>}

      <ul className="mt-6 space-y-3">
        {alerts.length === 0 && (
          <li className="text-sm text-[var(--muted)]">No alerts.</li>
        )}
        {alerts.map((a) => (
          <li
            key={a.id}
            className="flex flex-wrap items-start justify-between gap-3 border-b border-[#cbbba3] pb-3"
          >
            <div>
              <p className="font-medium text-[var(--ink)]">
                {a.device_name ?? a.device_id.slice(0, 8)} · {a.kind}
              </p>
              <p className="text-sm text-[var(--muted)]">{a.message}</p>
              <p className="mt-1 text-xs text-[var(--muted)]">
                {new Date(a.created_at).toLocaleString()}
                {a.acknowledged_at
                  ? ` · acked ${new Date(a.acknowledged_at).toLocaleString()}`
                  : ""}
              </p>
            </div>
            {!a.acknowledged_at && (
              <button
                type="button"
                disabled={busy === a.id}
                onClick={() => onAck(a.id)}
                className="bg-[var(--chrome)] px-3 py-1 text-sm text-[#f4ead8] disabled:opacity-50"
              >
                {busy === a.id ? "…" : "Acknowledge"}
              </button>
            )}
          </li>
        ))}
      </ul>
    </main>
  );
}
