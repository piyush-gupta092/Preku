import { describe, expect, it } from "vitest";
import type { ClipItem } from "../../../lib/types/generated/ClipItem";
import type { TimerSnapshot } from "../../../lib/types/generated/TimerSnapshot";
import {
  clampMinutes,
  filterClips,
  formatClock,
  focusOnAction,
  formatFocusTotal,
  moveId,
  progress,
  remainingSeconds,
} from "./helpers";

const running = (endsAt: number, totalS = 600): TimerSnapshot => ({
  status: "running",
  label: "Focus",
  totalS,
  remainingS: 999, // stale on purpose: endsAt wins while running
  endsAt,
  taskId: null,
  finished: null,
});

describe("timer helpers", () => {
  it("never replaces a session silently from the task list", () => {
    expect(focusOnAction({ status: "idle", taskId: null }, "t1")).toBe("start");
    expect(focusOnAction({ status: "running", taskId: "t1" }, "t1")).toBe("open");
    expect(focusOnAction({ status: "paused", taskId: "t1" }, "t1")).toBe("resume");
    expect(focusOnAction({ status: "running", taskId: "t2" }, "t1")).toBe("confirm");
    expect(focusOnAction({ status: "paused", taskId: null }, "t1")).toBe("confirm");
  });

  it("says focus totals the way people do", () => {
    expect(formatFocusTotal(0)).toBe("0 min");
    expect(formatFocusTotal(25 * 60)).toBe("25 min");
    expect(formatFocusTotal(60 * 60)).toBe("1 h");
    expect(formatFocusTotal(125 * 60 + 20)).toBe("2 h 5 min");
  });

  it("formats clocks", () => {
    expect(formatClock(0)).toBe("00:00");
    expect(formatClock(25 * 60)).toBe("25:00");
    expect(formatClock(3725)).toBe("1:02:05");
    expect(formatClock(-5)).toBe("00:00");
  });

  it("interpolates a running timer from endsAt", () => {
    expect(remainingSeconds(running(10_000), 0)).toBe(10);
    expect(remainingSeconds(running(10_000), 9_001)).toBe(1);
    expect(remainingSeconds(running(10_000), 20_000)).toBe(0);
  });

  it("uses the snapshot as-is when paused", () => {
    const paused: TimerSnapshot = { ...running(0), status: "paused", remainingS: 42, endsAt: null };
    expect(remainingSeconds(paused, 123_456)).toBe(42);
  });

  it("reports progress", () => {
    expect(progress(running(300_000, 600), 0)).toBeCloseTo(0.5);
    expect(progress({ ...running(0), totalS: 0 }, 0)).toBe(0);
  });

  it("clamps custom minutes", () => {
    expect(clampMinutes(0)).toBe(1);
    expect(clampMinutes(500)).toBe(180);
    expect(clampMinutes(Number.NaN)).toBe(25);
    expect(clampMinutes(12.6)).toBe(13);
  });
});

describe("clipboard search", () => {
  const clip = (id: string, preview: string, kind: ClipItem["kind"] = "text"): ClipItem => ({
    id,
    kind,
    preview,
    width: null,
    height: null,
    pinned: false,
    createdAt: 0,
  });
  const items = [clip("a", "Hello World"), clip("b", "goodbye"), clip("c", "", "image")];

  it("matches text case-insensitively", () => {
    expect(filterClips(items, "WORLD").map((i) => i.id)).toEqual(["a"]);
  });

  it("returns everything for an empty query", () => {
    expect(filterClips(items, "  ")).toHaveLength(3);
  });
});

describe("task reordering", () => {
  it("swaps with the neighbour", () => {
    expect(moveId(["a", "b", "c"], "b", -1)).toEqual(["b", "a", "c"]);
    expect(moveId(["a", "b", "c"], "b", 1)).toEqual(["a", "c", "b"]);
  });

  it("ignores moves past the edges and unknown ids", () => {
    const ids = ["a", "b"];
    expect(moveId(ids, "a", -1)).toBe(ids);
    expect(moveId(ids, "b", 1)).toBe(ids);
    expect(moveId(ids, "z", 1)).toBe(ids);
  });
});
