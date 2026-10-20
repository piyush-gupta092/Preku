import { describe, expect, it } from "vitest";
import type { Task } from "../../lib/types/generated/Task";
import { nextAction, recapParts, type RecapWords } from "./today";

const task = (id: string, change: Partial<Task> = {}): Task => ({
  id,
  title: `Task ${id}`,
  done: false,
  createdAt: 0,
  updatedAt: 0,
  focusSessions: 0,
  focusS: 0,
  priorityToday: false,
  dueAt: null,
  ...change,
});

const idle = { status: "idle" as const, label: "", finished: null };

const words: RecapWords = {
  focused: (seconds) => `${Math.round(seconds / 60)} min focused`,
  done: (count) => `${count} done`,
  priorities: (done, total) => `${done} of ${total} priorities`,
  nextBreak: (seconds) => `break in ${Math.round(seconds / 60)} min`,
};

describe("today card", () => {
  it("offers the running session before anything else", () => {
    const tasks = [task("a", { priorityToday: true })];
    expect(nextAction({ status: "running", label: "Essay", finished: null }, tasks)).toEqual({
      kind: "focusing",
      title: "Essay",
      paused: false,
    });
    expect(nextAction({ status: "paused", label: "Essay", finished: null }, tasks)).toMatchObject({
      kind: "focusing",
      paused: true,
    });
  });

  it("asks about a session that just ended", () => {
    const finished = { label: "Essay", taskId: "a", totalS: 1500, focusedS: 1500 };
    expect(nextAction({ ...idle, finished }, [task("a")])).toEqual({
      kind: "finished",
      title: "Essay",
    });
  });

  it("puts today's priority ahead of the list order, and skips finished tasks", () => {
    const tasks = [
      task("a"),
      task("b", { priorityToday: true, done: true }),
      task("c", { priorityToday: true }),
    ];
    expect(nextAction(idle, tasks)).toMatchObject({ kind: "task", priority: true });
    expect(nextAction(idle, tasks)).toHaveProperty("task.id", "c");
    // No priorities: the first open task.
    expect(nextAction(idle, [task("x", { done: true }), task("y")])).toMatchObject({
      kind: "task",
      priority: false,
      task: { id: "y" },
    });
    expect(nextAction(idle, [task("x", { done: true })])).toEqual({ kind: "empty" });
    expect(nextAction(idle, [])).toEqual({ kind: "empty" });
  });

  it("recaps only what was recorded", () => {
    const nothing = { focusS: 0, sessions: 0, tasksDone: 0, priorities: 0, prioritiesDone: 0 };
    expect(recapParts(nothing, null, words)).toEqual([]);
    // Wellbeing on: the next break is a fact too. Wellbeing off: not mentioned.
    expect(recapParts(nothing, 720, words)).toEqual(["break in 12 min"]);
    // Under a minute of focus isn't worth a line.
    expect(recapParts({ ...nothing, focusS: 40 }, null, words)).toEqual([]);

    const busy = { focusS: 3000, sessions: 2, tasksDone: 3, priorities: 3, prioritiesDone: 1 };
    expect(recapParts(busy, 300, words)).toEqual([
      "50 min focused",
      "1 of 3 priorities",
      "3 done",
      "break in 5 min",
    ]);
  });
});
