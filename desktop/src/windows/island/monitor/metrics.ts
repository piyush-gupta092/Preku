import type { Metric } from "../../../lib/types/generated/Metric";

export interface MetricInfo {
  /** Language-neutral unit shown next to the threshold. */
  unit: string;
  /** Largest threshold the backend accepts (alerts.rs `max_threshold`). */
  max: number;
  /** Sensible starting rule when this metric is picked. */
  threshold: number;
  op: "above" | "below";
}

/** Every alert metric, in menu order, with its unit and limits. */
export const METRICS: Record<Metric, MetricInfo> = {
  cpu: { unit: "%", max: 100, threshold: 90, op: "above" },
  memory: { unit: "%", max: 100, threshold: 90, op: "above" },
  swap: { unit: "%", max: 100, threshold: 80, op: "above" },
  disk: { unit: "%", max: 100, threshold: 95, op: "above" },
  diskFree: { unit: "GB", max: 100_000, threshold: 10, op: "below" },
  battery: { unit: "%", max: 100, threshold: 15, op: "below" },
  netDown: { unit: "MB/s", max: 100_000, threshold: 50, op: "above" },
  netUp: { unit: "MB/s", max: 100_000, threshold: 20, op: "above" },
  temperature: { unit: "°C", max: 150, threshold: 85, op: "above" },
};

export const METRIC_ORDER = Object.keys(METRICS) as Metric[];

/** A rule as people would say it: "CPU above 90% for 30 s", "Disk free below 10 GB". */
export function ruleSentence(
  rule: { metric: Metric; op: "above" | "below"; threshold: number; durationS: number },
  words: {
    metric: string;
    above: string;
    below: string;
    forDuration: (seconds: number) => string;
  },
): string {
  const unit = METRICS[rule.metric].unit;
  // "90%" reads better without a space; "10 GB" and "85 °C" with one.
  const amount = unit === "%" ? `${rule.threshold}%` : `${rule.threshold} ${unit}`;
  const base = `${words.metric} ${rule.op === "above" ? words.above : words.below} ${amount}`;
  return rule.durationS > 0 ? `${base} ${words.forDuration(rule.durationS)}` : base;
}

/** Keeps a threshold inside its metric's range (whole numbers). */
export function clampThreshold(metric: Metric, value: number): number {
  if (!Number.isFinite(value)) return METRICS[metric].threshold;
  return Math.min(METRICS[metric].max, Math.max(0, Math.round(value)));
}
