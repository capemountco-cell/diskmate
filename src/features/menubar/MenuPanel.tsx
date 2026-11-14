import { useCallback, useEffect, useState } from "react";

import { Sparkline } from "../../components/Sparkline";
import { formatBytes } from "../../lib/format";
import { describeError, engine, engineAvailable, requestWipe } from "../../lib/ipc";
import { foldByName } from "../../lib/processes";
import type { MemoryPressure, MemoryReport, ProcessMemory, VolumeInfo } from "../../lib/types";

const POLL_MS = 2_000;
/** About three minutes of readings at one every two seconds. */
const KEEP = 90;
const TOP = 5;

/** The word beside the dot, in the dot's colour. */
const WORD: Record<MemoryPressure, string> = {
  normal: "Normal",
  warning: "Elevated",
  critical: "High",
  unknown: "Unknown",
};

/**
 * The panel that opens from the dot in the menu bar.
 *
 * Memory, always in view: the dot is the pressure macOS reports, not a score,
 * and the top of the panel takes its colour. Below that, what is using memory
 * with a Quit for each ordinary app (Quit asks the app to quit the way its own
 * menu would; nothing is force-closed or purged), free space, and the app's
 * own commands. It reads only while it is open, so a closed panel costs
 * nothing.
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
    // The shortcuts the panel shows: ⌘O opens Mr Clean, ⌘Q quits it.
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void engine.hidePanel();
      if (!event.metaKey) return;
      if (event.key === "o") {
        event.preventDefault();
        void engine.openMainWindow();
      } else if (event.key === "q") {
        event.preventDefault();
        void engine.quitMrClean();
      }
    };
    window.addEventListener("focus", onFocus);
    window.addEventListener("keydown", onKey);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("keydown", onKey);
    };
  }, [read]);

  useEffect(() => {
    if (armed === null) return;
    const timer = window.setTimeout(() => setArmed(null), 4_000);
    return () => window.clearTimeout(timer);
  }, [armed]);

  /** Two clicks: the first arms the button, the second asks the app to quit. */
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

  const snapshot = report?.snapshot ?? null;
  const pressure = snapshot?.pressure ?? "unknown";
  const total = snapshot?.totalBytes ?? 0;
  const share = (bytes: number) => (total > 0 ? `${Math.min(bytes / total, 1) * 100}%` : "0%");

  return (
    <div className="shell mp" data-pressure={pressure}>
      <section className="mp__memory" aria-label="Memory in use">
        <div className="mp__row">
          <span className="mp__label">Memory in use</span>
          <span className="mp__pressure">
            <i aria-hidden="true" />
            {WORD[pressure]}
          </span>
        </div>
        <p className="mp__figure">
          <b className="num">{snapshot ? formatBytes(snapshot.usedBytes) : "—"}</b>
          {snapshot && <span>of {formatBytes(snapshot.totalBytes)}</span>}
        </p>
        {snapshot && (
          <div
            className="mp__meter"
            role="img"
            aria-label={`${formatBytes(snapshot.usedBytes)} in use and ${formatBytes(snapshot.cachedBytes)} cached, of ${formatBytes(total)}`}
          >
            <i className="mp__meter-used" style={{ width: share(snapshot.usedBytes) }} />
            <i className="mp__meter-cached" style={{ width: share(snapshot.cachedBytes) }} />
          </div>
        )}
        {snapshot && (
          <div className="mp__row mp__small">
            <span>
              Apps {formatBytes(snapshot.usedBytes)} · Cached{" "}
              {formatBytes(snapshot.cachedBytes)}
            </span>
            <span>
              Swap{" "}
              <b>{snapshot.swapUsedBytes ? formatBytes(snapshot.swapUsedBytes) : "0 bytes"}</b>
            </span>
          </div>
        )}
        <div className="mp__chart">
          {history.length > 1 && (
            <Sparkline
              values={history}
              max={total || 1}
              height={44}
              label="Memory in use over the last few minutes"
            />
          )}
        </div>
        <p className="mp__headline">{report ? report.headline : "Reading memory…"}</p>
      </section>

      <section className="mp__section" aria-label="Using the most memory">
        <p className="mp__label">Using the most memory</p>
        {message && <p className="mp__message">{message}</p>}
        <ul className="mp__apps">
          {(apps ?? []).map((app) => (
            <li key={`${app.pid}-${app.name}`} className="mp__app">
              <span className="mp__app-name">{app.name}</span>
              <span className="num mp__app-size">{formatBytes(app.residentBytes)}</span>
              <span className="mp__app-action">
                {app.canQuit &&
                  (armed === app.pid ? (
                    <button
                      type="button"
                      className="mp__quit mp__quit--armed"
                      aria-label={`Confirm: quit ${app.name}`}
                      onClick={() => void quit(app)}
                    >
                      Confirm
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="mp__quit"
                      aria-label={`Quit ${app.name}`}
                      onClick={() => setArmed(app.pid)}
                    >
                      Quit
                    </button>
                  ))}
              </span>
            </li>
          ))}
        </ul>
      </section>

      {disk && (
        <section className="mp__section mp__row" aria-label="Disk">
          <span className="mp__label">{disk.name}</span>
          <span className="mp__disk">
            <b className="num">
              {disk.availableBytes !== null ? formatBytes(disk.availableBytes) : "—"}
            </b>{" "}
            free{disk.totalBytes ? ` of ${formatBytes(disk.totalBytes)}` : ""}
          </span>
        </section>
      )}

      <nav className="mp__menu" aria-label="Mr Clean">
        <button type="button" onClick={() => void engine.openMainWindow()}>
          Open Mr Clean <kbd>⌘O</kbd>
        </button>
        <button
          type="button"
          onClick={() => {
            void requestWipe();
            void engine.openMainWindow();
          }}
        >
          Wipe down this Mac…
        </button>
        <button type="button" onClick={() => void engine.quitMrClean()}>
          Quit Mr Clean <kbd>⌘Q</kbd>
        </button>
      </nav>
    </div>
  );
}
