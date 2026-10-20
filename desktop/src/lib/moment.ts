// "A moment for you": the once-a-day card with a greeting, a word about the weather (when the
// user has picked a place) and the thought for today. Pure helpers, unit-tested.

import { condition, type Condition } from "./weather";
import type { TemperatureUnit } from "./types/generated/TemperatureUnit";

/** Local calendar day as YYYY-MM-DD (what "already seen today" is compared with). */
export function dayKey(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export interface SkyWords {
  day: Record<Condition, string>;
  night: Record<Condition, string>;
  hot: string;
  chilly: string;
  /** "30° in Mumbai" */
  at: (temp: string, place: string) => string;
}

export interface WeatherNow {
  temperature: number;
  code: number;
  isDay: boolean;
  unit: TemperatureUnit;
  place: string;
}

const HOT = { celsius: 30, fahrenheit: 86 } as const;
const CHILLY = { celsius: 8, fahrenheit: 46 } as const;

/**
 * One sentence about the sky, e.g. "It's bright and sunny out, and hot: 30° in Mumbai."
 * Describes what the forecast says; no advice. `null` when no place is set.
 */
export function skyLine(weather: WeatherNow | null, words: SkyWords): string | null {
  if (!weather) return null;
  const sky = (weather.isDay ? words.day : words.night)[condition(weather.code)];
  const feel =
    weather.temperature >= HOT[weather.unit]
      ? `, ${words.hot}`
      : weather.temperature <= CHILLY[weather.unit]
        ? `, ${words.chilly}`
        : "";
  return `${sky}${feel}: ${words.at(`${Math.round(weather.temperature)}°`, weather.place)}.`;
}

/** Whether today's moment is still to be shown (not answered or seen yet today). */
export function momentDue(enabled: boolean, seenDay: string | null, now: Date): boolean {
  return enabled && seenDay !== dayKey(now);
}

const MAX_SAVED = 100;

/** Adds a thought to the saved list (newest first, no duplicates, bounded). */
export function saveThought(saved: readonly string[], thought: string): string[] {
  return [thought, ...saved.filter((s) => s !== thought)].slice(0, MAX_SAVED);
}

export function removeThought(saved: readonly string[], thought: string): string[] {
  return saved.filter((s) => s !== thought);
}
