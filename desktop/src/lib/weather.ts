// WMO weather codes (Open-Meteo) → icon and a condition key the UI translates.
// https://open-meteo.com/en/docs — 0 clear … 99 thunderstorm with hail.

import type { TemperatureUnit } from "./types/generated/TemperatureUnit";

export type Condition =
  "clear" | "partlyCloudy" | "cloudy" | "fog" | "drizzle" | "rain" | "snow" | "showers" | "thunder";

export function condition(code: number): Condition {
  if (code === 0) return "clear";
  if (code <= 2) return "partlyCloudy";
  if (code === 3) return "cloudy";
  if (code === 45 || code === 48) return "fog";
  if (code >= 51 && code <= 57) return "drizzle";
  if ((code >= 61 && code <= 67) || code === 80 || code === 81 || code === 82) {
    return code >= 80 ? "showers" : "rain";
  }
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return "snow";
  if (code >= 95) return "thunder";
  return "cloudy";
}

const ICONS: Record<Condition, string> = {
  clear: "☀️",
  partlyCloudy: "⛅",
  cloudy: "☁️",
  fog: "🌫️",
  drizzle: "🌦️",
  rain: "🌧️",
  snow: "❄️",
  showers: "🌧️",
  thunder: "⛈️",
};

export function weatherIcon(code: number, isDay: boolean): string {
  const kind = condition(code);
  if (!isDay && kind === "clear") return "🌙";
  if (!isDay && kind === "partlyCloudy") return "☁️";
  return ICONS[kind];
}

/** 23.6 → "24°" (the unit letter is shown separately where there's room). */
export function formatTemp(value: number): string {
  return `${Math.round(value)}°`;
}

export function unitLetter(unit: TemperatureUnit): string {
  return unit === "fahrenheit" ? "F" : "C";
}
