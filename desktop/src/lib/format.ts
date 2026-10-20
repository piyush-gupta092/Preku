// Locale-aware number formatting for metrics. Units are SI-style but binary-sized (KiB = 1024),
// labelled the way people expect to read them (KB/MB/GB).

const UNITS = ["B", "KB", "MB", "GB", "TB"];

function scaled(bytes: number): { value: number; unit: string } {
  let value = Math.max(0, bytes);
  let index = 0;
  while (value >= 1024 && index < UNITS.length - 1) {
    value /= 1024;
    index++;
  }
  return { value, unit: UNITS[index]! };
}

function number(value: number, digits: number): string {
  return value.toLocaleString(undefined, { maximumFractionDigits: digits });
}

/** 1536 → "1.5 KB"; one decimal below 10, none above. */
export function formatBytes(bytes: number): string {
  const { value, unit } = scaled(bytes);
  return `${number(value, value < 10 && unit !== "B" ? 1 : 0)} ${unit}`;
}

/** Bytes per second → "1.2 MB/s". */
export function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

/** 42.37 → "42%". */
export function formatPercent(value: number): string {
  return `${number(Math.round(Math.max(0, value)), 0)}%`;
}

/** Seconds → "1h 05m", "12m", "<1m". */
export function formatDuration(seconds: number): string {
  const minutes = Math.floor(Math.max(0, seconds) / 60);
  if (minutes < 1) return "<1m";
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h > 0 ? `${h}h ${String(m).padStart(2, "0")}m` : `${m}m`;
}

export function percentOf(used: number, total: number): number {
  return total > 0 ? (used / total) * 100 : 0;
}
