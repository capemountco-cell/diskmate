import { Note, Panel } from "../../components/primitives";
import { formatCount } from "../../lib/format";
import type { StorageSnapshot } from "../../lib/types";

/**
 * What is left of the old overview once the headline and the map have said it.
 *
 * The totals moved into the headline and the largest folders and files are the
 * map and the inspector's "Largest inside", so all that remains here is the
 * part no chart can carry: that a scan was cancelled, and what DiskMate could
 * not read. Both are reported rather than hidden, because a total that quietly
 * counts an unreadable folder as empty is a lie told by omission.
 */
export function ScanNotes({ snapshot }: { snapshot: StorageSnapshot }) {
  const { scan, errors } = snapshot;
  if (scan.status !== "cancelled" && errors.length === 0) return null;

  return (
    <>
      {scan.status === "cancelled" && (
        <Note tone="caution" title="This scan was cancelled.">
          What it managed to index is above. Scan again for a complete picture.
        </Note>
      )}

      {errors.length > 0 && (
        <Panel
          title="Not included in these totals"
          note="Reported rather than counted as empty."
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
    </>
  );
}
