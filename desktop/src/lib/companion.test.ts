import { describe, expect, it } from "vitest";
import {
  ACTIVITIES,
  companionMood,
  displayMoodFor,
  PROMPT_ATTENTION_MS,
  formatCountdown,
  isNudge,
  nudgeLine,
  secondsLeft,
  showsUsage,
} from "./companion";
import { en } from "./i18n/en";
import { EXPRESSIONS } from "../pip/expressions";

const KINDS = Object.keys(ACTIVITIES) as Array<keyof typeof ACTIVITIES>;

describe("wellbeing companion", () => {
  it("has an animation and words for every kind of reminder", () => {
    for (const kind of KINDS) {
      expect(EXPRESSIONS[ACTIVITIES[kind].mood], kind).toBeDefined();
      expect(en.companion.invite[kind], kind).toBeTruthy();
      expect(en.companion.short[kind], kind).toBeTruthy();
    }
  });

  it("matches the brief: stretch for movement, sip for water, breathe to reset, yawn at bedtime", () => {
    expect(ACTIVITIES.stand.mood).toBe("stretch");
    expect(ACTIVITIES.water.mood).toBe("sip");
    expect(ACTIVITIES.reset.mood).toBe("breathe");
    expect(ACTIVITIES.bedtime.mood).toBe("yawn");
  });

  it("invites instead of commanding", () => {
    for (const kind of KINDS) {
      const text = en.companion.invite[kind]!;
      // No guilt words, no orders.
      expect(text, kind).not.toMatch(/\b(must|should|need to|you forgot|again)\b/i);
    }
  });

  it("mentions screen use only when it is about screen use and measured", () => {
    expect(showsUsage("eyes", 52)).toBe(true);
    expect(showsUsage("eyes", 2)).toBe(false);
    expect(showsUsage("water", 90)).toBe(false);
    // The posture nudge stays one line.
    expect(showsUsage("posture", 90)).toBe(false);
    expect(showsUsage("bedtime", 90)).toBe(false);
    expect(en.companion.usage(52)).toContain("52");
  });

  it("nudges say it straight, with the measured time and no guilt", () => {
    expect(isNudge("posture")).toBe(true);
    expect(isNudge("water")).toBe(true);
    expect(isNudge("eyes")).toBe(false);

    const water = { id: "reminder.water.1", kind: "water" as const, sinceMin: 62 };
    expect(nudgeLine(water).toLowerCase()).toContain("an hour");
    expect(nudgeLine(water)).toBe(nudgeLine({ ...water }));
    expect(nudgeLine({ ...water, sinceMin: 40 })).toContain("40 min");
    expect(en.companion.span(80)).toBe("1 h 20 min");
    expect(en.companion.span(120)).toBe("2 hours");

    // Different pop-ins use different lines.
    const seen = new Set<string>();
    for (let i = 0; i < 40; i++) {
      seen.add(nudgeLine({ id: `reminder.posture.${i * 7919}`, kind: "posture", sinceMin: 30 }));
    }
    expect(seen.size).toBeGreaterThan(1);
    const all = [
      ...en.companion.nudges.posture,
      ...en.companion.nudges.water.map((line) => line("an hour")),
    ];
    for (const text of all) {
      expect(text).not.toMatch(/\b(must|should|need to|you forgot|again|lazy|bad)\b/i);
      expect(text.length).toBeLessThanOrEqual(40);
    }
  });

  it("lets Pip dance again once a waiting reminder is a minute old", () => {
    const base = {
      mood: "idle" as string,
      playing: true,
      session: null,
      prompt: null as { kind: "eyes"; createdAt: number } | null,
      promptShown: true,
      now: 100_000,
    };
    expect(displayMoodFor(base)).toBe("dance");
    expect(displayMoodFor({ ...base, playing: false })).toBe("idle");
    // Focus sessions and alerts keep their own mood.
    expect(displayMoodFor({ ...base, mood: "focus" })).toBe("focus");
    expect(displayMoodFor({ ...base, mood: "alert" })).toBe("alert");

    // A fresh reminder has Pip's attention; an old unanswered one no longer blocks the dance.
    const fresh = { kind: "eyes" as const, createdAt: 100_000 - 10_000 };
    const old = { kind: "eyes" as const, createdAt: 100_000 - PROMPT_ATTENTION_MS - 1 };
    expect(displayMoodFor({ ...base, prompt: fresh })).toBe("restEyes");
    expect(displayMoodFor({ ...base, prompt: old })).toBe("dance");
    // Without music the reminder keeps its pose; a hidden reminder never takes over.
    expect(displayMoodFor({ ...base, prompt: old, playing: false })).toBe("restEyes");
    expect(displayMoodFor({ ...base, prompt: fresh, promptShown: false })).toBe("dance");
    // A break in progress always wins.
    expect(displayMoodFor({ ...base, session: { kind: "reset", phase: "break" } })).toBe("breathe");
  });

  it("formats the countdown", () => {
    expect(formatCountdown(125)).toBe("2:05");
    expect(formatCountdown(0)).toBe("0:00");
    expect(secondsLeft(10_000, 1_000)).toBe(9);
    expect(secondsLeft(1_000, 5_000)).toBe(0);
  });

  it("picks Pip's mood from the break or the waiting prompt", () => {
    expect(companionMood(null, null)).toBeNull();
    expect(companionMood(null, { kind: "water" })).toBe("sip");
    expect(companionMood({ kind: "reset", phase: "break" }, null)).toBe("breathe");
    expect(companionMood({ kind: "eyes", phase: "done" }, null)).toBe("celebrate");
  });
});
