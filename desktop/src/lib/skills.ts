// What installed skills add to the UI: thoughts for "A moment for you" and nudge sounds.
// Pure helpers over the installed list; the backend validated every skill already.

import { THOUGHTS, localDay } from "./quotes";
import type { InstalledSkill } from "./types/generated/InstalledSkill";
import type { SkillNote } from "./types/generated/SkillNote";

export interface Thought {
  text: string;
  /** Who said it, when a skill quotes someone. */
  author: string | null;
  /** The skill it came from; null for Preku's own thoughts. */
  skill: string | null;
}

/** Preku's own thoughts, then those of every switched-on skill. */
export function thoughtPool(skills: readonly InstalledSkill[]): Thought[] {
  const own = THOUGHTS.map((text) => ({ text, author: null, skill: null }));
  const added = skills
    .filter((skill) => skill.enabled)
    .flatMap((skill) =>
      skill.manifest.contributes.thoughts.map((thought) => ({
        text: thought.text,
        author: thought.author ?? null,
        skill: skill.manifest.name,
      })),
    );
  // A line in two places is offered once.
  const seen = new Set<string>();
  return [...own, ...added].filter((thought) => {
    const key = thought.text.trim().toLowerCase();
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/** The thought for this local day from `pool`: the same all day, another one tomorrow. */
export function thoughtOn(date: Date, pool: readonly Thought[]): Thought {
  const count = pool.length;
  return pool[((localDay(date) % count) + count) % count]!;
}

/** What a skill nudge (live item `skill.<skill id>.<nudge id>`) sounds like. */
export function nudgeSound(
  skills: readonly InstalledSkill[],
  itemId: string,
): "default" | "none" | SkillNote[] {
  const skill = skills.find((s) => itemId.startsWith(`skill.${s.manifest.id}.`));
  if (!skill) return "default";
  const nudgeId = itemId.slice(`skill.${skill.manifest.id}.`.length);
  const nudge = skill.manifest.contributes.nudges.find((n) => n.id === nudgeId);
  const sound = nudge?.sound ?? "default";
  if (sound === "default" || sound === "none") return sound;
  return skill.manifest.contributes.sounds.find((s) => s.id === sound)?.notes ?? "default";
}
