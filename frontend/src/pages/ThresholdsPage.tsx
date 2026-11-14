import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { fetchThresholds, saveThresholds } from "../api";

export function ThresholdsPage() {
  const [engine, setEngine] = useState(110);
  const [battery, setBattery] = useState(15);
  const [offline, setOffline] = useState(60);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchThresholds()
      .then((t) => {
        setEngine(t.engine_temp_alert_c);
        setBattery(t.battery_low_pct);
        setOffline(t.offline_after_secs);
      })
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, []);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setMessage(null);
    setError(null);
    try {
      const saved = await saveThresholds({
        engine_temp_alert_c: engine,
        battery_low_pct: battery,
        offline_after_secs: offline,
      });
      setMessage(
        `Saved. New alerts use these limits${
          saved.updated_at ? ` · ${new Date(saved.updated_at).toLocaleString()}` : ""
        }.`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Save failed");
    }
  }

  return (
    <main className="page-shell min-h-screen bg-[linear-gradient(160deg,#d9cdb8_0%,#c4b49a_50%,#b8a482_100%)] px-3 pb-16 sm:px-4 md:px-8">
      <div className="mx-auto max-w-xl">
        <h1 className="brand text-4xl text-[var(--ink)]">Red zone limits</h1>
        <p className="mt-2 text-[var(--muted)]">
          Readings past these limits create alerts and paint map dots in rust red.
        </p>

        {loading ? (
          <p className="mt-8 text-sm text-[var(--muted)]">Loading…</p>
        ) : (
          <form
            onSubmit={onSubmit}
            className="mt-8 space-y-5 rounded-sm border border-[#8a7358]/25 bg-[#f4ead8]/85 p-6 shadow-sm"
          >
            <label className="block text-sm">
              <span className="font-medium">Engine temp alert (°C)</span>
              <input
                type="number"
                step="0.1"
                value={engine}
                onChange={(e) => setEngine(Number(e.target.value))}
                className="mt-1 w-full rounded-sm border border-[#8a7358] bg-white/70 px-3 py-2"
              />
            </label>
            <label className="block text-sm">
              <span className="font-medium">Low battery alert (%)</span>
              <input
                type="number"
                step="0.1"
                value={battery}
                onChange={(e) => setBattery(Number(e.target.value))}
                className="mt-1 w-full rounded-sm border border-[#8a7358] bg-white/70 px-3 py-2"
              />
            </label>
            <label className="block text-sm">
              <span className="font-medium">Offline after (seconds)</span>
              <input
                type="number"
                value={offline}
                onChange={(e) => setOffline(Number(e.target.value))}
                className="mt-1 w-full rounded-sm border border-[#8a7358] bg-white/70 px-3 py-2"
              />
            </label>
            <button
              type="submit"
              className="rounded-sm bg-[var(--ink)] px-4 py-2 text-sm font-medium text-[#f4ead8] hover:bg-[#2a221a]"
            >
              Save limits
            </button>
            {message && <p className="text-sm text-[var(--moss)]">{message}</p>}
            {error && <p className="text-sm text-[var(--rust)]">{error}</p>}
          </form>
        )}
      </div>
    </main>
  );
}
