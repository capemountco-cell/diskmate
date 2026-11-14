import { useCallback, useEffect, useState } from "react";

import { Button, Note, Spinner } from "../../components/primitives";
import { Sparkline } from "../../components/Sparkline";
import { formatBytes } from "../../lib/format";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import type { MemoryPressure, MonitorReport, ProcessMemory } from "../../lib/types";
import { StatusChip } from "../overview/OverviewPage";

const POLL_MS = 1_000;
const APPS_POLL_MS = 3_000;
const TOP_APPS = 8;

const PRESSURE: Record<
  MemoryPressure,
  { tone: "good" | "attention" | "act"; word: string; quiet?: boolean } | null
> = {
  normal: { tone: "good", word: "Normal", quiet: true },
  warning: { tone: "attention", word: "Under pressure" },
  critical: { tone: "act", word: "High" },
  unknown: null,
};

/**
 * Memory, watched rather than sampled once.
 *
 * There is no "free memory" button here and there will not be one. macOS uses
 * memory that would otherwise sit idle as cache; emptying it does not make a
 * Mac faster, it makes it slower until the cache refills. What matters is
 * pressure over time, and, when it is up, which app is worth quitting. That is
 * what this page shows: a headline, the last few minutes as a graph in the
 * page's colour, and the apps using the most, largest first.
 */
export function MemoryPage({
  onPressure,
}: {
  /** Tells the shell the current pressure, for the dot on Memory's tab. */
  onPressure?: (pressure: MemoryPressure) => void;
} = {}) {
  const [report, setReport] = useState<MonitorReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [apps, setApps] = useState<ProcessMemory[] | null>(null);
  const [armed, setArmed] = useState<number | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const poll = useCallback(async () => {
    try {
      setReport(await engine.monitorReport());
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    }
  }, []);

  const loadApps = useCallback(async () => {
    try {
      setApps(await engine.topApps(TOP_APPS));
    } catch (cause) {
      setMessage(describeError(cause));
    }
  }, []);

  useEffect(() => {
    if (!engineAvailable()) {
      setError("The Mr Clean engine is not running.");
      return;
    }
    // Start the sampler on arrival and stop it on leaving: a monitor nobody is
    // looking at is just background work.
    void engine.monitorStart(POLL_MS, null).then(poll);
    void loadApps();
    const timer = window.setInterval(poll, POLL_MS);
    const appsTimer = window.setInterval(loadApps, APPS_POLL_MS);
    return () => {
      window.clearInterval(timer);
      window.clearInterval(appsTimer);
      void engine.monitorStop();
    };
  }, [poll, loadApps]);

  const pressure = report?.latest?.pressure ?? "unknown";
  useEffect(() => {
    onPressure?.(pressure);
  }, [pressure, onPressure]);

  // An armed Quit disarms itself, so a stray click later does nothing.
  useEffect(() => {
    if (armed === null) return;
    const timer = window.setTimeout(() => setArmed(null), 4_000);
    return () => window.clearTimeout(timer);
  }, [armed]);

  /**
   * Quit takes two clicks: the first arms the button, the second sends the
   * request. It is the same request the app's own Quit menu item sends, so the
   * app can still ask about unsaved work.
   */
  const quit = async (app: ProcessMemory) => {
    setArmed(null);
    try {
      await engine.quitApp(app.pid, app.name);
      setMessage(`Asked ${app.name} to quit. It may ask you to save first.`);
      window.setTimeout(loadApps, 1_500);
    } catch (cause) {
      setMessage(describeError(cause));
    }
  };

  const quitButton = (app: ProcessMemory) =>
    armed === app.pid ? (
      <Button
        variant="primary"
        aria-label={`Confirm: quit ${app.name}`}
        onClick={() => void quit(app)}
      >
        Confirm Quit
      </Button>
    ) : (
      <Button aria-label={`Quit ${app.name}`} onClick={() => setArmed(app.pid)}>
        Quit
      </Button>
    );

  if (error)
    return (
      <div className="page">
        <Head />
        <Note tone="caution">{error}</Note>
      </div>
    );
  if (!report)
    return (
      <div className="page">
        <Head />
        <Spinner />
      </div>
    );

  const latest = report.latest;
  const total = latest?.memoryTotalBytes ?? 0;
  const used = report.samples.map((sample) => sample.memoryUsedBytes);
  const cached = report.samples.map((sample) => sample.memoryCachedBytes);
  const windowMinutes = (report.samples.length * report.intervalMs) / 60_000;
  // "Over the last 0 minutes" is a worse answer than saying it is still
  // collecting.
  const windowLabel =
    windowMinutes < 1
      ? "Collecting, a few seconds so far"
      : `The last ${Math.round(windowMinutes)} ${Math.round(windowMinutes) === 1 ? "minute" : "minutes"}`;
  const status = PRESSURE[pressure];
  const heaviest = apps?.find((app) => app.canQuit) ?? null;
  const largest = apps?.[0]?.residentBytes ?? 0;
  const free = latest
    ? Math.max(0, total - latest.memoryUsedBytes - latest.memoryCachedBytes)
    : 0;

  return (
    <div className="page">
      <Head
        aside={
          <Button
            variant="quiet"
            onClick={() =>
              report.running ? engine.monitorStop() : engine.monitorStart(POLL_MS, null)
            }
          >
            {report.running ? "Pause" : "Resume"}
          </Button>
        }
      >
        {latest && (
          <div className="memory-head">
            <p className="bigline">
              <b className="num">{formatBytes(latest.memoryUsedBytes)}</b>
              <span>of {formatBytes(total)} in use</span>
            </p>
            {status && <StatusChip status={status} />}
          </div>
        )}
        <p className="lede">{report.detail}</p>
      </Head>

      {heaviest && (pressure === "warning" || pressure === "critical") && (
        <div className={`banner banner--${pressure === "critical" ? "act" : "attention"}`}>
          <div className="banner__text">
            <p className="banner__title">{report.headline}</p>
            <p>
              Quitting {heaviest.name} would free about {formatBytes(heaviest.residentBytes)}.
              It can ask to save your work first.
            </p>
          </div>
          {quitButton(heaviest)}
        </div>
      )}

      <section className="card" aria-label="Memory over time">
        <div className="card__head">
          <p className="card__title">Memory in use</p>
          <p className="card__note">{windowLabel} · cached files shown beneath</p>
        </div>
        <div className="card__chart">
          <Sparkline
            values={used}
            baseline={cached}
            max={total}
            height={140}
            label="Memory in use over time, with cached files stacked beneath"
          />
        </div>
        <div className="stats">
          <Stat label="Used by apps and macOS" value={formatBytes(latest?.memoryUsedBytes)} />
          <Stat label="Cached files" value={formatBytes(latest?.memoryCachedBytes)} />
          <Stat label="Free" value={formatBytes(free)} />
          <Stat label="Moved to disk (swap)" value={formatBytes(latest?.swapUsedBytes ?? 0)} />
        </div>
      </section>

      <section className="card" aria-label="Apps using the most memory">
        <div className="card__head">
          <p className="card__title">What is using it</p>
          <p className="card__note">
            Largest first. Helper processes are counted with their app.
          </p>
        </div>
        {message && (
          <div className="card__message">
            <Note tone="info">{message}</Note>
          </div>
        )}
        {apps === null ? (
          <Spinner />
        ) : apps.length === 0 ? (
          <p className="rows__empty">Mr Clean could not read per-app memory on this Mac.</p>
        ) : (
          <ul className="procs">
            {apps.map((app) => (
              <li key={`${app.pid}-${app.name}`} className="procs__row">
                <span className="app-tile" aria-hidden="true">
                  {app.name.trim().charAt(0).toUpperCase() || "?"}
                </span>
                <div className="procs__text">
                  <p className="procs__name">{app.name}</p>
                  <p className="procs__sub">
                    {app.instanceCount > 1 ? `${app.instanceCount} processes` : "1 process"}
                  </p>
                </div>
                <span className="procs__bar" aria-hidden="true">
                  <i
                    style={{
                      width: `${largest > 0 ? Math.max(2, (app.residentBytes / largest) * 100) : 0}%`,
                    }}
                  />
                </span>
                <span className="procs__size num">{formatBytes(app.residentBytes)}</span>
                <span className="procs__action">
                  {app.canQuit ? quitButton(app) : <span className="faint">Background</span>}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <p className="faint">
        Cached memory is not wasted memory: macOS hands it to any app that asks. What frees
        memory when you are running low is quitting an app you are not using. Mr Clean also
        shows memory in the menu bar while it is running.
      </p>
    </div>
  );
}

function Head({ aside, children }: { aside?: React.ReactNode; children?: React.ReactNode }) {
  return (
    <header className="page-head">
      <div className="page-head__row">
        <p className="kicker">Read every second while this screen is open</p>
        {aside}
      </div>
      {children}
    </header>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="stats__item">
      <p className="stats__label">{label}</p>
      <p className="stats__value num">{value}</p>
    </div>
  );
}
