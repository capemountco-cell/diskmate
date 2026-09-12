/**
 * A small line chart for the monitor.
 *
 * Deliberately minimal: no axes, no grid, no legend. It exists to show a shape
 * over time next to a number that gives the value, and anything more would be
 * chart furniture competing with the reading it annotates.
 */
export function Sparkline({
  values,
  baseline,
  max,
  height = 72,
  label,
}: {
  values: number[];
  /**
   * Optional second series drawn beneath the first, as a stacked band.
   *
   * Memory needs this: plotting only committed memory against installed memory
   * draws a flat line near the floor, which is accurate and tells you nothing.
   * Stacking cache underneath shows how full memory actually is, which is the
   * question the chart is there to answer.
   */
  baseline?: number[];
  /** Fixed ceiling, so the line does not rescale itself into looking flat. */
  max: number;
  height?: number;
  label: string;
}) {
  if (values.length < 2) {
    return (
      <svg className="sparkline" viewBox={`0 0 100 ${height}`} role="img" aria-label={label}>
        <line x1="0" y1={height - 1} x2="100" y2={height - 1} className="sparkline__axis" />
      </svg>
    );
  }

  const width = 100;
  const ceiling = Math.max(max, 1);
  const step = width / (values.length - 1);
  const toY = (value: number) =>
    height - (Math.min(Math.max(value, 0), ceiling) / ceiling) * (height - 4) - 2;
  const point = (index: number, value: number) =>
    `${(index * step).toFixed(2)},${toY(value).toFixed(2)}`;

  // The baseline is drawn as its own band and the main series rides on top of
  // it, so the two together read as "how full", not as two unrelated lines.
  const band = baseline?.length === values.length ? baseline : undefined;
  const stacked = values.map((value, index) => value + (band?.[index] ?? 0));

  const points = stacked.map((value, index) => point(index, value));
  const bandPoints = band?.map((value, index) => point(index, value));

  // The two areas are drawn as neighbouring slices rather than one on top of
  // the other, so neither has to be opaque and neither needs to know what
  // colour it is sitting on. The upper slice is closed along the band's top
  // edge, traced back right to left; without a band it closes on the floor.
  const fillOutline = bandPoints
    ? `${points.join(" ")} ${[...bandPoints].reverse().join(" ")}`
    : `0,${height} ${points.join(" ")} ${width},${height}`;

  return (
    <svg
      className="sparkline"
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={label}
    >
      {bandPoints ? (
        <polygon
          className="sparkline__band"
          points={`0,${height} ${bandPoints.join(" ")} ${width},${height}`}
        />
      ) : null}
      <polygon className="sparkline__fill" points={fillOutline} />
      <polyline
        className="sparkline__line"
        points={points.join(" ")}
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
