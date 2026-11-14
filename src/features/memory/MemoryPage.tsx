import { useCallback, useEffect, useState } from "react";

import { Meter } from "../../components/Meter";
import { Button, Note, Panel, Spinner, Stat } from "../../components/primitives";
import { Sparkline } from "../../components/Sparkline";
import { DisplayTitle } from "../../components/layout";
import { formatBytes, formatPercent } from "../../lib/format";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import type { MemoryPressure, MonitorReport, ProcessMemory } from "../../lib/types";

const POLL_MS = 1_000;

/**
 * Memory, watched rather than sampled once.
 *
 * There is no "free memory" button here and there will not be one. macOS uses
 * memory that would otherwise sit idle as cache; emptying it does not make a
 * Mac faster, it makes it slower until the cache refills. What matters is
 * pressure over time, which is what this page shows.
 */
export function MemoryPage({
  onPressure,
}: {
  /** Tells the shell the current pressure, which colours the window. */
  onPressure?: (pressure: MemoryPressure) => void;
} = {}) {
  const [report, setReport] = useState<MonitorReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const poll = useCallback(async () => {
    try {
      setReport(await engine.monitorReport());
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
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
    const timer = window.setInterval(poll, POLL_MS);
    return () => {
      window.clearInterval(timer);
      void engine.monitorStop();
    };
  }, [poll]);

  const currentPressure = report?.latest?.pressure ?? "unknown";
  useEffect(() => {
    onPressure?.(currentPressure);
  }, [currentPressure, onPressure]);

  if (error)
    return (
      <div className="page page--dashboard">
        <DisplayTitle title="Memory" />
        <Note tone="caution">{error}</Note>
      </div>
    );
  if (!report)
    return (
      <div className="page page--dashboard">
        <DisplayTitle title="Memory" />
        <Spinner />
      </div>
    );

  const latest = report.latest;
  const total = latest?.memoryTotalBytes ?? 0;
  const used = report.samples.map((sample) => sample.memoryUsedBytes);
  const cached = report.samples.map((sample) => sample.memoryCachedBytes);
  const swap = report.samples.map((sample) => sample.swapUsedBytes ?? 0);
  const peakSwap = Math.max(1, ...swap);
  const windowMinutes = (report.samples.length * report.intervalMs) / 60_000;
  // "Over the last 0 minutes" is a worse answer than saying it is still
  // collecting.
  const windowLabel =
    windowMinutes < 1
      ? "Collecting, a few seconds so far"
      : `Over the last ${Math.round(windowMinutes)} minutes`;

  return (
    <div className="page page--dashboard">
      <DisplayTitle title="Memory" aside="Read every second while this screen is open" />
      <div className="stack">
        <Panel
          title={
            <span>
              <span className={`pressure-dot pressure-dot--${report.peakPressure}`} />
              {report.headline}
            </span>
          }
          note={report.detail}
          action={
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
            <Meter
              total={total}
              segments={[
                {
                  id: "used",
                  label: "App memory and wired",
                  value: latest.memoryUsedBytes,
                  color: "var(--accent)",
                },
                {
                  id: "cached",
                  label: "Cached files",
                  value: latest.memoryCachedBytes,
                  color: "var(--grey-6)",
                },
              ]}
            />
          )}
        </Panel>

        <div className="monitor-grid">
          <Panel title="Memory in use" note={windowLabel}>
            <Sparkline
              values={used}
              baseline={cached}
              max={total}
              label="Memory in use over time, with cache stacked beneath"
            />
            <ul className="meter__legend">
              <li className="legend-item">
                <span className="legend-item__swatch" style={{ background: "var(--accent)" }} />
                <span>App memory</span>
              </li>
              <li className="legend-item">
                <span
                  className="legend-item__swatch"
                  style={{ background: "var(--grey-6)" }}
                />
                <span>Cached</span>
              </li>
            </ul>
            <div className="stat-row" style={{ marginTop: "var(--space-3)" }}>
              <Stat
                label="Now"
                value={formatBytes(latest?.memoryUsedBytes)}
                small
                hint={
                  total > 0 ? formatPercent((latest?.memoryUsedBytes ?? 0) / total) : undefined
                }
              />
              <Stat label="Installed" value={formatBytes(total)} small />
            </div>
          </Panel>

          <Panel title="Swap" note="Memory your Mac had to write to disk.">
            <Sparkline values={swap} max={peakSwap} label="Swap in use over time" />
            <div className="stat-row" style={{ marginTop: "var(--space-3)" }}>
              <Stat label="In use" value={formatBytes(latest?.swapUsedBytes ?? 0)} small />
              <Stat
                label="Pressure"
                value={<PressureLabel pressure={latest?.pressure ?? "unknown"} />}
                small
                hint={
                  report.peakPressure !== (latest?.pressure ?? "unknown")
                    ? `peaked at ${report.peakPressure}`
                    : undefined
                }
              />
            </div>
          </Panel>
        </div>

        <TopApps />

        <Note tone="info" title="Cached memory is not wasted memory.">
          Your Mac keeps recently used files in memory that nothing else needs right now, and
          hands that memory to any app that asks for it. What frees memory when you are running
          low is quitting an app you are not using, which is what the list above is for.
        </Note>

        <p className="faint">
          Mr Clean also shows memory in the menu bar while it is running. Closing this window
          keeps it there; choose Quit Mr Clean from that menu to stop it.
        </p>
      </div>
    </div>
  );
}

function PressureLabel({ pressure }: { pressure: MemoryPressure }) {
  const label = {
    normal: "Normal",
    warning: "Elevated",
    critical: "High",
    unknown: "Unknown",
  }[pressure];
  return (
    <span>
      <span className={`pressure-dot pressure-dot--${pressure}`} />
      {label}
    </span>
  );
}

const TOP_APPS = 8;

/**
 * The apps using the most memory, each with a Quit button when it is an
 * ordinary app.
 *
 * Quit takes two clicks: the first turns the button into "Confirm Quit", and
 * only the second sends the request. The request is the same one the app's own
 * Quit menu item sends, so it can still ask about unsaved work.
 */
function TopApps() {
  const [apps, setApps] = useState<ProcessMemory[] | null>(null);
  const [armed, setArmed] = useState<number | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setApps(await engine.topApps(TOP_APPS));
    } catch (cause) {
      setMessage(describeError(cause));
    }
  }, []);

  useEffect(() => {
    void load();
    const timer = window.setInterval(load, 3_000);
    return () => window.clearInterval(timer);
  }, [load]);

  // An armed button disarms itself, so a stray click later does nothing.
  useEffect(() => {
    if (armed === null) return;
    const timer = window.setTimeout(() => setArmed(null), 4_000);
    return () => window.clearTimeout(timer);
  }, [armed]);

  const quit = async (app: ProcessMemory) => {
    setArmed(null);
    try {
      await engine.quitApp(app.pid, app.name);
      setMessage(`Asked ${app.name} to quit. It may ask you to save first.`);
      window.setTimeout(load, 1_500);
    } catch (cause) {
      setMessage(describeError(cause));
    }
  };

  const largest = apps?.[0]?.residentBytes ?? 0;

  return (
    <Panel
      title="Using the most memory"
      note="Helper processes are counted with their app. Background processes are listed but not offered a Quit."
    >
      {message && (
        <div style={{ marginBottom: "var(--space-3)" }}>
          <Note tone="info">{message}</Note>
        </div>
      )}
      {apps === null ? (
        <Spinner />
      ) : apps.length === 0 ? (
        <p className="muted">Mr Clean could not read per-app memory on this Mac.</p>
      ) : (
        <ul className="top-apps">
          {apps.map((app) => (
            <li key={`${app.pid}-${app.name}`} className="top-apps__row">
              <div className="top-apps__text">
                <span className="top-apps__name">{app.name}</span>
                <span className="top-apps__meta">
                  {app.instanceCount > 1 ? `${app.instanceCount} processes` : "1 process"}
                  {!app.canQuit && " · background"}
                </span>
                <span className="top-apps__bar" aria-hidden="true">
                  <span
                    style={{
                      width: `${largest > 0 ? (app.residentBytes / largest) * 100 : 0}%`,
                    }}
                  />
                </span>
              </div>
              <span className="num top-apps__size">{formatBytes(app.residentBytes)}</span>
              {app.canQuit ? (
                armed === app.pid ? (
                  <Button
                    variant="primary"
                    aria-label={`Confirm: quit ${app.name}`}
                    onClick={() => void quit(app)}
                  >
                    Confirm Quit
                  </Button>
                ) : (
                  <Button onClick={() => setArmed(app.pid)}>Quit</Button>
                )
              ) : (
                <span className="top-apps__none" />
              )}
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
