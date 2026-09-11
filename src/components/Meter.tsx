/**
 * A proportional bar with a legend.
 *
 * Used for volume capacity and for the category breakdown. It is the only chart
 * in Milestone 01: a treemap is more evocative but a stacked bar answers "what
 * is taking the space" faster, and the engine is where the effort belongs.
 */

import { formatBytes, formatPercent } from "../lib/format";

export interface MeterSegment {
  id: string;
  label: string;
  value: number;
  color: string;
}

export function Meter({
  segments,
  total,
  showLegend = true,
}: {
  segments: MeterSegment[];
  total: number;
  showLegend?: boolean;
}) {
  const visible = segments.filter((segment) => segment.value > 0);
  const accounted = visible.reduce((sum, segment) => sum + segment.value, 0);
  // Whatever the segments do not account for is shown as free space rather than
  // being silently scaled away, so the bar always represents the whole disk.
  const remainder = Math.max(total - accounted, 0);

  return (
    <div>
      <div
        className="meter"
        role="img"
        aria-label={visible
          .map((segment) => `${segment.label} ${formatBytes(segment.value)}`)
          .join(", ")}
      >
        {visible.map((segment) => (
          <div
            key={segment.id}
            className="meter__segment"
            style={{ flexGrow: segment.value, background: segment.color }}
            title={`${segment.label} — ${formatBytes(segment.value)}`}
          />
        ))}
        {remainder > 0 && (
          <div className="meter__segment" style={{ flexGrow: remainder, background: "transparent" }} />
        )}
      </div>

      {showLegend && (
        <ul className="meter__legend">
          {visible.map((segment) => (
            <li key={segment.id} className="legend-item">
              <span className="legend-item__swatch" style={{ background: segment.color }} />
              <span>{segment.label}</span>
              <span className="legend-item__value">
                {formatBytes(segment.value)}
                {total > 0 && ` · ${formatPercent(segment.value / total)}`}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** The colour assigned to each storage category, read from the design tokens. */
export const CATEGORY_COLORS: Record<string, string> = {
  applications: "var(--cat-applications)",
  documents: "var(--cat-documents)",
  media: "var(--cat-media)",
  downloads: "var(--cat-downloads)",
  developer: "var(--cat-developer)",
  caches: "var(--cat-caches)",
  system: "var(--cat-system)",
  other: "var(--cat-other)",
  unknown: "var(--cat-unknown)",
};
