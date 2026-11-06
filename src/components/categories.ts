/**
 * The storage category palette.
 *
 * One place for the mapping from a category to the colour that carries it, so
 * a cache is the same colour in the capacity bar, the treemap, the inspector's
 * bars and a cleanup card. The values themselves live in the design tokens;
 * this is only the lookup.
 */

/**
 * The tint a category's treemap card is filled with.
 *
 * The solid colour above is kept for the dot and the bars. A cell filled with
 * it at full strength cannot carry a legible name and a size on top, which is
 * the whole point of the card.
 */
export const CATEGORY_TINTS: Record<string, { tint: string; edge: string }> = {
  applications: { tint: "var(--tint-applications)", edge: "var(--edge-applications)" },
  documents: { tint: "var(--tint-documents)", edge: "var(--edge-documents)" },
  media: { tint: "var(--tint-media)", edge: "var(--edge-media)" },
  downloads: { tint: "var(--tint-downloads)", edge: "var(--edge-downloads)" },
  developer: { tint: "var(--tint-developer)", edge: "var(--edge-developer)" },
  caches: { tint: "var(--tint-caches)", edge: "var(--edge-caches)" },
  system: { tint: "var(--tint-system)", edge: "var(--edge-system)" },
  other: { tint: "var(--tint-other)", edge: "var(--edge-other)" },
  unknown: { tint: "var(--tint-unknown)", edge: "var(--edge-unknown)" },
};

/** The gradient fill for a category card, falling back to the neutral tint. */
export function categoryFill(category: string): string {
  const pair = CATEGORY_TINTS[category] ?? CATEGORY_TINTS.unknown!;
  return `linear-gradient(158deg, ${pair.tint}, ${pair.edge})`;
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
