import { en, type Strings } from "./en";

/** Active strings. Locale switching arrives with the settings window. */
export const t: Strings = en;

export function greetingFor(date: Date): string {
  const hour = date.getHours();
  if (hour >= 5 && hour < 12) return t.greeting.morning;
  if (hour >= 12 && hour < 18) return t.greeting.afternoon;
  if (hour >= 18 && hour < 23) return t.greeting.evening;
  return t.greeting.night;
}
