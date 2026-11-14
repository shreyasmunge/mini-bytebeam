import { useState } from "react";
import { NavLink, Outlet } from "react-router-dom";

const linkClass = ({ isActive }: { isActive: boolean }) =>
  `whitespace-nowrap px-2.5 py-1.5 text-sm tracking-wide md:px-3 md:py-1 ${
    isActive ? "text-[#f4ead8] underline underline-offset-4" : "text-[#d2c4b0] hover:text-[#f4ead8]"
  }`;

const githubBtn =
  "whitespace-nowrap rounded-sm bg-[#f4ead8]/15 px-2.5 py-1.5 text-sm text-[#f4ead8] hover:bg-[#f4ead8]/25 md:ml-1 md:px-3 md:py-1";

const GITHUB_URL =
  (import.meta.env.VITE_GITHUB_URL as string | undefined)?.trim() ||
  "https://github.com/shreyasmunge/mini-bytebeam";

/** Always on the dashboard — no Vercel build-env required */
const RELATED_GITHUB_URL = "https://github.com/shreyasmunge/energygrid-data-aggregator";

const links = [
  { to: "/", end: true, label: "Map" },
  { to: "/analytics", end: false, label: "Analytics" },
  { to: "/alerts", end: false, label: "Alerts" },
  { to: "/pipeline", end: false, label: "Pipeline" },
  { to: "/thresholds", end: false, label: "Red zone" },
] as const;

export function Layout() {
  const [open, setOpen] = useState(false);

  return (
    <div className="min-h-screen">
      <header className="pointer-events-none fixed inset-x-0 top-0 z-[1000] p-3 sm:p-4 md:p-6">
        <div className="flex items-start justify-between gap-3">
          <div className="pointer-events-auto max-w-[70%] rounded-sm bg-[var(--chrome)] px-3 py-2.5 text-[#f4ead8] shadow-lg backdrop-blur-sm sm:max-w-md sm:px-4 sm:py-3">
            <p className="brand text-2xl leading-none sm:text-3xl md:text-4xl">FleetPulse</p>
            <p className="mt-1 text-xs text-[#d2c4b0] sm:text-sm">Live fleet map</p>
          </div>

          {/* Desktop / tablet nav */}
          <nav className="pointer-events-auto hidden max-w-[58%] flex-wrap items-center justify-end gap-0.5 rounded-sm bg-[var(--chrome)] px-2 py-2 shadow-lg backdrop-blur-sm md:flex">
            {links.map((l) => (
              <NavLink key={l.to} to={l.to} end={l.end} className={linkClass}>
                {l.label}
              </NavLink>
            ))}
            {GITHUB_URL && (
              <a href={GITHUB_URL} target="_blank" rel="noreferrer" className={githubBtn} title="FleetPulse codebase">
                GitHub
              </a>
            )}
            {RELATED_GITHUB_URL && (
              <a
                href={RELATED_GITHUB_URL}
                target="_blank"
                rel="noreferrer"
                className={githubBtn}
                title="Related project: energygrid-data-aggregator"
              >
                Relevant work
              </a>
            )}
          </nav>

          {/* Mobile menu toggle */}
          <button
            type="button"
            aria-expanded={open}
            aria-label={open ? "Close menu" : "Open menu"}
            onClick={() => setOpen((v) => !v)}
            className="pointer-events-auto flex h-11 w-11 shrink-0 items-center justify-center rounded-sm bg-[var(--chrome)] text-[#f4ead8] shadow-lg backdrop-blur-sm md:hidden"
          >
            <span className="sr-only">Menu</span>
            <span aria-hidden className="flex flex-col gap-1.5">
              <span className={`block h-0.5 w-5 bg-current transition ${open ? "translate-y-2 rotate-45" : ""}`} />
              <span className={`block h-0.5 w-5 bg-current transition ${open ? "opacity-0" : ""}`} />
              <span className={`block h-0.5 w-5 bg-current transition ${open ? "-translate-y-2 -rotate-45" : ""}`} />
            </span>
          </button>
        </div>

        {open && (
          <nav className="pointer-events-auto mt-2 flex flex-col gap-1 rounded-sm bg-[var(--chrome)] p-2 shadow-lg backdrop-blur-sm md:hidden">
            {links.map((l) => (
              <NavLink
                key={l.to}
                to={l.to}
                end={l.end}
                className={linkClass}
                onClick={() => setOpen(false)}
              >
                {l.label}
              </NavLink>
            ))}
            {GITHUB_URL && (
              <a href={GITHUB_URL} target="_blank" rel="noreferrer" className={githubBtn} onClick={() => setOpen(false)}>
                GitHub
              </a>
            )}
            {RELATED_GITHUB_URL && (
              <a
                href={RELATED_GITHUB_URL}
                target="_blank"
                rel="noreferrer"
                className={githubBtn}
                onClick={() => setOpen(false)}
              >
                Relevant work
              </a>
            )}
          </nav>
        )}
      </header>
      <Outlet />
    </div>
  );
}
