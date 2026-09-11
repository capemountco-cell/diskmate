/**
 * Formatting helpers.
 *
 * Sizes use decimal units (1 KB = 1000 bytes) because that is what macOS has
 * reported since Snow Leopard, and a storage tool that disagrees with Finder
 * about the size of the same folder is a tool people stop trusting.
 */

const SIZE_UNITS = ["bytes", "KB", "MB", "GB", "TB", "PB"] as const;

export function formatBytes(bytes: number | null | undefined, fractionDigits?: number): string {
  if (bytes === null || bytes === undefined || Number.isNaN(bytes)) return "—";
  if (bytes === 0) return "Zero bytes";

  const sign = bytes < 0 ? "-" : "";
  let value = Math.abs(bytes);
  let unit = 0;
  while (value >= 1000 && unit < SIZE_UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  if (unit === 0) return `${sign}${Math.round(value)} bytes`;

  // One decimal below 100, none above: "8.3 GB" but "412 GB".
  const digits = fractionDigits ?? (value < 100 ? 1 : 0);
  return `${sign}${value.toFixed(digits)} ${SIZE_UNITS[unit]}`;
}

export function formatCount(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  return new Intl.NumberFormat().format(Math.round(value));
}

export function formatPercent(fraction: number, digits = 1): string {
  if (!Number.isFinite(fraction)) return "—";
  const percent = fraction * 100;
  if (percent > 0 && percent < 0.1) return "<0.1%";
  return `${percent.toFixed(percent >= 10 ? 0 : digits)}%`;
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds < 10 ? 1 : 0)} seconds`;
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  return remainder === 0 ? `${minutes} min` : `${minutes} min ${remainder} s`;
}

const DATE_FORMAT = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
});

export function formatDate(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "—";
  return DATE_FORMAT.format(new Date(unixSeconds * 1000));
}

export function formatDateTime(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "—";
  return new Date(unixSeconds * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

/** "2 hours ago", for scan recency. Falls back to a date beyond a week. */
export function formatRelative(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "—";
  const deltaSeconds = Math.round(Date.now() / 1000 - unixSeconds);
  if (deltaSeconds < 60) return "just now";
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["minute", 60],
    ["hour", 3600],
    ["day", 86_400],
  ];
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  for (let index = units.length - 1; index >= 0; index -= 1) {
    const entry = units[index];
    if (!entry) continue;
    const [unit, seconds] = entry;
    if (deltaSeconds >= seconds) {
      const amount = Math.round(deltaSeconds / seconds);
      if (unit === "day" && amount > 7) return formatDate(unixSeconds);
      return formatter.format(-amount, unit);
    }
  }
  return "just now";
}

/** Shorten a long path for a single line, keeping the ends that identify it. */
export function truncatePath(path: string, maxLength = 72): string {
  if (path.length <= maxLength) return path;
  const segments = path.split("/");
  if (segments.length <= 3) return `…${path.slice(-(maxLength - 1))}`;
  const tail = segments.slice(-2).join("/");
  const head = segments.slice(0, 2).join("/");
  return `${head}/…/${tail}`;
}
