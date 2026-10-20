// Task reminder times (PRD A11): the quick choices and how a time is said. Pure, tested.

export interface DueChoice {
  id: "hour" | "evening" | "tomorrow";
  at: number;
}

/** "In 1 hour", "This evening" (18:00, only while that is at least an hour away), "Tomorrow morning" (9:00). */
export function dueChoices(now: number): DueChoice[] {
  const choices: DueChoice[] = [{ id: "hour", at: now + 60 * 60_000 }];
  const evening = new Date(now);
  evening.setHours(18, 0, 0, 0);
  if (evening.getTime() - now >= 60 * 60_000)
    choices.push({ id: "evening", at: evening.getTime() });
  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);
  tomorrow.setHours(9, 0, 0, 0);
  choices.push({ id: "tomorrow", at: tomorrow.getTime() });
  return choices;
}

export interface DueWords {
  overdue: string;
  today: (time: string) => string;
  tomorrow: (time: string) => string;
  later: (date: string, time: string) => string;
}

/** How a reminder time reads next to a task, and whether it has passed. */
export function dueLabel(
  dueAt: number,
  now: number,
  words: DueWords,
): { text: string; overdue: boolean } {
  if (dueAt <= now) return { text: words.overdue, overdue: true };
  const due = new Date(dueAt);
  const time = due.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  const days = Math.floor((dueAt - midnight.getTime()) / 86_400_000);
  if (days === 0) return { text: words.today(time), overdue: false };
  if (days === 1) return { text: words.tomorrow(time), overdue: false };
  const date = due.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" });
  return { text: words.later(date, time), overdue: false };
}

/** Unix ms → the value of an `<input type="datetime-local">` (local time, minutes). */
export function toLocalInput(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** The value of an `<input type="datetime-local">` → unix ms, or null when empty or invalid. */
export function fromLocalInput(value: string): number | null {
  if (!value) return null;
  const ms = new Date(value).getTime();
  return Number.isFinite(ms) ? ms : null;
}
