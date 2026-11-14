import { useCallback, useEffect, useState } from "react";

import { Button, EmptyState, Note, Panel } from "../../components/primitives";
import { formatBytes, formatCount, formatRelative } from "../../lib/format";
import { describeError, engine } from "../../lib/ipc";
import type { Comparison, Snapshot } from "../../lib/types";

/**
 * Snapshots.
 *
 * A scanner tells you what is big. Remembering tells you what *got* big, which
 * is the question people actually have when a disk fills up over a month.
 */
export function SnapshotPanel({ scanId }: { scanId: number }) {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [comparison, setComparison] = useState<Comparison | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSnapshots(await engine.snapshots(20));
    } catch (cause) {
      setError(describeError(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const capture = async () => {
    try {
      const label = new Date().toLocaleString(undefined, {
        dateStyle: "medium",
        timeStyle: "short",
      });
      await engine.captureSnapshot(scanId, label);
      setError(null);
      await refresh();
    } catch (cause) {
      setError(describeError(cause));
    }
  };

  const compare = async (baseId: number, targetId: number) => {
    try {
      setComparison(await engine.compareSnapshots(baseId, targetId));
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    }
  };

  return (
    <Panel
      title="Snapshots"
      note="Record this scan, then compare it with a later one."
      action={<Button onClick={capture}>Take a snapshot</Button>}
    >
      {error && <Note tone="caution">{error}</Note>}

      {snapshots.length === 0 ? (
        <EmptyState title="No snapshots yet">
          Take one now, then another after a week, and Mr Clean will show you which folders grew
          in between.
        </EmptyState>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Taken</th>
              <th className="table__numeric">Indexed</th>
              <th className="table__numeric">Files</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {snapshots.map((snapshot, index) => (
              <tr key={snapshot.id}>
                <td>
                  {snapshot.label}
                  <div className="faint" style={{ fontSize: "var(--text-xs)" }}>
                    {formatRelative(snapshot.capturedAt)}
                  </div>
                </td>
                <td className="table__numeric">{formatBytes(snapshot.indexedBytes)}</td>
                <td className="table__numeric faint">{formatCount(snapshot.fileCount)}</td>
                <td className="table__numeric">
                  {index < snapshots.length - 1 && (
                    <Button
                      variant="quiet"
                      onClick={() => compare(snapshots[index + 1]!.id, snapshot.id)}
                    >
                      Compare with previous
                    </Button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {comparison && <ComparisonResult comparison={comparison} />}
    </Panel>
  );
}

function ComparisonResult({ comparison }: { comparison: Comparison }) {
  const grew = comparison.netBytes >= 0;
  const widest = Math.max(1, ...comparison.deltas.map((delta) => Math.abs(delta.deltaBytes)));

  return (
    <div style={{ marginTop: "var(--space-5)" }}>
      <h3 className="section-heading">
        {grew ? "Grew by " : "Shrank by "}
        {formatBytes(Math.abs(comparison.netBytes))}
      </h3>
      <p className="muted" style={{ marginBottom: "var(--space-3)" }}>
        Between {comparison.base.label} and {comparison.target.label}.
      </p>

      {comparison.deltas.length === 0 ? (
        <p className="muted">No folder changed by more than a megabyte.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Folder</th>
              <th className="table__numeric">Before</th>
              <th className="table__numeric">After</th>
              <th className="table__numeric">Change</th>
            </tr>
          </thead>
          <tbody>
            {comparison.deltas.slice(0, 20).map((delta) => (
              <tr key={delta.path}>
                <td className="table__cell--truncate">
                  <div className="table__name-text">{delta.name}</div>
                  <div className="candidate__path mono" title={delta.path}>
                    {delta.path}
                  </div>
                  <div className="delta-bar" style={{ marginTop: 4 }}>
                    <div
                      className={
                        delta.deltaBytes >= 0 ? "delta-bar__grew" : "delta-bar__shrank"
                      }
                      style={{
                        width: `${(Math.abs(delta.deltaBytes) / widest) * 50}%`,
                      }}
                    />
                  </div>
                </td>
                <td className="table__numeric faint">{formatBytes(delta.beforeBytes)}</td>
                <td className="table__numeric faint">{formatBytes(delta.afterBytes)}</td>
                <td className="table__numeric">
                  {delta.deltaBytes >= 0 ? "+" : "−"}
                  {formatBytes(Math.abs(delta.deltaBytes))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
