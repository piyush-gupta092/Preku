import type { ClipItem } from "../../../lib/types/generated/ClipItem";
import type { TimerSnapshot } from "../../../lib/types/generated/TimerSnapshot";

/** `mm:ss`, or `h:mm:ss` from an hour up (same format as the backend's live item). */
export function formatClock(totalSeconds: number): string {
  const s = Math.max(0, Math.round(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor(s / 60) % 60;
  const sec = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

/**
 * Seconds left, interpolated between backend ticks from `endsAt`, so the countdown is smooth.
 * The backend stays the source of truth: paused/idle snapshots are used as-is.
 */
export function remainingSeconds(timer: TimerSnapshot, nowMs: number): number {
  if (timer.status !== "running" || timer.endsAt === null) return timer.remainingS;
  return Math.max(0, Math.ceil((timer.endsAt - nowMs) / 1000));
}

/**
 * What "Focus on this" does for a task, given the timer:
 * - `start`: nothing is running, start a session for it;
 * - `open` / `resume`: it's this task's session already, just show it (resuming if paused);
 * - `confirm`: another session is running or paused; ask before ending it.
 */
export function focusOnAction(
  timer: Pick<TimerSnapshot, "status" | "taskId">,
  taskId: string,
): "start" | "open" | "resume" | "confirm" {
  if (timer.status === "idle") return "start";
  if (timer.taskId === taskId) return timer.status === "paused" ? "resume" : "open";
  return "confirm";
}

/** Focus time as people say it: "25 min", "1 h", "2 h 5 min". */
export function formatFocusTotal(totalSeconds: number): string {
  const minutes = Math.max(0, Math.round(totalSeconds / 60));
  if (minutes < 60) return `${minutes} min`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m ? `${h} h ${m} min` : `${h} h`;
}

/** 0…1 elapsed share of the session, for the progress ring. */
export function progress(timer: TimerSnapshot, nowMs: number): number {
  if (timer.totalS <= 0) return 0;
  return 1 - remainingSeconds(timer, nowMs) / timer.totalS;
}

/** Case-insensitive search over text previews; images match nothing but an empty query. */
export function filterClips(items: ClipItem[], query: string): ClipItem[] {
  const q = query.trim().toLowerCase();
  if (!q) return items;
  return items.filter((item) => item.kind === "text" && item.preview.toLowerCase().includes(q));
}

/** New order after moving `id` one step up (-1) or down (+1); unchanged at the edges. */
export function moveId(ids: string[], id: string, step: -1 | 1): string[] {
  const from = ids.indexOf(id);
  const to = from + step;
  if (from < 0 || to < 0 || to >= ids.length) return ids;
  const next = [...ids];
  [next[from], next[to]] = [next[to]!, next[from]!];
  return next;
}

/** Clamp the custom timer length to what the backend accepts. */
export function clampMinutes(value: number): number {
  if (!Number.isFinite(value)) return 25;
  return Math.min(180, Math.max(1, Math.round(value)));
}
