// The wellbeing companion (Design.md §5 "Wellbeing companion"): what Pip does for each kind of
// reminder and how long a guided break lasts. Pure, so it is unit-tested; the words live in
// i18n (`t.companion`).
//
// Rules from the brief: invitations, not commands; no guilt; observed data stated accurately
// ("you've been on this PC for 52 minutes", never "you've been sitting"); general wellbeing
// only, no health claims.

import type { PipMood } from "../pip/renderer";
import { t } from "./i18n";
import type { ReminderKind } from "./types/generated/ReminderKind";

export interface CompanionActivity {
  /** Pip while inviting and during the break. */
  mood: PipMood;
  /** Guided break length in seconds; 0 = nothing to time (acknowledge, log water…). */
  breakS: number;
}

export const ACTIVITIES: Record<ReminderKind, CompanionActivity> = {
  eyes: { mood: "restEyes", breakS: 20 },
  stand: { mood: "stretch", breakS: 120 },
  // A quick nudge: "Done" counts it at once, no timer.
  posture: { mood: "proud", breakS: 0 },
  water: { mood: "sip", breakS: 0 },
  screenBalance: { mood: "breathe", breakS: 300 },
  bedtime: { mood: "yawn", breakS: 0 },
  meal: { mood: "happy", breakS: 0 },
  moodCheckIn: { mood: "curious", breakS: 0 },
  reset: { mood: "breathe", breakS: 60 },
};

/** The check-in answers, happiest first (same list as the backend). */
export const MOODS = [
  { id: "great", emoji: "😄" },
  { id: "good", emoji: "🙂" },
  { id: "okay", emoji: "😐" },
  { id: "low", emoji: "😕" },
  { id: "rough", emoji: "😣" },
] as const;

/** Drive-by nudges: said straight, answered with one tap, gone again in a minute. */
export function isNudge(kind: ReminderKind): boolean {
  return kind === "posture" || kind === "water";
}

/**
 * What Pip says for a nudge. One of a few lines, picked by the prompt's id so the pill and
 * the card say the same thing; the water lines carry the measured time.
 */
export function nudgeLine(prompt: { id: string; kind: ReminderKind; sinceMin: number }): string {
  let hash = 0;
  for (const char of prompt.id) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  if (prompt.kind === "water") {
    const lines = t.companion.nudges.water;
    const line = lines[hash % lines.length]!(t.companion.span(prompt.sinceMin));
    return line.charAt(0).toUpperCase() + line.slice(1);
  }
  const lines = t.companion.nudges.posture;
  return lines[hash % lines.length]!;
}

/**
 * Whether to mention how long the PC has been in use. Only for reminders that are about
 * screen time, and only once there's something real to say.
 */
export function showsUsage(kind: ReminderKind, activeMin: number): boolean {
  return (kind === "eyes" || kind === "stand" || kind === "screenBalance") && activeMin >= 5;
}

/** 125 → "2:05". */
export function formatCountdown(seconds: number): string {
  const s = Math.max(0, Math.ceil(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Seconds left in a break that ends at `endsAt` (ms). */
export function secondsLeft(endsAt: number, now: number): number {
  return Math.max(0, Math.ceil((endsAt - now) / 1000));
}

/** Pip's mood for the companion card / pill, or null when the companion is quiet. */
export function companionMood(
  session: { kind: ReminderKind; phase: "ask" | "break" | "done" } | null,
  prompt: { kind: ReminderKind } | null | undefined,
): PipMood | null {
  if (session?.phase === "done") return session.kind === "bedtime" ? "happy" : "celebrate";
  const kind = session?.kind ?? prompt?.kind;
  return kind ? ACTIVITIES[kind].mood : null;
}

/** A new prompt has Pip's attention this long; after that, music may win him back. */
export const PROMPT_ATTENTION_MS = 60_000;

/**
 * The mood Pip shows. A break in progress always wins. A waiting prompt wins only while it
 * is shown in the notch and fresh (or nothing is playing), so an unanswered reminder
 * doesn't stop Pip dancing for its whole 15 minutes. Music makes a resting Pip dance; alerts,
 * sad news and focus sessions keep their own mood.
 */
export function displayMoodFor<Mood extends string>(input: {
  mood: Mood;
  playing: boolean;
  session: { kind: ReminderKind; phase: "ask" | "break" | "done" } | null;
  prompt: { kind: ReminderKind; createdAt: number } | null | undefined;
  /** The prompt's kind is allowed in the notch (Settings → Modules). */
  promptShown: boolean;
  now: number;
}): Mood | PipMood | "dance" {
  const { mood, playing, session, prompt, promptShown, now } = input;
  const resting = mood === "idle" || mood === "happy" || mood === "sleepy";
  const dancing = playing && resting;
  if (session) return companionMood(session, null) ?? mood;
  if (prompt && promptShown && !(dancing && now - prompt.createdAt > PROMPT_ATTENTION_MS)) {
    return ACTIVITIES[prompt.kind].mood;
  }
  return dancing ? "dance" : mood;
}
