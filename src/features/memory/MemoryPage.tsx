import { useCallback, useEffect, useState } from "react";

import { Meter } from "../../components/Meter";
import { Button, Note, Panel, Spinner, Stat } from "../../components/primitives";
import { Sparkline } from "../../components/Sparkline";
import { formatBytes, formatPercent } from "../../lib/format";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import type { MemoryPressure, MonitorReport } from "../../lib/types";

const POLL_MS = 1_000;

/**
 * Memory, watched rather than sampled once.
 *
 * There is no "free memory" button here and there will not be one. macOS uses
 * memory that would otherwise sit idle as cache; emptying it does not make a
 * Mac faster, it makes it slower until the cache refills. What matters is
 * pressure over time, which is what this page shows.
 */
export function MemoryPage() {
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
      setError("The DiskMate engine is not running.");
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

  if (error) return <Note tone="caution">{error}</Note>;
  if (!report) return <Spinner />;

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
                color: "var(--cat-documents)",
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
                style={{ background: "var(--cat-documents)" }}
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

      <Note tone="info" title="Cached memory is not wasted memory.">
        Your Mac keeps recently used files in memory that nothing else needs right now. That is
        why a healthy machine reports very little free memory, and why DiskMate does not offer
        to empty it.
      </Note>

      <p className="faint">
        Per-process memory and any action on a running process are not part of this build.
      </p>
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
