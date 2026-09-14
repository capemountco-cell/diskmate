import { useMemo, useState } from "react";

import { buildHierarchy, squarify, type PlacedNode } from "../lib/hierarchy";
import { formatBytes, formatPercent } from "../lib/format";
import type { EntryRow } from "../lib/types";

import { CATEGORY_COLORS, categoryFill } from "./categories";

/** The coordinate space the layout is computed in. Cells render as a share of it. */
const SPACE = { width: 960, height: 560 };

/**
 * The treemap.
 *
 * Every rectangle is a real file or folder and its area is its share of the
 * disk. Colour carries the storage category, so the same folder is the same
 * colour here as in the breakdown bar above it; it is not decoration and it is
 * not random.
 *
 * Drawn as elements rather than SVG rects. A rect can hold a fill and a
 * clipped label; a card can hold a tint, a category dot, a name that truncates
 * properly and the size set large enough to read without hovering - which is
 * the difference between a chart you interrogate and one you glance at.
 */
export function Treemap({
  rows,
  rootId,
  onOpen,
  selectedId = null,
  onSelect,
}: {
  rows: EntryRow[];
  rootId: number;
  onOpen: (row: EntryRow) => void;
  /** Drawn with a ring, so the inspector and the map agree on what is chosen. */
  selectedId?: number | null;
  onSelect?: (row: EntryRow) => void;
}) {
  const [hovered, setHovered] = useState<PlacedNode | null>(null);

  const placed = useMemo(() => {
    const tree = buildHierarchy(rows, rootId);
    // Top level only: maxDepth is an inclusive bound, so 0 means no children.
    // Nested rectangles worked when a cell was a flat fill; as cards they stack
    // their own tint, padding and label on top of the parent's and the map
    // stops being readable at a glance. Depth is reached by opening the folder,
    // which is what the double-click is for.
    return squarify(tree, { x: 0, y: 0, ...SPACE }, 0, 0);
  }, [rows, rootId]);

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
      <div
        className="treemap__plot"
        role="img"
        aria-label="Treemap of folder sizes"
        onMouseLeave={() => setHovered(null)}
      >
        {placed.map((item) => (
          <Cell
            key={`${item.node.row.id}-${item.depth}`}
            item={item}
            isHovered={hovered?.node.row.id === item.node.row.id}
            isSelected={selectedId === item.node.row.id}
            onHover={setHovered}
            onOpen={onOpen}
            onSelect={onSelect}
          />
        ))}
      </div>

      <figcaption className="treemap__caption">
        {hovered ? (
          <>
            <span className="treemap__name">{hovered.node.row.name}</span>
            <span className="treemap__meta num">
              {formatBytes(hovered.node.row.size)}
              {total > 0 && ` · ${formatPercent(hovered.node.row.size / total)} of this view`}
            </span>
            <span className="treemap__path mono">{hovered.node.row.path}</span>
          </>
        ) : (
          <span className="muted">Click to inspect · double-click a folder to open it</span>
        )}
      </figcaption>
    </figure>
  );
}

function cellClass(isHovered: boolean, isSelected: boolean): string {
  return [
    "treemap__card",
    isHovered ? "treemap__card--hover" : "",
    isSelected ? "treemap__card--selected" : "",
  ]
    .filter(Boolean)
    .join(" ");
}

/**
 * How large the size reads, by how much room the cell has.
 *
 * Returning null is a real answer: a number crammed into a sliver is noise,
 * and the cell still carries its colour, its name and its area.
 */
function figureSize(width: number, height: number): number | null {
  if (width < 80 || height < 58) return null;
  if (width >= 190 && height >= 150) return 24;
  if (height >= 104) return 20;
  return 16;
}

function Cell({
  item,
  isHovered,
  isSelected,
  onHover,
  onOpen,
  onSelect,
}: {
  item: PlacedNode;
  isHovered: boolean;
  isSelected: boolean;
  onHover: (item: PlacedNode | null) => void;
  onOpen: (row: EntryRow) => void;
  onSelect?: (row: EntryRow) => void;
}) {
  const { node, depth, x, y, width, height } = item;
  if (width < 3 || height < 3) return null;

  const row = node.row;
  const isFolder = row.kind === "directory";
  const color = CATEGORY_COLORS[row.category] ?? "var(--cat-unknown)";
  const showName = width > 62 && height > 26;
  const figure = figureSize(width, height);

  return (
    <div
      className="treemap__cell"
      style={{
        left: `${(x / SPACE.width) * 100}%`,
        top: `${(y / SPACE.height) * 100}%`,
        width: `${(width / SPACE.width) * 100}%`,
        height: `${(height / SPACE.height) * 100}%`,
        zIndex: depth + 1,
      }}
      onMouseEnter={() => onHover(item)}
      onClick={onSelect ? () => onSelect(row) : undefined}
      onDoubleClick={isFolder ? () => onOpen(row) : undefined}
    >
      <div
        className={cellClass(isHovered, isSelected)}
        style={{ background: categoryFill(row.category) }}
        title={`${row.name} — ${formatBytes(row.size)}`}
      >
        {showName && (
          <div className="treemap__cell-head">
            <i className="treemap__dot" style={{ background: color }} />
            <span className="treemap__cell-name">{row.name}</span>
          </div>
        )}
        {figure !== null && (
          <div className="treemap__cell-size num" style={{ fontSize: figure }}>
            {formatBytes(row.size)}
          </div>
        )}
      </div>
    </div>
  );
}
