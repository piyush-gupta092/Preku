import { describe, expect, it } from "vitest";
import type { LiveItem } from "../../lib/types/generated/LiveItem";
import {
  ACTIVITIES,
  accentFor,
  activityOf,
  isShown,
  isUrgent,
  liveText,
  nextRotation,
} from "./live";

const item = (priority: LiveItem["priority"]): LiveItem => ({
  id: "x",
  module: "demo",
  priority,
  title: "x",
  subtitle: null,
  progress: null,
  expiresAt: null,
  createdAt: 0,
});

describe("live helpers", () => {
  it("uses module accents and falls back to brand", () => {
    expect(accentFor("wellbeing")).toBe("var(--mod-wellbeing)");
    expect(accentFor("demo")).toBe("var(--brand)");
  });

  it("only alerts and calls are urgent", () => {
    expect(isUrgent(item("alert"))).toBe(true);
    expect(isUrgent(item("incomingCall"))).toBe(true);
    expect(isUrgent(item("timer"))).toBe(false);
    expect(isUrgent(item("info"))).toBe(false);
  });

  it("translates known kinds and falls back to backend text", () => {
    const messages = { "reminder.eyes": { title: "Rest your eyes", subtitle: "20 s" } };
    expect(liveText({ ...item("alert"), kind: "reminder.eyes" }, messages)).toEqual({
      title: "Rest your eyes",
      subtitle: "20 s",
    });
    expect(liveText({ ...item("alert"), kind: "unknown" }, messages).title).toBe("x");
    expect(liveText(item("alert"), messages).title).toBe("x");
  });

  it("rotates through items and wraps", () => {
    expect(nextRotation(0, 0)).toBe(0);
    expect(nextRotation(0, 1)).toBe(0);
    expect(nextRotation(0, 3)).toBe(1);
    expect(nextRotation(2, 3)).toBe(0);
  });
});

describe("notch activities", () => {
  it("maps live items to their activity", () => {
    expect(activityOf({ id: "productivity.media" })).toBe("nowPlaying");
    expect(activityOf({ id: "monitor.alert.default-cpu" })).toBe("systemAlerts");
    expect(activityOf({ id: "monitor.ci.preku/preku" })).toBe("ci");
    expect(activityOf({ id: "wellbeing.reminder.stand" })).toBe("breakReminders");
    expect(activityOf({ id: "wellbeing.reminder.water" })).toBe("waterReminders");
    expect(activityOf({ id: "something.new" })).toBeNull();
  });

  it("hides only what the user switched off", () => {
    const settings = { nowPlaying: false, ci: true };
    expect(isShown({ id: "productivity.media" }, settings)).toBe(false);
    expect(isShown({ id: "monitor.ci.x/y" }, settings)).toBe(true);
    expect(isShown({ id: "productivity.timer" }, settings)).toBe(true);
    expect(isShown({ id: "something.new" }, settings)).toBe(true);
  });

  it("every activity id is unique", () => {
    const ids = ACTIVITIES.flatMap((a) => [...a.ids]);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
