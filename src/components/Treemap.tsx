import { useMemo, useState } from "react";

import { buildHierarchy, squarify, type PlacedNode } from "../lib/hierarchy";
import { formatBytes, formatPercent } from "../lib/format";
import type { EntryRow } from "../lib/types";

import { CATEGORY_COLORS } from "./Meter";

/**
 * The treemap.
 *
 * Every rectangle is a real file or folder and its area is its share of the
 * disk. Colour carries the storage category, so the same folder is the same
 * colour here as in the breakdown bar above it; it is not decoration and it is
 * not random.
 *
 * Labels appear only where they fit. A treemap covered in truncated text is
 * harder to read than one that lets you hover.
 */
export function Treemap({
  rows,
  rootId,
  width = 900,
  height = 460,
  onOpen,
}: {
  rows: EntryRow[];
  rootId: number;
  width?: number;
  height?: number;
  onOpen: (row: EntryRow) => void;
}) {
  const [hovered, setHovered] = useState<PlacedNode | null>(null);

  const placed = useMemo(() => {
    const tree = buildHierarchy(rows, rootId);
    return squarify(tree, { x: 0, y: 0, width, height }, 0, 3);
  }, [rows, rootId, width, height]);

  if (placed.length === 0) {
    return (
      <p className="muted" style={{ padding: "var(--space-6)", textAlign: "center" }}>
        Nothing here takes up enough space to draw.
      </p>
    );
  }

  const total = placed
    .filter((p) => p.depth === 0)
    .reduce((sum, p) => sum + p.node.row.size, 0);

  return (
    <figure className="treemap">
      <svg
        viewBox={`0 0 ${width} ${height}`}
        width="100%"
        height={height}
        role="img"
        aria-label="Treemap of folder sizes"
        onMouseLeave={() => setHovered(null)}
      >
        {placed.map((item) => (
          <Cell
            key={`${item.node.row.id}-${item.depth}`}
            item={item}
            isHovered={hovered?.node.row.id === item.node.row.id}
            onHover={setHovered}
            onOpen={onOpen}
          />
        ))}
      </svg>

      <figcaption className="treemap__caption">
        {hovered ? (
          <>
            <span className="treemap__name">{hovered.node.row.name}</span>
            <span className="treemap__meta">
              {formatBytes(hovered.node.row.size)}
              {total > 0 && ` · ${formatPercent(hovered.node.row.size / total)} of this view`}
            </span>
            <span className="treemap__path mono">{hovered.node.row.path}</span>
          </>
        ) : (
          <span className="muted">
            Point at a rectangle to see what it is. Double-click a folder to go into it.
          </span>
        )}
      </figcaption>
    </figure>
  );
}

function Cell({
  item,
  isHovered,
  onHover,
  onOpen,
}: {
  item: PlacedNode;
  isHovered: boolean;
  onHover: (item: PlacedNode | null) => void;
  onOpen: (row: EntryRow) => void;
}) {
  const { node, depth, x, y, width, height } = item;
  if (width < 1 || height < 1) return null;

  const isFolder = node.row.kind === "directory";
  const color = CATEGORY_COLORS[node.row.category] ?? "var(--cat-unknown)";
  // Nested rectangles sit on top of their parent, so they lighten rather than
  // repeat the same fill: depth reads as depth without a second colour scale.
  const opacity = depth === 0 ? 0.9 : 0.55 + depth * 0.14;
  const showLabel = width > 52 && height > 18;

  return (
    <g
      onMouseEnter={() => onHover(item)}
      onDoubleClick={isFolder ? () => onOpen(node.row) : undefined}
      style={{ cursor: "default" }}
    >
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        fill={color}
        fillOpacity={opacity}
        stroke="var(--surface-panel)"
        strokeWidth={depth === 0 ? 1.5 : 0.75}
        rx={depth === 0 ? 3 : 1.5}
        className={isHovered ? "treemap__rect treemap__rect--hover" : "treemap__rect"}
      />
      {showLabel && (
        <text
          x={x + 6}
          y={y + 13}
          className="treemap__label"
          clipPath={`inset(0 0 0 0)`}
          style={{ fontSize: depth === 0 ? 11 : 10 }}
        >
          {truncateToWidth(node.row.name, width - 10, depth === 0 ? 11 : 10)}
        </text>
      )}
      {showLabel && height > 32 && depth === 0 && (
        <text x={x + 6} y={y + 27} className="treemap__sublabel">
          {formatBytes(node.row.size)}
        </text>
      )}
    </g>
  );
}

/**
 * Trim a label to the space available.
 *
 * SVG has no text overflow, so this estimates from an average glyph width
 * rather than measuring. Slightly conservative on purpose: a label that stops
 * short reads better than one that spills over its rectangle.
 */
function truncateToWidth(text: string, pixels: number, fontSize: number): string {
  const perChar = fontSize * 0.56;
  const max = Math.floor(pixels / perChar);
  if (max <= 1) return "";
  if (text.length <= max) return text;
  return `${text.slice(0, Math.max(max - 1, 1))}…`;
}
