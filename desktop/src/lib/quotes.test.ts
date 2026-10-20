import { describe, expect, it } from "vitest";
import { THOUGHTS, thoughtFor } from "./quotes";

describe("thought for today", () => {
  it("is the same all day and different the next day", () => {
    const morning = new Date(2026, 9, 12, 6, 0);
    const night = new Date(2026, 9, 12, 23, 59);
    const tomorrow = new Date(2026, 9, 13, 0, 1);
    expect(thoughtFor(morning)).toBe(thoughtFor(night));
    expect(thoughtFor(tomorrow)).not.toBe(thoughtFor(morning));
  });

  it("goes through every thought before repeating", () => {
    const seen = new Set<string>();
    for (let day = 0; day < THOUGHTS.length; day++)
      seen.add(thoughtFor(new Date(2026, 0, 1 + day, 12)));
    expect(seen.size).toBe(THOUGHTS.length);
  });

  it("keeps every line short, unique and free of pressure words", () => {
    expect(new Set(THOUGHTS).size).toBe(THOUGHTS.length);
    for (const thought of THOUGHTS) {
      expect(thought.length, thought).toBeLessThanOrEqual(90);
      expect(thought, thought).not.toMatch(/\b(must|should|lazy|failure|never give up|hustle)\b/i);
    }
  });
});
