// The Today card's logic (usability U2), pure so it is unit-tested: what the next action is,
// and which facts the recap line mentions.

import type { Task } from "../../lib/types/generated/Task";
import type { TimerSnapshot } from "../../lib/types/generated/TimerSnapshot";
import type { TodaySummary } from "../../lib/types/generated/TodaySummary";

export type NextAction =
  | { kind: "focusing"; title: string; paused: boolean }
  /** A session ran out and waits for Mark done / Continue / Take a break. */
  | { kind: "finished"; title: string }
  | { kind: "task"; task: Task; priority: boolean }
  | { kind: "empty" };

/**
 * What the user would do next: the running (or paused) session, the session that just
 * ended, today's first open priority, otherwise the first open task.
 */
export function nextAction(
  timer: Pick<TimerSnapshot, "status" | "label" | "finished">,
  tasks: Task[],
): NextAction {
  if (timer.status !== "idle") {
    return { kind: "focusing", title: timer.label, paused: timer.status === "paused" };
  }
  if (timer.finished) return { kind: "finished", title: timer.finished.label };
  const open = tasks.filter((task) => !task.done);
  const task = open.find((candidate) => candidate.priorityToday) ?? open[0];
  return task ? { kind: "task", task, priority: task.priorityToday } : { kind: "empty" };
}

export interface RecapWords {
  focused: (seconds: number) => string;
  done: (count: number) => string;
  priorities: (done: number, total: number) => string;
  nextBreak: (seconds: number) => string;
}

/**
 * The facts worth a mention today, in order. Nothing recorded means nothing said: an empty
 * recap is better than a row of zeros. `nextBreakS` is null when Wellbeing (or its break
 * reminders) is off.
 */
export function recapParts(
  today: TodaySummary,
  nextBreakS: number | null,
  words: RecapWords,
): string[] {
  const parts: string[] = [];
  if (today.focusS >= 60) parts.push(words.focused(today.focusS));
  if (today.priorities > 0) parts.push(words.priorities(today.prioritiesDone, today.priorities));
  if (today.tasksDone > 0) parts.push(words.done(today.tasksDone));
  if (nextBreakS !== null) parts.push(words.nextBreak(nextBreakS));
  return parts;
}
