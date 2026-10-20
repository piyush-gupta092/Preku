import { describe, expect, it } from "vitest";
import { METRICS, METRIC_ORDER, clampThreshold, ruleSentence } from "./metrics";

describe("alert metrics", () => {
  it("offers all nine metrics with units", () => {
    expect(METRIC_ORDER).toHaveLength(9);
    expect(METRICS.netDown.unit).toBe("MB/s");
    expect(METRICS.diskFree.unit).toBe("GB");
    expect(METRICS.temperature.unit).toBe("°C");
  });

  it("defaults are inside each metric's range", () => {
    for (const metric of METRIC_ORDER) {
      const info = METRICS[metric];
      expect(info.threshold).toBeGreaterThanOrEqual(0);
      expect(info.threshold).toBeLessThanOrEqual(info.max);
    }
  });

  it("clamps per metric", () => {
    expect(clampThreshold("cpu", 150)).toBe(100);
    expect(clampThreshold("netDown", 150)).toBe(150);
    expect(clampThreshold("temperature", 400)).toBe(150);
    expect(clampThreshold("disk", -3)).toBe(0);
    expect(clampThreshold("battery", Number.NaN)).toBe(15);
  });
});

describe("alert sentences", () => {
  const words = {
    metric: "CPU",
    above: "above",
    below: "below",
    forDuration: (seconds: number) =>
      seconds >= 60 ? `for ${Math.round(seconds / 60)} min` : `for ${seconds} s`,
  };

  it("reads like a sentence, with the duration only when there is one", () => {
    expect(ruleSentence({ metric: "cpu", op: "above", threshold: 90, durationS: 30 }, words)).toBe(
      "CPU above 90% for 30 s",
    );
    expect(
      ruleSentence(
        { metric: "diskFree", op: "below", threshold: 10, durationS: 0 },
        { ...words, metric: "Disk free" },
      ),
    ).toBe("Disk free below 10 GB");
    expect(
      ruleSentence(
        { metric: "temperature", op: "above", threshold: 85, durationS: 120 },
        { ...words, metric: "Temperature" },
      ),
    ).toBe("Temperature above 85 °C for 2 min");
  });
});
