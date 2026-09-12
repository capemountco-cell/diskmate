import { formatBytes, formatCount, formatPercent } from "../lib/format";
import type { AgeBucket } from "../lib/types";

/**
 * Bytes by how long ago they were last touched.
 *
 * The most useful single view of a disk, because "big" is rarely actionable on
 * its own and "big and untouched for two years" almost always is. The bands
 * darken with age so the eye lands on the right-hand end without anything
 * needing to be coloured as a warning.
 */
export function AgeMap({ buckets }: { buckets: AgeBucket[] }) {
  const total = buckets.reduce((sum, bucket) => sum + bucket.totalSize, 0);
  if (total === 0) {
    return <p className="muted">No files in this scan carry a modification date.</p>;
  }

  const stale = buckets
    .filter((bucket) => bucket.fromDays >= 365)
    .reduce((sum, bucket) => sum + bucket.totalSize, 0);

  return (
    <div className="agemap">
      <div className="agemap__bar" role="img" aria-label="Bytes by age">
        {buckets.map((bucket, index) => {
          if (bucket.totalSize === 0) return null;
          // One hue, darkening with age. Age is a continuum, so a single ramp
          // reads as one; separate colours would imply separate categories.
          const shade = 0.22 + (index / Math.max(buckets.length - 1, 1)) * 0.62;
          return (
            <div
              key={bucket.label}
              className="agemap__band"
              style={{
                flexGrow: bucket.totalSize,
                background: `color-mix(in srgb, var(--accent) ${shade * 100}%, var(--surface-sunken))`,
              }}
              title={`${bucket.label}: ${formatBytes(bucket.totalSize)}`}
            />
          );
        })}
      </div>

      <table className="table agemap__table">
        <thead>
          <tr>
            <th>Last changed</th>
            <th className="table__numeric">Files</th>
            <th className="table__numeric">Size</th>
            <th className="table__numeric">Share</th>
          </tr>
        </thead>
        <tbody>
          {buckets.map((bucket) => (
            <tr key={bucket.label}>
              <td>{bucket.label}</td>
              <td className="table__numeric faint">{formatCount(bucket.fileCount)}</td>
              <td className="table__numeric">{formatBytes(bucket.totalSize)}</td>
              <td className="table__numeric faint">
                {formatPercent(bucket.totalSize / total)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {stale > 0 && (
        <p className="agemap__note">
          <strong>{formatBytes(stale)}</strong> has not been touched in over a year. That is a
          fact about age, not about whether you still want it.
        </p>
      )}
    </div>
  );
}
