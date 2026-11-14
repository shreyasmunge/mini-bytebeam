import { NavLink, Outlet } from "react-router-dom";

const linkClass = ({ isActive }: { isActive: boolean }) =>
  `px-3 py-1 text-sm tracking-wide ${
    isActive ? "text-[#f4ead8] underline underline-offset-4" : "text-[#d2c4b0] hover:text-[#f4ead8]"
  }`;

const githubBtn =
  "ml-1 rounded-sm bg-[#f4ead8]/15 px-3 py-1 text-sm text-[#f4ead8] hover:bg-[#f4ead8]/25";

const GITHUB_URL =
  (import.meta.env.VITE_GITHUB_URL as string | undefined)?.trim() ||
  "https://github.com/shreyasmunge/mini-bytebeam";

/** Always on the dashboard — no Vercel build-env required */
const RELATED_GITHUB_URL = "https://github.com/shreyasmunge/energygrid-data-aggregator";

export function Layout() {
  return (
    <div className="min-h-screen">
      <header className="pointer-events-none fixed inset-x-0 top-0 z-[1000] flex items-start justify-between gap-4 p-4 md:p-6">
        <div className="pointer-events-auto max-w-md rounded-sm bg-[var(--chrome)] px-4 py-3 text-[#f4ead8] shadow-lg backdrop-blur-sm">
          <p className="brand text-3xl leading-none md:text-4xl">FleetPulse</p>
          <p className="mt-1 text-sm text-[#d2c4b0]">Live fleet map</p>
        </div>
        <nav className="pointer-events-auto flex flex-wrap items-center gap-1 rounded-sm bg-[var(--chrome)] px-2 py-2 shadow-lg backdrop-blur-sm">
          <NavLink to="/" end className={linkClass}>
            Map
          </NavLink>
          <NavLink to="/analytics" className={linkClass}>
            Analytics
          </NavLink>
          <NavLink to="/thresholds" className={linkClass}>
            Red zone
          </NavLink>
          {GITHUB_URL && (
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noreferrer"
              className={githubBtn}
              title="FleetPulse codebase"
            >
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
      </header>
      <Outlet />
    </div>
  );
}
