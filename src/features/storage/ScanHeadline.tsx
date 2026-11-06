import { CATEGORY_COLORS } from "../../components/categories";
import { Button } from "../../components/primitives";
import { formatBytes, formatCount, formatRelative } from "../../lib/format";
import type { StorageSnapshot, VolumeInfo } from "../../lib/types";

/**
 * The top of the Storage screen.
 *
 * It replaces three stacked panels - the volume meter, "What DiskMate indexed"
 * and "Where the space went" - with one figure, one bar and a legend. The
 * panels each had a title, a note and a table; between them they said in about
 * sixty words what the bar says by being the right widths.
 *
 * The bar is deliberately in three kinds of segment rather than one. Drawing
 * the categories across the whole volume would imply DiskMate understands all
 * of it, and it only understands what it walked: what it indexed is coloured by
 * category, the rest of what the volume reports as used is grey, and free space
 * is empty. That distinction is the reason this is not simply a pie chart of
 * the disk.
 */
export function ScanHeadline({
  snapshot,
  volume,
  onRescan,
}: {
  snapshot: StorageSnapshot;
  volume: VolumeInfo | null;
  onRescan: () => void;
}) {
  const { scan, categories } = snapshot;
  const indexed = scan.logicalBytes;

  const total = volume?.totalBytes ?? null;
  const used = volume?.usedBytes ?? null;
  // Without volume figures the bar is simply the scan, which still answers
  // "where did it go" - it just cannot also answer "out of how much".
  const scale = total ?? indexed;
  const unindexed = used === null ? 0 : Math.max(used - indexed, 0);
  const free = total === null || used === null ? 0 : Math.max(total - used, 0);

  const buckets = categories.filter((bucket) => bucket.totalSize > 0);

  return (
    <header className="headline">
      <div className="headline__top">
        <div className="headline__figure">
          <span className="headline__number num">{formatBytes(indexed)}</span>
          <span className="headline__caption">indexed</span>
        </div>

        <span className="headline__rule" />

        <Figure value={formatCount(scan.fileCount)} label="Files" />
        <Figure value={formatCount(scan.dirCount)} label="Folders" />
        {volume?.availableBytes != null && (
          <Figure value={formatBytes(volume.availableBytes)} label="Free" />
        )}

        <div className="headline__actions">
          <span className="headline__when">{formatRelative(scan.startedAt)}</span>
          <Button onClick={onRescan}>Scan again</Button>
        </div>
      </div>

      <div
        className="headline__bar"
        role="img"
        aria-label={buckets
          .map((bucket) => `${bucket.label} ${formatBytes(bucket.totalSize)}`)
          .join(", ")}
      >
        {buckets.map((bucket) => (
          <span
            key={bucket.category}
            style={{
              flexGrow: bucket.totalSize,
              background: CATEGORY_COLORS[bucket.category] ?? "var(--cat-unknown)",
            }}
            title={`${bucket.label} — ${formatBytes(bucket.totalSize)}`}
          />
        ))}
        {unindexed > 0 && (
          <span
            style={{ flexGrow: unindexed, background: "var(--grey-5)" }}
            title={`Used, not indexed — ${formatBytes(unindexed)}`}
          />
        )}
        {free > 0 && <span style={{ flexGrow: free, background: "transparent" }} />}
      </div>

      <div className="headline__legend">
        {buckets.map((bucket) => (
          <Chip
            key={bucket.category}
            color={CATEGORY_COLORS[bucket.category] ?? "var(--cat-unknown)"}
            label={bucket.label}
            value={formatBytes(bucket.totalSize)}
          />
        ))}
        {unindexed > 0 && (
          <Chip color="var(--grey-5)" label="Not indexed" value={formatBytes(unindexed)} muted />
        )}
        {free > 0 && <Chip color="var(--grey-3)" label="Free" value={formatBytes(free)} muted />}
        {scale > 0 && total === null && (
          <span className="headline__chip headline__chip--muted">of what was scanned</span>
        )}
      </div>
    </header>
  );
}

function Figure({ value, label }: { value: string; label: string }) {
  return (
    <div className="headline__stat">
      <span className="num">{value}</span>
      <span>{label}</span>
    </div>
  );
}

function Chip({
  color,
  label,
  value,
  muted,
}: {
  color: string;
  label: string;
  value: string;
  muted?: boolean;
}) {
  return (
    <span className={muted ? "headline__chip headline__chip--muted" : "headline__chip"}>
      <i style={{ background: color }} />
      {label}
      <span className="num">{value}</span>
    </span>
  );
}
