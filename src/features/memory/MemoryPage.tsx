import { useEffect, useState } from "react";

import { Meter } from "../../components/Meter";
import { Note, Panel, Spinner, Stat } from "../../components/primitives";
import { formatBytes, formatPercent } from "../../lib/format";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import type { MemoryReport } from "../../lib/types";

const REFRESH_MS = 4_000;

/**
 * Memory, explained rather than scored.
 *
 * There is no "free memory" button here and there will not be one. macOS uses
 * memory that would otherwise sit idle as cache; emptying it does not make a
 * Mac faster, it makes it slower until the cache refills. What matters is
 * memory pressure, which is what this page leads with.
 */
export function MemoryPage() {
  const [report, setReport] = useState<MemoryReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!engineAvailable()) {
      setError("The DiskMate engine is not running.");
      return;
    }
    let active = true;
    const sample = () => {
      engine
        .memoryReport()
        .then((next) => active && setReport(next))
        .catch((cause) => active && setError(describeError(cause)));
    };
    sample();
    const timer = window.setInterval(sample, REFRESH_MS);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  if (error) return <Note tone="caution">{error}</Note>;
  if (!report) return <Spinner />;

  const { snapshot, headline, detail } = report;
  const segments = [
    {
      id: "used",
      label: "App memory and wired",
      value: snapshot.usedBytes,
      color: "var(--accent)",
    },
    {
      id: "cached",
      label: "Cached files",
      value: snapshot.cachedBytes,
      color: "var(--cat-documents)",
    },
  ];

  return (
    <div className="stack">
      <Panel title={headline} note={detail}>
        <Meter segments={segments} total={snapshot.totalBytes} />
      </Panel>

      <Panel title="Details">
        <div className="stat-row">
          <Stat label="Installed" value={formatBytes(snapshot.totalBytes)} small />
          <Stat
            label="In use"
            value={formatBytes(snapshot.usedBytes)}
            small
            hint={formatPercent(snapshot.usedBytes / Math.max(snapshot.totalBytes, 1))}
          />
          <Stat
            label="Cached"
            value={formatBytes(snapshot.cachedBytes)}
            small
            hint="Available on demand"
          />
          {snapshot.compressedBytes !== null && (
            <Stat label="Compressed" value={formatBytes(snapshot.compressedBytes)} small />
          )}
          {snapshot.swapUsedBytes !== null && (
            <Stat label="Swap used" value={formatBytes(snapshot.swapUsedBytes)} small />
          )}
        </div>
      </Panel>

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
