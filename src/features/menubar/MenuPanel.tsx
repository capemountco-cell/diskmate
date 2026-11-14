import { useCallback, useEffect, useState } from "react";

import brandMark from "../../assets/brand-broom.png";
import { Button } from "../../components/primitives";
import { Sparkline } from "../../components/Sparkline";
import { formatBytes } from "../../lib/format";
import { describeError, engine, engineAvailable, requestWipe } from "../../lib/ipc";
import { foldByName } from "../../lib/processes";
import type { MemoryReport, ProcessMemory, VolumeInfo } from "../../lib/types";
import { StatusChip } from "../overview/OverviewPage";

const POLL_MS = 2_000;
/** About three minutes of readings at one every two seconds. */
const KEEP = 90;
const TOP = 5;

const PRESSURE = {
  normal: { tone: "good", word: "Normal", quiet: true },
  warning: { tone: "attention", word: "Under pressure" },
  critical: { tone: "act", word: "High" },
  unknown: null,
} as const;

/**
 * The panel that opens from the broom in the menu bar.
 *
 * The same look as the app, at the size of a menu: memory now and over the
 * last few minutes, what is using it with a Quit for each ordinary app, free
 * space, and the ways into the app and into wipe-down mode. It reads only
 * while it is open (has focus), so a closed panel costs nothing.
 */
export function MenuPanel() {
  const [report, setReport] = useState<MemoryReport | null>(null);
  const [history, setHistory] = useState<number[]>([]);
  const [apps, setApps] = useState<ProcessMemory[] | null>(null);
  const [disk, setDisk] = useState<VolumeInfo | null>(null);
  const [armed, setArmed] = useState<number | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const read = useCallback(async () => {
    if (!engineAvailable()) return;
    try {
      const next = await engine.memoryReport();
      setReport(next);
      setHistory((current) => [...current, next.snapshot.usedBytes].slice(-KEEP));
      setApps(foldByName(await engine.topApps(TOP + 3)).slice(0, TOP));
    } catch (cause) {
      setMessage(describeError(cause));
    }
  }, []);

  useEffect(() => {
    if (!engineAvailable()) return;
    void read();
    engine
      .listVolumes()
      .then((volumes) =>
        setDisk(
          volumes.find((volume) => volume.mountPoint === "/") ??
            volumes.find((volume) => volume.kind === "internal") ??
            null,
        ),
      )
      .catch(() => setDisk(null));
    const timer = window.setInterval(() => document.hasFocus() && void read(), POLL_MS);
    const onFocus = () => void read();
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [read]);

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
      window.setTimeout(() => void read(), 1_500);
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
        Confirm
      </Button>
    ) : (
      <Button aria-label={`Quit ${app.name}`} onClick={() => setArmed(app.pid)}>
        Quit
      </Button>
    );

  const snapshot = report?.snapshot ?? null;
  const pressure = snapshot?.pressure ?? "unknown";
  const status = PRESSURE[pressure];
  const heaviest = apps?.find((app) => app.canQuit) ?? null;
  const largest = apps?.[0]?.residentBytes ?? 0;
  const diskShare =
    disk && disk.usedBytes !== null && disk.totalBytes
      ? disk.usedBytes / disk.totalBytes
      : null;

  return (
    <div className="shell panel-shell" data-section="memory">
      <header className="mp__head">
        <span className="mp__brand">
          <img src={brandMark} alt="" width={22} height={22} />
          Mr Clean
        </span>
        <button
          type="button"
          className="link-button"
          onClick={() => void engine.openMainWindow()}
        >
          Open app
        </button>
      </header>

      <section className="mp__section" aria-label="Memory">
        <p className="kicker">Memory</p>
        <div className="mp__figure">
          <p className="bigline">
            <b className="num">{snapshot ? formatBytes(snapshot.usedBytes) : "—"}</b>
            {snapshot && <span>of {formatBytes(snapshot.totalBytes)}</span>}
          </p>
          {status && <StatusChip status={status} />}
        </div>
        {history.length > 1 && (
          <Sparkline
            values={history}
            max={snapshot?.totalBytes ?? 1}
            height={56}
            label="Memory in use over the last few minutes"
          />
        )}
        <p className="mp__meta">
          {snapshot?.swapUsedBytes
            ? `${formatBytes(snapshot.swapUsedBytes)} moved to disk · `
            : ""}
          {snapshot ? `${formatBytes(snapshot.cachedBytes)} cached files` : "Reading…"}
        </p>
      </section>

      {heaviest && (pressure === "warning" || pressure === "critical") && (
        <div className={`banner banner--${pressure === "critical" ? "act" : "attention"}`}>
          <div className="banner__text">
            <p className="banner__title">Memory is short</p>
            <p>
              Quitting {heaviest.name} would free about {formatBytes(heaviest.residentBytes)}.
            </p>
          </div>
          {quitButton(heaviest)}
        </div>
      )}

      <section className="mp__section" aria-label="Using the most memory">
        <p className="kicker">Using the most</p>
        {message && <p className="mp__message">{message}</p>}
        <ul className="mp__apps">
          {(apps ?? []).map((app) => (
            <li key={`${app.pid}-${app.name}`} className="mp__app">
              <span className="app-tile app-tile--small" aria-hidden="true">
                {app.name.trim().charAt(0).toUpperCase() || "?"}
              </span>
              <span className="mp__app-text">
                <span className="mp__app-name">{app.name}</span>
                <span className="procs__bar" aria-hidden="true">
                  <i
                    style={{
                      width: `${largest > 0 ? Math.max(3, (app.residentBytes / largest) * 100) : 0}%`,
                    }}
                  />
                </span>
              </span>
              <span className="num mp__app-size">{formatBytes(app.residentBytes)}</span>
              <span className="mp__app-action">
                {app.canQuit ? quitButton(app) : <span className="faint">Background</span>}
              </span>
            </li>
          ))}
        </ul>
      </section>

      {disk && (
        <section className="mp__section" aria-label="Disk">
          <p className="kicker">{disk.name}</p>
          <p className="mp__disk">
            <b className="num">
              {disk.availableBytes !== null ? formatBytes(disk.availableBytes) : "—"}
            </b>{" "}
            free
            {disk.totalBytes ? ` of ${formatBytes(disk.totalBytes)}` : ""}
          </p>
          {diskShare !== null && (
            <div className="ov__bar ov__bar--thin" aria-hidden="true">
              <i style={{ width: `${Math.min(diskShare, 1) * 100}%` }} />
            </div>
          )}
        </section>
      )}

      <footer className="mp__foot">
        <Button
          onClick={() => {
            void requestWipe();
            void engine.openMainWindow();
          }}
        >
          Wipe down this Mac…
        </Button>
        <Button variant="primary" onClick={() => void engine.openMainWindow()}>
          Open Mr Clean
        </Button>
        <button type="button" className="link-button" onClick={() => void engine.quitMrClean()}>
          Quit
        </button>
      </footer>
    </div>
  );
}
