import { describe, expect, it } from "vitest";
import { en } from "./i18n/en";
import { dayKey, momentDue, removeThought, saveThought, skyLine, type WeatherNow } from "./moment";

const words = en.moment.sky;
const mumbai: WeatherNow = {
  temperature: 30.4,
  code: 0,
  isDay: true,
  unit: "celsius",
  place: "Mumbai",
};

describe("a moment for you", () => {
  it("says what the sky is doing, with the temperature and place", () => {
    expect(skyLine(mumbai, words)).toBe("It's bright and sunny out, and hot: 30° in Mumbai.");
    expect(skyLine({ ...mumbai, temperature: 22, code: 3 }, words)).toBe(
      "A soft grey sky today: 22° in Mumbai.",
    );
    expect(skyLine({ ...mumbai, temperature: 4, code: 71 }, words)).toContain("and chilly: 4°");
    // Night has its own words; Fahrenheit has its own thresholds.
    expect(skyLine({ ...mumbai, isDay: false, temperature: 24 }, words)).toMatch(/^A clear night/);
    expect(skyLine({ ...mumbai, unit: "fahrenheit", temperature: 80 }, words)).not.toContain("hot");
    expect(skyLine({ ...mumbai, unit: "fahrenheit", temperature: 90 }, words)).toContain("hot");
    // No place picked: nothing is said about the weather.
    expect(skyLine(null, words)).toBeNull();
  });

  it("greets without doubling punctuation", () => {
    expect(en.moment.hello("Good morning")).toBe("Good morning. Here's a moment for you.");
    expect(en.moment.hello("Up late?")).toBe("Up late? Here's a moment for you.");
    expect(en.moment.helloSky("Good evening", "A cloudy night: 22° in Pune.")).toBe(
      "Good evening. A cloudy night: 22° in Pune. Here's a moment for you.",
    );
  });

  it("is due once per local day, and only when switched on", () => {
    const today = new Date(2026, 9, 12, 9, 0);
    expect(dayKey(today)).toBe("2026-10-12");
    expect(momentDue(true, null, today)).toBe(true);
    expect(momentDue(true, "2026-10-11", today)).toBe(true);
    expect(momentDue(true, "2026-10-12", today)).toBe(false);
    expect(momentDue(false, null, today)).toBe(false);
    expect(momentDue(true, "2026-10-12", new Date(2026, 9, 13, 0, 1))).toBe(true);
  });

  it("keeps saved thoughts newest first without duplicates", () => {
    let saved = saveThought([], "a");
    saved = saveThought(saved, "b");
    saved = saveThought(saved, "a");
    expect(saved).toEqual(["a", "b"]);
    expect(removeThought(saved, "a")).toEqual(["b"]);
    const many = Array.from({ length: 130 }, (_, i) => `t${i}`).reduce(saveThought, [] as string[]);
    expect(many.length).toBe(100);
    expect(many[0]).toBe("t129");
  });
});
