import { describe, expect, it } from "vitest";
import { formatBytes, formatDuration, formatPercent, formatRate, percentOf } from "./format";

describe("format", () => {
  it("bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(50 * 1024 * 1024)).toBe("50 MB");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16 GB");
    expect(formatBytes(-5)).toBe("0 B");
  });

  it("durations", () => {
    expect(formatDuration(0)).toBe("<1m");
    expect(formatDuration(59)).toBe("<1m");
    expect(formatDuration(12 * 60 + 30)).toBe("12m");
    expect(formatDuration(3900)).toBe("1h 05m");
  });

  it("rates and percents", () => {
    expect(formatRate(2048)).toBe("2 KB/s");
    expect(formatPercent(42.6)).toBe("43%");
    expect(formatPercent(-1)).toBe("0%");
    expect(percentOf(1, 4)).toBe(25);
    expect(percentOf(1, 0)).toBe(0);
  });
});
