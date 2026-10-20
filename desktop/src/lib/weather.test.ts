import { describe, expect, it } from "vitest";
import { condition, formatTemp, unitLetter, weatherIcon } from "./weather";

describe("weather", () => {
  it("maps WMO codes to conditions", () => {
    expect(condition(0)).toBe("clear");
    expect(condition(2)).toBe("partlyCloudy");
    expect(condition(3)).toBe("cloudy");
    expect(condition(45)).toBe("fog");
    expect(condition(53)).toBe("drizzle");
    expect(condition(63)).toBe("rain");
    expect(condition(81)).toBe("showers");
    expect(condition(75)).toBe("snow");
    expect(condition(86)).toBe("snow");
    expect(condition(95)).toBe("thunder");
    // Unknown codes fall back to cloudy.
    expect(condition(42)).toBe("cloudy");
  });

  it("uses night icons after dark", () => {
    expect(weatherIcon(0, true)).toBe("☀️");
    expect(weatherIcon(0, false)).toBe("🌙");
    expect(weatherIcon(63, false)).toBe("🌧️");
  });

  it("formats temperatures", () => {
    expect(formatTemp(23.6)).toBe("24°");
    expect(formatTemp(-0.4)).toBe("0°");
    expect(unitLetter("fahrenheit")).toBe("F");
    expect(unitLetter("celsius")).toBe("C");
  });
});
