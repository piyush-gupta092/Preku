import { describe, expect, it } from "vitest";
import { dueChoices, dueLabel, fromLocalInput, toLocalInput, type DueWords } from "./due";

const at = (h: number, m = 0, day = 12) => new Date(2026, 9, day, h, m).getTime();

const words: DueWords = {
  overdue: "overdue",
  today: (time) => `today ${time}`,
  tomorrow: (time) => `tomorrow ${time}`,
  later: (date, time) => `${date} ${time}`,
};

describe("task reminder times", () => {
  it("offers an hour from now, this evening while it is far enough, and tomorrow morning", () => {
    const morning = dueChoices(at(10, 30));
    expect(morning.map((c) => c.id)).toEqual(["hour", "evening", "tomorrow"]);
    expect(morning[0]!.at).toBe(at(11, 30));
    expect(morning[1]!.at).toBe(at(18));
    expect(morning[2]!.at).toBe(at(9, 0, 13));
    // At 17:30 "this evening" is less than an hour away: not offered.
    expect(dueChoices(at(17, 30)).map((c) => c.id)).toEqual(["hour", "tomorrow"]);
    expect(dueChoices(at(22)).map((c) => c.id)).toEqual(["hour", "tomorrow"]);
  });

  it("says when a reminder is", () => {
    const now = at(10);
    expect(dueLabel(at(9, 59), now, words)).toEqual({ text: "overdue", overdue: true });
    expect(dueLabel(at(15), now, words).text).toMatch(/^today /);
    expect(dueLabel(at(9, 0, 13), now, words).text).toMatch(/^tomorrow /);
    const later = dueLabel(at(9, 0, 16), now, words);
    expect(later.overdue).toBe(false);
    expect(later.text).not.toMatch(/today|tomorrow/);
  });

  it("round-trips the date-time field", () => {
    const value = toLocalInput(at(15, 5));
    expect(value).toBe("2026-10-12T15:05");
    expect(fromLocalInput(value)).toBe(at(15, 5));
    expect(fromLocalInput("")).toBeNull();
    expect(fromLocalInput("nonsense")).toBeNull();
  });
});
