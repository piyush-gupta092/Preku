import { describe, expect, it } from "vitest";
import { armAngle } from "./renderer";

describe("Pip arms", () => {
  it("raise for happy moods and hang for sad ones", () => {
    expect(armAngle("celebrate", 0, 1)).toBeGreaterThan(Math.PI / 2);
    expect(armAngle("happy", 0, -1)).toBeGreaterThan(Math.PI / 2);
    expect(armAngle("alert", 0, 1)).toBeGreaterThan(Math.PI / 2);
    expect(armAngle("sad", 0, 1)).toBeLessThan(0.5);
    expect(armAngle("sleepy", 0, -1)).toBeLessThan(0.5);
  });

  it("idle arms sway a little but stay down", () => {
    for (let t = 0; t < 5000; t += 250) {
      const a = armAngle("idle", t, 1);
      expect(a).toBeGreaterThan(0.3);
      expect(a).toBeLessThan(0.8);
    }
  });

  it("celebrating arms wave out of phase", () => {
    expect(armAngle("celebrate", 300, -1)).not.toBeCloseTo(armAngle("celebrate", 300, 1));
  });
});
