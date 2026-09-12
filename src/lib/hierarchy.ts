/**
 * Turning the engine's flat, bounded row list into something drawable.
 *
 * The engine deliberately never hands over a tree: `subtree` returns the
 * largest few children at each level, capped. Rebuilding the shape here is a
 * few lines and keeps the boundary honest — the UI can only draw what it was
 * given, and it was given a bounded amount.
 */

import type { EntryRow } from "./types";

export interface Node {
  row: EntryRow;
  children: Node[];
  /** Sum of the drawn children, which is not the same as `row.size`. */
  drawnSize: number;
}

/**
 * Build the tree under `rootId` from a flat list.
 *
 * Rows whose parent is missing from the list are dropped rather than
 * reparented: they are the ones the engine trimmed, and inventing a position
 * for them would draw a rectangle that is not where it claims to be.
 */
export function buildHierarchy(rows: EntryRow[], rootId: number): Node[] {
  const byParent = new Map<number, EntryRow[]>();
  for (const row of rows) {
    if (row.parentId === null) continue;
    const siblings = byParent.get(row.parentId);
    if (siblings) siblings.push(row);
    else byParent.set(row.parentId, [row]);
  }

  const build = (parentId: number, depth: number): Node[] => {
    if (depth > 8) return [];
    const children = byParent.get(parentId) ?? [];
    return children
      .filter((row) => row.size > 0)
      .map((row) => {
        const kids = row.kind === "directory" ? build(row.id, depth + 1) : [];
        return { row, children: kids, drawnSize: row.size };
      })
      .sort((a, b) => b.drawnSize - a.drawnSize);
  };

  return build(rootId, 0);
}

/** Total of a node list, for laying out proportions. */
export function totalOf(nodes: Node[]): number {
  return nodes.reduce((sum, node) => sum + node.drawnSize, 0);
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PlacedNode extends Rect {
  node: Node;
  depth: number;
}

/**
 * Squarified treemap layout.
 *
 * The naive slice-and-dice layout produces slivers: rectangles so long and thin
 * that their area is impossible to compare and their label will not fit.
 * Squarification keeps aspect ratios near 1, which is the whole reason a
 * treemap is readable at all.
 *
 * Bruls, Huizing and van Wijk, "Squarified Treemaps" (2000).
 */
export function squarify(nodes: Node[], rect: Rect, depth = 0, maxDepth = 3): PlacedNode[] {
  const placed: PlacedNode[] = [];
  if (depth > maxDepth || nodes.length === 0) return placed;

  const total = totalOf(nodes);
  if (total <= 0) return placed;

  const remaining = [...nodes];
  let area = { ...rect };
  let remainingValue = total;

  while (remaining.length > 0 && area.width > 0.5 && area.height > 0.5) {
    const shortSide = Math.min(area.width, area.height);
    const scale = (area.width * area.height) / remainingValue;

    // Grow a row while doing so improves the worst aspect ratio in it.
    const row: Node[] = [];
    let rowValue = 0;
    let best = Infinity;
    while (remaining.length > 0) {
      const next = remaining[0]!;
      const candidateValue = rowValue + next.drawnSize;
      const ratio = worstAspect(
        [...row, next].map((n) => n.drawnSize * scale),
        candidateValue * scale,
        shortSide,
      );
      if (row.length > 0 && ratio > best) break;
      row.push(next);
      rowValue = candidateValue;
      best = ratio;
      remaining.shift();
    }

    const rowArea = rowValue * scale;
    const thickness = shortSide > 0 ? rowArea / shortSide : 0;
    let offset = 0;
    for (const node of row) {
      const share = rowValue > 0 ? node.drawnSize / rowValue : 0;
      const cell: Rect =
        area.width >= area.height
          ? {
              x: area.x,
              y: area.y + offset,
              width: thickness,
              height: area.height * share,
            }
          : {
              x: area.x + offset,
              y: area.y,
              width: area.width * share,
              height: thickness,
            };
      offset += area.width >= area.height ? cell.height : cell.width;
      placed.push({ ...cell, node, depth });

      // Recurse into folders that are still big enough to subdivide usefully.
      // Below this, another level of rectangles is visual noise.
      if (node.children.length > 0 && cell.width > 34 && cell.height > 26) {
        const inset = depth === 0 ? 14 : 3;
        placed.push(
          ...squarify(
            node.children,
            {
              x: cell.x + 1,
              y: cell.y + inset,
              width: Math.max(cell.width - 2, 0),
              height: Math.max(cell.height - inset - 1, 0),
            },
            depth + 1,
            maxDepth,
          ),
        );
      }
    }

    if (area.width >= area.height) {
      area = {
        x: area.x + thickness,
        y: area.y,
        width: area.width - thickness,
        height: area.height,
      };
    } else {
      area = {
        x: area.x,
        y: area.y + thickness,
        width: area.width,
        height: area.height - thickness,
      };
    }
    remainingValue -= rowValue;
  }

  return placed;
}

function worstAspect(areas: number[], rowArea: number, shortSide: number): number {
  if (rowArea <= 0 || shortSide <= 0) return Infinity;
  const thickness = rowArea / shortSide;
  let worst = 0;
  for (const area of areas) {
    const length = area / thickness;
    if (length <= 0) continue;
    worst = Math.max(worst, Math.max(thickness / length, length / thickness));
  }
  return worst || Infinity;
}

/** Arc segments for the sunburst, laid out by depth and share. */
export interface Arc {
  node: Node;
  depth: number;
  startAngle: number;
  endAngle: number;
}

export function radialLayout(nodes: Node[], maxDepth = 4): Arc[] {
  const arcs: Arc[] = [];
  const walk = (items: Node[], depth: number, start: number, end: number) => {
    if (depth > maxDepth) return;
    const total = totalOf(items);
    if (total <= 0) return;
    let angle = start;
    for (const node of items) {
      const sweep = ((end - start) * node.drawnSize) / total;
      // Below about a third of a degree an arc is a hairline nobody can aim at.
      if (sweep > 0.006) {
        arcs.push({ node, depth, startAngle: angle, endAngle: angle + sweep });
        if (node.children.length > 0) {
          walk(node.children, depth + 1, angle, angle + sweep);
        }
      }
      angle += sweep;
    }
  };
  walk(nodes, 0, 0, Math.PI * 2);
  return arcs;
}
