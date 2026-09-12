import { describe, expect, it } from "vitest";

import { buildHierarchy, radialLayout, squarify, totalOf, type Node } from "./hierarchy";
import type { EntryRow } from "./types";

function row(
  id: number,
  parentId: number | null,
  name: string,
  size: number,
  kind: EntryRow["kind"] = "directory",
): EntryRow {
  return {
    id,
    parentId,
    name,
    path: `/${name}`,
    kind,
    depth: 1,
    size,
    allocatedSize: size,
    fileCount: 0,
    dirCount: 0,
    extension: null,
    modifiedAt: null,
    category: "other",
    isHidden: false,
    accessible: true,
    shareOfParent: 0,
    sharesStorage: false,
  };
}

describe("buildHierarchy", () => {
  it("nests children under their parent and orders them largest first", () => {
    const nodes = buildHierarchy(
      [row(1, 0, "small", 100), row(2, 0, "large", 900), row(3, 2, "inside", 400)],
      0,
    );

    expect(nodes.map((node) => node.row.name)).toEqual(["large", "small"]);
    expect(nodes[0]!.children.map((node) => node.row.name)).toEqual(["inside"]);
  });

  it("drops a row whose parent is missing rather than reparenting it", () => {
    // The engine trims rows it did not return. Attaching an orphan to the root
    // would draw a rectangle somewhere it does not belong.
    const nodes = buildHierarchy([row(1, 0, "kept", 100), row(9, 77, "orphan", 500)], 0);

    expect(nodes.map((node) => node.row.name)).toEqual(["kept"]);
  });

  it("ignores zero-byte rows, which have no area to draw", () => {
    const nodes = buildHierarchy([row(1, 0, "empty", 0), row(2, 0, "real", 10)], 0);

    expect(nodes.map((node) => node.row.name)).toEqual(["real"]);
  });

  it("stops descending before a hard-linked loop can run away", () => {
    // A directory that claims itself as its own child. The scanner refuses to
    // follow these, but the layout must not depend on that.
    const rows = [row(1, 0, "top", 100), row(1, 1, "top", 100)];

    expect(() => buildHierarchy(rows, 0)).not.toThrow();
  });
});

describe("squarify", () => {
  const rect = { x: 0, y: 0, width: 400, height: 300 };

  it("fills the rectangle it was given, in proportion to size", () => {
    const nodes = buildHierarchy([row(1, 0, "a", 600), row(2, 0, "b", 400)], 0);
    const placed = squarify(nodes, rect, 0, 0);

    const area = placed.reduce((sum, p) => sum + p.width * p.height, 0);
    expect(area).toBeCloseTo(rect.width * rect.height, 0);

    const a = placed.find((p) => p.node.row.name === "a")!;
    const b = placed.find((p) => p.node.row.name === "b")!;
    expect((a.width * a.height) / (b.width * b.height)).toBeCloseTo(600 / 400, 2);
  });

  it("keeps every rectangle inside its parent", () => {
    const nodes = buildHierarchy(
      [
        row(1, 0, "big", 800),
        row(2, 1, "child-one", 500),
        row(3, 1, "child-two", 300),
        row(4, 0, "small", 200),
      ],
      0,
    );
    const placed = squarify(nodes, rect);

    for (const p of placed) {
      expect(p.x).toBeGreaterThanOrEqual(rect.x - 0.001);
      expect(p.y).toBeGreaterThanOrEqual(rect.y - 0.001);
      expect(p.x + p.width).toBeLessThanOrEqual(rect.x + rect.width + 0.001);
      expect(p.y + p.height).toBeLessThanOrEqual(rect.y + rect.height + 0.001);
    }
  });

  it("does not produce slivers, which is the reason for squarifying at all", () => {
    // Ten equal shares in a 4:3 box. Slice-and-dice would give each an aspect
    // ratio of 13:1; squarification should keep them comfortably readable.
    const rows = Array.from({ length: 10 }, (_, i) => row(i + 1, 0, `n${i}`, 100));
    const placed = squarify(buildHierarchy(rows, 0), rect, 0, 0);

    for (const p of placed) {
      const aspect = Math.max(p.width / p.height, p.height / p.width);
      expect(aspect).toBeLessThan(4);
    }
  });

  it("returns nothing for an empty list or an exhausted depth budget", () => {
    expect(squarify([], rect)).toEqual([]);
    expect(squarify(buildHierarchy([row(1, 0, "a", 10)], 0), rect, 5, 3)).toEqual([]);
  });
});

describe("radialLayout", () => {
  it("covers the full circle at the first ring", () => {
    const nodes = buildHierarchy([row(1, 0, "a", 750), row(2, 0, "b", 250)], 0);
    const arcs = radialLayout(nodes);

    const first = arcs.filter((arc) => arc.depth === 0);
    const swept = first.reduce((sum, arc) => sum + (arc.endAngle - arc.startAngle), 0);
    expect(swept).toBeCloseTo(Math.PI * 2, 5);
  });

  it("nests a child inside its parent's sweep", () => {
    const nodes = buildHierarchy([row(1, 0, "outer", 1000), row(2, 1, "inner", 1000)], 0);
    const arcs = radialLayout(nodes);

    const outer = arcs.find((arc) => arc.node.row.name === "outer")!;
    const inner = arcs.find((arc) => arc.node.row.name === "inner")!;
    expect(inner.depth).toBe(outer.depth + 1);
    expect(inner.startAngle).toBeGreaterThanOrEqual(outer.startAngle - 1e-9);
    expect(inner.endAngle).toBeLessThanOrEqual(outer.endAngle + 1e-9);
  });

  it("drops arcs too thin to aim at, rather than drawing hairlines", () => {
    const rows = [row(1, 0, "dominant", 1_000_000), row(2, 0, "speck", 1)];
    const arcs = radialLayout(buildHierarchy(rows, 0));

    expect(arcs.map((arc) => arc.node.row.name)).toEqual(["dominant"]);
  });
});

describe("totalOf", () => {
  it("sums the drawn size, not the reported size", () => {
    const nodes: Node[] = buildHierarchy([row(1, 0, "a", 30), row(2, 0, "b", 12)], 0);
    expect(totalOf(nodes)).toBe(42);
  });
});
