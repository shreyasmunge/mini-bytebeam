import { useEffect, useMemo, useRef, useState } from "react";
import { MapContainer, TileLayer, Marker, Tooltip, useMap } from "react-leaflet";
import L from "leaflet";
import { fetchFleetSnapshot, isBrowserDemoMode, type FleetVehicle } from "../api";

function dotIcon(v: FleetVehicle) {
  const kind = v.in_red_zone ? "alert" : v.status === "online" ? "online" : "offline";
  return L.divIcon({
    className: "",
    html: `<div class="vehicle-dot ${kind}"></div>`,
    iconSize: [14, 14],
    iconAnchor: [7, 7],
  });
}

/** Fit the map once when GPS first appears — never again on poll updates. */
function FitBoundsOnce({ vehicles }: { vehicles: FleetVehicle[] }) {
  const map = useMap();
  const didFit = useRef(false);

  useEffect(() => {
    if (didFit.current) return;

    const pts = vehicles.filter(
      (v) => v.latitude != null && v.longitude != null,
    ) as Array<FleetVehicle & { latitude: number; longitude: number }>;
    if (pts.length === 0) return;

    didFit.current = true;
    if (pts.length === 1) {
      map.setView([pts[0].latitude, pts[0].longitude], 13);
      return;
    }
    const bounds = L.latLngBounds(pts.map((p) => [p.latitude, p.longitude]));
    map.fitBounds(bounds.pad(0.25));
  }, [vehicles, map]);

  return null;
}

function tipHtml(v: FleetVehicle) {
  const bat = v.battery_pct?.toFixed(0) ?? "—";
  const spd = v.speed_kmh?.toFixed(0) ?? "—";
  const temp = v.engine_temp_c?.toFixed(0) ?? "—";
  const seen = v.last_seen_at
    ? new Date(v.last_seen_at).toLocaleString()
    : "never";
  return `
    <strong>${v.name}</strong><br/>
    Status: ${v.status}${v.in_red_zone ? " · RED ZONE" : ""}<br/>
    Battery: ${bat}% · Speed: ${spd} km/h<br/>
    Engine: ${temp}°C<br/>
    Last seen: ${seen}
  `;
}

export function MapPage() {
  const [vehicles, setVehicles] = useState<FleetVehicle[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const data = await fetchFleetSnapshot();
        if (!cancelled) {
          setVehicles(data);
          setUpdatedAt(new Date());
          setError(null);
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : "Failed to load fleet");
      }
    }
    load();
    const id = setInterval(load, 2000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  // Prefer live demo fleet; fall back to everything with GPS.
  const focus = useMemo(() => {
    const withGps = vehicles.filter((v) => v.latitude != null && v.longitude != null);
    const demos = withGps.filter((v) => v.name.startsWith("demo-"));
    return demos.length > 0 ? demos : withGps;
  }, [vehicles]);

  const onMap = focus;
  const online = focus.filter((v) => v.status === "online").length;
  const red = focus.filter((v) => v.in_red_zone).length;

  return (
    <main className="relative h-screen w-screen overflow-hidden">
      <MapContainer
        className="vintage-map h-full w-full"
        center={[12.9716, 77.5946]}
        zoom={11}
        zoomControl={true}
        attributionControl={true}
      >
        <TileLayer
          attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OSM</a>'
          url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
        />
        <FitBoundsOnce vehicles={onMap} />
        {onMap.map((v) => (
          <Marker
            key={v.id}
            position={[v.latitude as number, v.longitude as number]}
            icon={dotIcon(v)}
          >
            <Tooltip
              className="fleet-tip"
              direction="top"
              offset={[0, -8]}
              opacity={1}
              permanent={false}
            >
              <div dangerouslySetInnerHTML={{ __html: tipHtml(v) }} />
            </Tooltip>
          </Marker>
        ))}
      </MapContainer>

      <aside className="pointer-events-none absolute inset-x-3 bottom-3 z-[1000] max-h-[42vh] overflow-y-auto rounded-sm bg-[var(--chrome)] px-3 py-2.5 text-[#f4ead8] shadow-lg backdrop-blur-sm sm:inset-x-auto sm:bottom-4 sm:left-4 sm:max-h-none sm:max-w-sm sm:px-4 sm:py-3 md:bottom-6 md:left-6">
        <p className="text-[10px] uppercase tracking-[0.18em] text-[#d2c4b0] sm:text-xs">
          Chart legend
        </p>
        <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1.5 text-xs sm:text-sm">
          <span className="inline-flex items-center gap-2">
            <span className="vehicle-dot online" /> Online
          </span>
          <span className="inline-flex items-center gap-2">
            <span className="vehicle-dot offline" /> Offline
          </span>
          <span className="inline-flex items-center gap-2">
            <span className="vehicle-dot alert" /> Red zone
          </span>
        </div>
        <p className="mt-2 text-xs text-[#d2c4b0] sm:mt-3 sm:text-sm">
          {focus.length} devices · {online} online · {red} red zone
          {updatedAt ? ` · synced ${updatedAt.toLocaleTimeString()}` : ""}
        </p>
        <p className="mt-1 text-[11px] text-[#b9a890] sm:text-xs">
          Polling every 2s
          {isBrowserDemoMode()
            ? " · browser demo mode (no hosted API yet)"
            : " · live API"}
        </p>
        {onMap.length > 0 && (
          <ul className="mt-2 max-h-24 space-y-1 overflow-y-auto text-[11px] text-[#d2c4b0] sm:mt-3 sm:max-h-36 sm:text-xs">
            {focus.slice(0, 12).map((v) => (
              <li key={v.id} className="flex justify-between gap-3">
                  <span className="truncate">{v.name}</span>
                  <span
                    className={
                      v.in_red_zone
                        ? "text-[#f0a090]"
                        : v.status === "online"
                          ? "text-[#9bc49a]"
                          : "text-[#b9a890]"
                    }
                  >
                    {v.status}
                    {v.in_red_zone ? " · alert" : ""}
                  </span>
                </li>
              ))}
          </ul>
        )}
        {onMap.length === 0 && (
          <p className="mt-2 text-xs text-[#f0c9a0] sm:text-sm">
            No GPS yet — open Pipeline and fire a blast, or run demo_fleet locally.
          </p>
        )}
        {error && <p className="mt-2 text-xs text-[#f0a090] sm:text-sm">API: {error}</p>}
      </aside>
    </main>
  );
}
