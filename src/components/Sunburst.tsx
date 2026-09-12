import { useMemo, useState } from "react";

import { buildHierarchy, radialLayout, type Arc } from "../lib/hierarchy";
import { formatBytes, formatPercent } from "../lib/format";
import type { EntryRow } from "../lib/types";

import { CATEGORY_COLORS } from "./Meter";

/**
 * The sunburst.
 *
 * A treemap is better at comparing sizes; rings are better at showing how deep
 * something is and what contains it. Both exist because they answer different
 * questions, which is also why there are two of them and not eight.
 */
export function Sunburst({
  rows,
  rootId,
  size = 460,
  onOpen,
}: {
  rows: EntryRow[];
  rootId: number;
  size?: number;
  onOpen: (row: EntryRow) => void;
}) {
  const [hovered, setHovered] = useState<Arc | null>(null);

  const arcs = useMemo(() => {
    const tree = buildHierarchy(rows, rootId);
    return radialLayout(tree, 4);
  }, [rows, rootId]);

  if (arcs.length === 0) {
    return (
      <p className="muted" style={{ padding: "var(--space-6)", textAlign: "center" }}>
        Nothing here takes up enough space to draw.
      </p>
    );
  }

  const center = size / 2;
  // A hole in the middle rather than a full pie: the inner degrees of a full
  // circle are unreadable, and the space is more useful as a label.
  const innerRadius = size * 0.16;
  const ringWidth = (center - innerRadius - 8) / 5;
  const total = arcs.filter((a) => a.depth === 0).reduce((sum, a) => sum + a.node.drawnSize, 0);

  return (
    <figure className="sunburst">
      <svg
        viewBox={`0 0 ${size} ${size}`}
        width={size}
        height={size}
        role="img"
        aria-label="Rings showing folder sizes by depth"
        onMouseLeave={() => setHovered(null)}
      >
        {arcs.map((arc) => {
          const inner = innerRadius + arc.depth * ringWidth;
          const outer = inner + ringWidth - 1.5;
          const color = CATEGORY_COLORS[arc.node.row.category] ?? "var(--cat-unknown)";
          const isHovered = hovered?.node.row.id === arc.node.row.id;
          return (
            <path
              key={`${arc.node.row.id}-${arc.depth}`}
              d={arcPath(center, center, inner, outer, arc.startAngle, arc.endAngle)}
              fill={color}
              fillOpacity={isHovered ? 1 : 0.82 - arc.depth * 0.1}
              stroke="var(--surface-panel)"
              strokeWidth={0.75}
              onMouseEnter={() => setHovered(arc)}
              onDoubleClick={
                arc.node.row.kind === "directory" ? () => onOpen(arc.node.row) : undefined
              }
            />
          );
        })}

        <text x={center} y={center - 4} className="sunburst__center-value">
          {formatBytes(hovered ? hovered.node.drawnSize : total)}
        </text>
        <text x={center} y={center + 12} className="sunburst__center-label">
          {hovered ? truncate(hovered.node.row.name, 18) : "in this view"}
        </text>
      </svg>

      <figcaption className="treemap__caption">
        {hovered ? (
          <>
            <span className="treemap__name">{hovered.node.row.name}</span>
            <span className="treemap__meta">
              {formatPercent(hovered.node.drawnSize / Math.max(total, 1))} of this view
            </span>
            <span className="treemap__path mono">{hovered.node.row.path}</span>
          </>
        ) : (
          <span className="muted">
            Each ring is one folder deeper. Double-click to go into one.
          </span>
        )}
      </figcaption>
    </figure>
  );
}

/** An annular sector, as an SVG path. */
function arcPath(
  cx: number,
  cy: number,
  inner: number,
  outer: number,
  start: number,
  end: number,
): string {
  // Start at twelve o'clock rather than three: a chart that begins where a
  // clock begins is easier to read against.
  const a0 = start - Math.PI / 2;
  const a1 = end - Math.PI / 2;
  const large = end - start > Math.PI ? 1 : 0;

  const x0 = cx + outer * Math.cos(a0);
  const y0 = cy + outer * Math.sin(a0);
  const x1 = cx + outer * Math.cos(a1);
  const y1 = cy + outer * Math.sin(a1);
  const x2 = cx + inner * Math.cos(a1);
  const y2 = cy + inner * Math.sin(a1);
  const x3 = cx + inner * Math.cos(a0);
  const y3 = cy + inner * Math.sin(a0);

  return [
    `M ${x0} ${y0}`,
    `A ${outer} ${outer} 0 ${large} 1 ${x1} ${y1}`,
    `L ${x2} ${y2}`,
    `A ${inner} ${inner} 0 ${large} 0 ${x3} ${y3}`,
    "Z",
  ].join(" ");
}

function truncate(text: string, max: number): string {
  return text.length <= max ? text : `${text.slice(0, max - 1)}…`;
}
