import { CATEGORY_COLORS, Meter } from "../../components/Meter";
import { Note, Panel, Stat } from "../../components/primitives";
import { formatBytes, formatCount, formatDuration, formatRelative } from "../../lib/format";
import type { EntryRow, StorageSnapshot } from "../../lib/types";

/**
 * What the scan found, before the user navigates anywhere.
 *
 * The order is deliberate: totals, then where the space went, then the specific
 * folders and files worth looking at. Anything Mr Clean could not read is
 * stated plainly at the end rather than hidden or dramatised.
 */
export function StorageOverview({
  snapshot,
  onOpen,
}: {
  snapshot: StorageSnapshot;
  onOpen: (entry: EntryRow) => void;
}) {
  const { scan, categories, largestFolders, largestFiles, errors } = snapshot;
  const categorised = categories.reduce((sum, bucket) => sum + bucket.totalSize, 0);

  return (
    <div className="stack">
      {scan.status === "cancelled" && (
        <Note tone="caution" title="This scan was cancelled.">
          What it managed to index is below. Scan again for a complete picture.
        </Note>
      )}

      <Panel
        title="What Mr Clean indexed"
        note={`${formatRelative(scan.startedAt)} · ${formatDuration(scan.durationMs)}`}
      >
        <div className="stat-row">
          <Stat label="Indexed" value={formatBytes(scan.logicalBytes)} />
          <Stat label="Files" value={formatCount(scan.fileCount)} small />
          <Stat label="Folders" value={formatCount(scan.dirCount)} small />
          <Stat
            label="Scan rate"
            value={`${formatCount(scan.entriesPerSecond)}/s`}
            small
            hint={`Index ${formatBytes(scan.indexBytes)}`}
          />
        </div>
      </Panel>

      {categories.length > 0 && (
        <Panel title="Where the space went">
          <Meter
            total={categorised}
            segments={categories.map((bucket) => ({
              id: bucket.category,
              label: bucket.label,
              value: bucket.totalSize,
              color: CATEGORY_COLORS[bucket.category] ?? "var(--cat-unknown)",
            }))}
          />
        </Panel>
      )}

      <div className="grid-two">
        <TopList
          title="Largest folders"
          note="Folders that only contain one big folder are left out."
          rows={largestFolders}
          onOpen={onOpen}
        />
        <TopList title="Largest files" rows={largestFiles} onOpen={onOpen} />
      </div>

      {errors.length > 0 && (
        <Panel
          title="Not included in these totals"
          note="Mr Clean reports what it could not read rather than counting it as empty."
        >
          <div className="stack" style={{ gap: "var(--space-3)" }}>
            {errors.map((bucket) => (
              <div key={bucket.kind}>
                <strong>{formatCount(bucket.count)} </strong>
                <span className="muted">{bucket.explanation}</span>
              </div>
            ))}
          </div>
        </Panel>
      )}
    </div>
  );
}

function TopList({
  title,
  note,
  rows,
  onOpen,
}: {
  title: string;
  note?: string;
  rows: EntryRow[];
  onOpen: (entry: EntryRow) => void;
}) {
  return (
    <Panel title={title} note={note} flush>
      {rows.length === 0 ? (
        <p className="muted" style={{ padding: "var(--space-4)" }}>
          Nothing to show.
        </p>
      ) : (
        <table className="table">
          <tbody>
            {rows.map((row) => (
              <tr
                key={row.id}
                data-navigable={row.kind === "directory"}
                onDoubleClick={row.kind === "directory" ? () => onOpen(row) : undefined}
              >
                <td className="table__cell--truncate">
                  <div className="table__name">
                    <span className="table__name-text" title={row.path}>
                      {row.name}
                    </span>
                  </div>
                  <div
                    className="faint table__name-text"
                    style={{ fontSize: "var(--text-xs)" }}
                  >
                    {parentOf(row.path)}
                  </div>
                </td>
                <td className="table__numeric">
                  {formatBytes(row.size)}
                  {row.sharesStorage && (
                    <div className="faint" style={{ fontSize: "var(--text-xs)" }}>
                      shared copy
                    </div>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  );
}

function parentOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut <= 0 ? "/" : path.slice(0, cut);
}
