import { describe, expect, it } from "vitest";
import kindWords from "../../../skills/examples/kind-words.json";
import wrists from "../../../skills/examples/wrist-and-eye-care.json";
import { THOUGHTS } from "./quotes";
import { nudgeSound, thoughtOn, thoughtPool } from "./skills";
import type { InstalledSkill } from "./types/generated/InstalledSkill";
import type { SkillManifest } from "./types/generated/SkillManifest";

const installed = (manifest: unknown, enabled = true): InstalledSkill => ({
  manifest: manifest as SkillManifest,
  enabled,
});

describe("skills in the UI", () => {
  it("adds the thoughts of switched-on skills, with where they came from", () => {
    expect(thoughtPool([])).toHaveLength(THOUGHTS.length);
    const pool = thoughtPool([installed(kindWords)]);
    expect(pool).toHaveLength(THOUGHTS.length + kindWords.contributes.thoughts.length);
    const quote = pool.find((t) => t.author === "Aristotle");
    expect(quote).toEqual({
      text: "Well begun is half done.",
      author: "Aristotle",
      skill: "Kind words",
    });
    expect(thoughtPool([installed(kindWords, false)])).toHaveLength(THOUGHTS.length);
  });

  it("offers a line only once and keeps one thought all day", () => {
    const copy = {
      ...kindWords,
      id: "io.github.alex.copy",
      contributes: { thoughts: [{ text: THOUGHTS[0] }] },
    };
    expect(thoughtPool([installed(copy)])).toHaveLength(THOUGHTS.length);
    const pool = thoughtPool([installed(kindWords)]);
    const morning = thoughtOn(new Date(2026, 9, 19, 7), pool);
    expect(thoughtOn(new Date(2026, 9, 19, 23), pool)).toBe(morning);
    expect(thoughtOn(new Date(2026, 9, 20, 7), pool)).not.toBe(morning);
  });

  it("finds each nudge's sound", () => {
    const skills = [installed(wrists)];
    const bell = nudgeSound(skills, "skill.app.preku.wrist-and-eye-care.wrists");
    expect(Array.isArray(bell) && bell.length).toBe(3);
    expect(nudgeSound(skills, "skill.app.preku.wrist-and-eye-care.eyes")).toBe("none");
    expect(nudgeSound(skills, "skill.app.preku.wrist-and-eye-care.shoulders")).toBe("default");
    expect(nudgeSound(skills, "skill.io.github.gone.nudge")).toBe("default");
  });
});
