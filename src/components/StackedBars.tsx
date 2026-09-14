/**
 * A stacked column chart with a real ceiling.
 *
 * The sparkline it replaces scaled itself to whatever it had, which makes a
 * quiet machine and a struggling one draw the same picture. Here the full
 * height of the plot is a fixed maximum - installed memory - so how full the
 * chart looks is how full the thing is. Series are given top-first, the order
 * they stack on screen.
 */
export function StackedBars({
  series,
  max,
  height = 168,
  label,
  ceiling: ceilingLabel,
}: {
  series: Array<{ key: string; color: string; values: number[] }>;
  /** The ceiling the columns are drawn against. Never inferred from the data. */
  max: number;
  height?: number;
  label: string;
  /** Printed at the top of the plot, so the empty space has a value. */
  ceiling?: string;
}) {
  const columns = Math.max(0, ...series.map((s) => s.values.length));
  const ceiling = Math.max(max, 1);

  // Quarter gridlines, so a machine using a tenth of its memory reads as one
  // with room left rather than as a chart that failed to draw.
  const plot = (children: React.ReactNode) => (
    <div className="bars-plot">
      {ceilingLabel && <span className="bars-plot__ceiling num">{ceilingLabel}</span>}
      <div className="bars" style={{ height }} role="img" aria-label={label}>
        {children}
      </div>
    </div>
  );

  if (columns < 2) return plot(<span className="bars__waiting" />);

  return plot(
    <>
      {Array.from({ length: columns }, (_, index) => (
        <span className="bars__column" key={index}>
          {series.map((s) => {
            const value = s.values[index] ?? 0;
            if (value <= 0) return null;
            return (
              <span
                key={s.key}
                style={{
                  height: `${Math.min(100, (value / ceiling) * 100)}%`,
                  background: s.color,
                }}
              />
            );
          })}
        </span>
      ))}
    </>,
  );
}
